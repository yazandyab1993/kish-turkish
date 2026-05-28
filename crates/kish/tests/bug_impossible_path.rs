//! Test for bug: impossible capture path generation
//!
//! ## Bug Description
//! The library generated geometrically impossible capture sequences.
//! Example: C8XC5XA5XH1XF1XH1XH5XH1
//!
//! Problems observed:
//! 1. A5→H1 is geometrically impossible (not on same row or column)
//! 2. H1 is visited 3 times in the path
//!
//! ## Root Cause
//! The `Action` struct stores moves as XOR deltas for efficiency, containing only:
//! - Source square (derived from delta)
//! - Final destination square (derived from delta)
//! - Captured pieces bitboard
//!
//! Critically, **intermediate landing squares are NOT stored**. When `to_detailed()`
//! reconstructed the path, it used a greedy algorithm that picked the first valid
//! landing square after each capture. For king multi-captures, this failed because:
//!
//! 1. Kings can land on ANY empty square beyond a captured piece (not just one)
//! 2. The greedy choice didn't verify remaining captures could be completed
//! 3. Choosing the wrong intermediate landing made subsequent captures unreachable
//!
//! ## Fix
//! Replaced the greedy algorithm with backtracking in `reconstruct_capture_path()`:
//! - Try all possible landing squares after each capture
//! - Recursively verify remaining captures can be completed
//! - Backtrack if a path doesn't lead to the final destination

use kish::{Board, Game, Square, State, Team};

/// Validates that a move path is geometrically valid for Turkish Checkers.
/// All moves must be horizontal or vertical (no diagonal).
fn validate_path(path: &[Square]) -> Result<(), String> {
    let mut visited = std::collections::HashSet::new();

    for (i, square) in path.iter().enumerate() {
        let notation = square.to_string();

        // Check for duplicate visits
        if !visited.insert(notation.clone()) {
            return Err(format!(
                "Square {} visited multiple times in path",
                notation
            ));
        }

        // Check geometry for consecutive squares
        if i > 0 {
            let prev = &path[i - 1];
            let prev_row = prev.row();
            let prev_col = prev.column();
            let curr_row = square.row();
            let curr_col = square.column();

            // Turkish Checkers: moves must be horizontal or vertical
            if prev_row != curr_row && prev_col != curr_col {
                return Err(format!(
                    "Invalid move {}→{}: not on same row or column (diagonal moves not allowed)",
                    prev, square
                ));
            }
        }
    }

    Ok(())
}

#[test]
fn test_impossible_capture_path_bug() {
    // Board state where the bug occurs
    let white: u64 = 288230376571242752;
    let black: u64 = 23104780763204;
    let kings: u64 = 288230376151711812;

    /*
    Board visualization:
        A B C D E F G H
      +-----------------+
    8 | . . W . . . . . | 8   (W = White King at C8)
    7 | . . . . . . . . | 7
    6 | b . b . b . . . | 6   (b = black pawn)
    5 | b b . . . . . . | 5
    4 | w . . w w . . b | 4   (w = white pawn)
    3 | w . . . . . b . | 3
    2 | w . . w . . . w | 2
    1 | . . B . . . B . | 1   (B = Black King)
      +-----------------+
        A B C D E F G H
    */

    let state = State::new([white, black], kings);
    let board = Board::new(Team::White, state);
    let game = Game::from_board(board);
    let actions = game.actions();

    println!("Legal moves ({} total):", actions.len());

    let mut found_bug = false;

    for action in &actions {
        let detailed = action.to_detailed(Team::White, &board.state);
        let path = detailed.path();
        let notation = detailed.to_notation();

        let path_strs: Vec<_> = path.iter().map(|s| s.to_string()).collect();
        println!("  Move: {} (path: {:?})", notation, path_strs);

        if let Err(e) = validate_path(path) {
            println!("    *** BUG: {} ***", e);
            found_bug = true;
        }
    }

    assert!(
        !found_bug,
        "Found geometrically impossible moves - see output above"
    );
}

