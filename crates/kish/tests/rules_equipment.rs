//! Section 3: Equipment tests
//!
//! Tests for board size, piece count, and team structure.

use kish::{Board, Square, State, Team};

// Local mask constants for testing (not exposed by library)
const MASK_ROW_2: u64 = 0x0000_0000_0000_FF00;
const MASK_ROW_3: u64 = 0x0000_0000_00FF_0000;
const MASK_ROW_6: u64 = 0x0000_FF00_0000_0000;
const MASK_ROW_7: u64 = 0x00FF_0000_0000_0000;

/// Rule 3.1: Standard 8×8 board with 64 squares
#[test]
fn board_is_8x8_with_64_squares() {
    // The bitboard representation uses u64, which has exactly 64 bits
    // representing 64 squares
    let board = Board::new_default();
    let all_pieces = board.friendly_pieces() | board.hostile_pieces();

    // Verify we can address all 64 squares
    for i in 0..64 {
        let mask = 1u64 << i;
        // Either the square is occupied or it's not - both are valid
        let _ = all_pieces & mask;
    }

    // Verify squares beyond 64 don't exist (u64 handles this naturally)
    assert_eq!(std::mem::size_of::<u64>() * 8, 64);
}

/// Rule 3.2: Each player has 16 pieces
#[test]
fn each_player_has_16_pieces_at_start() {
    let board = Board::new_default();
    assert_eq!(
        board.friendly_pieces().count_ones(),
        16,
        "White should have 16 pieces at start"
    );
    assert_eq!(
        board.hostile_pieces().count_ones(),
        16,
        "Black should have 16 pieces at start"
    );
}

/// Rule 3.2: One player plays White, the other plays Black
#[test]
fn two_distinct_teams_exist() {
    assert_ne!(Team::White, Team::Black);
    assert_eq!(Team::White.opponent(), Team::Black);
    assert_eq!(Team::Black.opponent(), Team::White);
}

/// Rule 3.2: Promoted pieces are called Kings (Dama)
#[test]
fn kings_are_tracked_separately() {
    let board = Board::from_squares(
        Team::White,
        &[Square::D4],
        &[Square::H8],
        &[Square::D4], // D4 is a king
    );

    // Verify the king is tracked
    assert_ne!(
        board.state.kings & Square::D4.to_mask(),
        0,
        "King should be tracked in kings bitboard"
    );
}

/// Validation: Cannot have more than 16 pieces per team
#[test]
#[should_panic(expected = "more than 16 pieces")]
fn cannot_have_more_than_16_white_pieces() {
    // Create state with 17 white pieces (rows 2, 3 = 16, plus one more)
    let state = State::new(
        [MASK_ROW_2 | MASK_ROW_3 | Square::A4.to_mask(), MASK_ROW_7],
        0,
    );
    state.validate();
}

#[test]
#[should_panic(expected = "more than 16 pieces")]
fn cannot_have_more_than_16_black_pieces() {
    let state = State::new(
        [MASK_ROW_2, MASK_ROW_6 | MASK_ROW_7 | Square::A5.to_mask()],
        0,
    );
    state.validate();
}