#[test]
fn test_a5_to_h1_impossible() {
    // Direct test: verify A5→H1 is NOT on same row or column
    let a5 = Square::A5;
    let h1 = Square::H1;

    let a5_row = a5.row(); // 4 (0-indexed)
    let a5_col = a5.column(); // 0
    let h1_row = h1.row(); // 0
    let h1_col = h1.column(); // 7

    // A5 and H1 are NOT on same row and NOT on same column
    assert_ne!(a5_row, h1_row, "A5 and H1 should NOT be on same row");
    assert_ne!(a5_col, h1_col, "A5 and H1 should NOT be on same column");

    // Therefore, a direct move A5→H1 is impossible in Turkish Checkers
    println!("A5 is at row={}, col={}", a5_row, a5_col);
    println!("H1 is at row={}, col={}", h1_row, h1_col);
    println!(
        "A5→H1 requires moving {} rows and {} columns",
        (h1_row as i8 - a5_row as i8).abs(),
        (h1_col as i8 - a5_col as i8).abs()
    );
}

// =============================================================================
// Comprehensive path reconstruction tests
// =============================================================================

/// Test that ALL actions from ANY position can be converted to valid detailed paths.
/// This is a comprehensive "fuzz-like" test that validates the path reconstruction
/// algorithm works for all generated actions.
#[test]
fn all_actions_have_valid_paths() {
    // Test the default starting position
    let game = Game::default();
    validate_all_actions_have_valid_paths(&game);
}

/// Test path reconstruction with a complex mid-game position.
#[test]
fn midgame_position_all_actions_valid() {
    // A complex mid-game position with kings and pawns
    let white = Square::A2.to_mask() | Square::D4.to_mask() | Square::H8.to_mask();
    let black =
        Square::B3.to_mask() | Square::C4.to_mask() | Square::E5.to_mask() | Square::F6.to_mask();
    let kings = Square::H8.to_mask(); // White king at H8

    let state = State::new([white, black], kings);
    let board = Board::new(Team::White, state);
    let game = Game::from_board(board);
    validate_all_actions_have_valid_paths(&game);
}

/// Test king with multiple capture opportunities requiring specific landing choices.
/// This tests the scenario where the greedy algorithm would fail.
#[test]
fn king_multi_capture_requires_specific_landing() {
    // White king at A1, black pieces arranged so that only specific landing
    // squares lead to a complete capture sequence
    //
    //     A B C D E F G H
    //   +-----------------+
    // 8 | . . . . . . . . |
    // 7 | . . . . . . . . |
    // 6 | . . . . . . . . |
    // 5 | . . . b . . . . |  b = black pawn at D5
    // 4 | . . . . . . . . |
    // 3 | . b . . . . . . |  b = black pawn at B3
    // 2 | . . . . . . . . |
    // 1 | W . . . . . . . |  W = white king at A1
    //   +-----------------+
    //
    // King at A1 can capture B3, but must land at C3 (not D3, E3, etc.)
    // to then be able to capture D5 and continue.

    let white = Square::A1.to_mask();
    let black = Square::B3.to_mask() | Square::D5.to_mask();
    let kings = Square::A1.to_mask();

    let state = State::new([white, black], kings);
    let board = Board::new(Team::White, state);
    let game = Game::from_board(board);

    validate_all_actions_have_valid_paths(&game);
}

/// Test king captures along board edges where landing options are limited.
#[test]
fn king_edge_captures() {
    // White king on edge, capturing along the edge
    //
    //     A B C D E F G H
    //   +-----------------+
    // 8 | W . b . b . b . |  W=king, b=black pawns
    // 7 | . . . . . . . . |
    // ...

    let white = Square::A8.to_mask();
    let black = Square::C8.to_mask() | Square::E8.to_mask() | Square::G8.to_mask();
    let kings = Square::A8.to_mask();

    let state = State::new([white, black], kings);
    let board = Board::new(Team::White, state);
    let game = Game::from_board(board);

    validate_all_actions_have_valid_paths(&game);
}

/// Test king captures in corner positions.
#[test]
fn king_corner_captures() {
    // King in corner with captures going both directions
    let white = Square::A1.to_mask();
    let black = Square::A3.to_mask() | Square::C1.to_mask();
    let kings = Square::A1.to_mask();

    let state = State::new([white, black], kings);
    let board = Board::new(Team::White, state);
    let game = Game::from_board(board);

    validate_all_actions_have_valid_paths(&game);
}

/// Test a long king capture chain (5+ captures).
#[test]
fn king_long_capture_chain() {
    // Arrange pieces for a long capture chain
    //
    //     A B C D E F G H
    //   +-----------------+
    // 8 | W . . . . . . . |  W = white king
    // 7 | . . . . . . . . |
    // 6 | b . . . . . b . |  b = black pawns
    // 5 | . . . . . . . . |
    // 4 | b . . . . . b . |
    // 3 | . . . . . . . . |
    // 2 | b . . . . . . . |
    // 1 | . . . . . . . . |
    //   +-----------------+

    let white = Square::A8.to_mask();
    let black = Square::A6.to_mask()
        | Square::A4.to_mask()
        | Square::A2.to_mask()
        | Square::G6.to_mask()
        | Square::G4.to_mask();
    let kings = Square::A8.to_mask();

    let state = State::new([white, black], kings);
    let board = Board::new(Team::White, state);
    let game = Game::from_board(board);

    validate_all_actions_have_valid_paths(&game);
}

/// Test black king multi-captures (ensure team handling is correct).
#[test]
fn black_king_multi_capture() {
    let white = Square::B3.to_mask() | Square::D5.to_mask();
    let black = Square::A1.to_mask();
    let kings = Square::A1.to_mask();

    let state = State::new([white, black], kings);
    let board = Board::new(Team::Black, state);
    let game = Game::from_board(board);

    validate_all_actions_have_valid_paths(&game);
}

/// Test pawn multi-captures (pawns have fixed landing squares, simpler case).
#[test]
fn pawn_multi_capture_path() {
    // White pawn at A3, black pieces at B3 and D3
    // Pawn captures: A3 -> C3 (over B3) -> E3 (over D3)
    let white = Square::A3.to_mask();
    let black = Square::B3.to_mask() | Square::D3.to_mask();
    let kings = 0;

    let state = State::new([white, black], kings);
    let board = Board::new(Team::White, state);
    let game = Game::from_board(board);

    validate_all_actions_have_valid_paths(&game);
}

/// Test mixed scenario: some pawns, some kings.
#[test]
fn mixed_pieces_captures() {
    let white = Square::A1.to_mask() | Square::A3.to_mask(); // King at A1, pawn at A3
    let black = Square::B3.to_mask() | Square::C3.to_mask() | Square::A5.to_mask();
    let kings = Square::A1.to_mask();

    let state = State::new([white, black], kings);
    let board = Board::new(Team::White, state);
    let game = Game::from_board(board);

    validate_all_actions_have_valid_paths(&game);
}

/// Stress test: Generate many random-ish positions and validate all paths.
#[test]
fn stress_test_path_reconstruction() {
    // Test multiple positions derived from playing out games
    let mut game = Game::default();

    // Play some moves and validate at each step
    for _ in 0..20 {
        validate_all_actions_have_valid_paths(&game);

        let actions = game.actions();
        if actions.is_empty() {
            break;
        }

        // Pick first action and apply
        game.make_move(&actions[0]);
    }
}

/// Test the specific pattern that caused the original bug:
/// King needs to make a specific landing choice to reach all captures.
#[test]
fn original_bug_pattern_variations() {
    // Variation 1: Similar to original but with different piece arrangement
    let test_cases = [
        // (white_pieces, black_pieces, kings, turn)
        (
            Square::C8.to_mask(),
            Square::C6.to_mask()
                | Square::C4.to_mask()
                | Square::A4.to_mask()
                | Square::A2.to_mask(),
            Square::C8.to_mask(),
            Team::White,
        ),
        (
            Square::H1.to_mask(),
            Square::H3.to_mask() | Square::F3.to_mask() | Square::D3.to_mask(),
            Square::H1.to_mask(),
            Team::White,
        ),
        (
            Square::A1.to_mask(),
            Square::A3.to_mask()
                | Square::A5.to_mask()
                | Square::C5.to_mask()
                | Square::E5.to_mask(),
            Square::A1.to_mask(),
            Team::White,
        ),
    ];

    for (white, black, kings, turn) in test_cases {
        let state = State::new([white, black], kings);
        let board = Board::new(turn, state);
        let game = Game::from_board(board);
        validate_all_actions_have_valid_paths(&game);
    }
}

// =============================================================================
// Helper functions
// =============================================================================

fn validate_all_actions_have_valid_paths(game: &Game) {
    let board = game.board();
    let actions = game.actions();

    for action in &actions {
        let detailed = action.to_detailed(board.turn, &board.state);
        let path = detailed.path();

        // Validate path is not empty
        assert!(!path.is_empty(), "Path should not be empty");

        // Validate path geometry
        if let Err(e) = validate_path(path) {
            panic!(
                "Invalid path for action {}: {}\nPath: {:?}",
                detailed.to_notation(),
                e,
                path.iter().map(|s| s.to_string()).collect::<Vec<_>>()
            );
        }
    }
}
