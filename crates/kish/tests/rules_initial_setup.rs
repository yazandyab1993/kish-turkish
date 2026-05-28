//! Section 4: Initial Setup tests
//!
//! Tests for starting position, piece placement, and game initialization.

use kish::{Board, Square, Team};

/// Rule 4: White pieces on rows 2 and 3
#[test]
fn white_pieces_on_rows_2_and_3() {
    let board = Board::new_default();
    let white_pieces = board.friendly_pieces(); // White moves first, so friendly = white

    // Check row 2 (indices 8-15)
    for col in 0..8 {
        let sq = Square::try_from_u8(8 + col).unwrap();
        assert_ne!(
            white_pieces & sq.to_mask(),
            0,
            "White piece should be at {sq:?}"
        );
    }

    // Check row 3 (indices 16-23)
    for col in 0..8 {
        let sq = Square::try_from_u8(16 + col).unwrap();
        assert_ne!(
            white_pieces & sq.to_mask(),
            0,
            "White piece should be at {sq:?}"
        );
    }
}

/// Rule 4: Black pieces on rows 6 and 7
#[test]
fn black_pieces_on_rows_6_and_7() {
    let board = Board::new_default();
    let black_pieces = board.hostile_pieces();

    // Check row 6 (indices 40-47)
    for col in 0..8 {
        let sq = Square::try_from_u8(40 + col).unwrap();
        assert_ne!(
            black_pieces & sq.to_mask(),
            0,
            "Black piece should be at {sq:?}"
        );
    }

    // Check row 7 (indices 48-55)
    for col in 0..8 {
        let sq = Square::try_from_u8(48 + col).unwrap();
        assert_ne!(
            black_pieces & sq.to_mask(),
            0,
            "Black piece should be at {sq:?}"
        );
    }
}

/// Rule 4: Back rows (1 and 8) initially empty
#[test]
fn back_rows_initially_empty() {
    let board = Board::new_default();
    let all_pieces = board.friendly_pieces() | board.hostile_pieces();

    // Row 1 (indices 0-7) should be empty
    for col in 0..8 {
        let sq = Square::try_from_u8(col).unwrap();
        assert_eq!(
            all_pieces & sq.to_mask(),
            0,
            "Row 1 should be empty at {sq:?}"
        );
    }

    // Row 8 (indices 56-63) should be empty
    for col in 0..8 {
        let sq = Square::try_from_u8(56 + col).unwrap();
        assert_eq!(
            all_pieces & sq.to_mask(),
            0,
            "Row 8 should be empty at {sq:?}"
        );
    }
}

/// Rule 4: White moves first
#[test]
fn white_moves_first() {
    let board = Board::new_default();
    assert_eq!(board.turn, Team::White, "White should move first");
}

/// Rule 4: No kings at game start
#[test]
fn no_kings_at_start() {
    let board = Board::new_default();
    assert_eq!(board.state.kings, 0, "No kings should exist at game start");
}

/// Rule 4: Rows 4 and 5 are empty at start
#[test]
fn middle_rows_empty_at_start() {
    let board = Board::new_default();
    let all_pieces = board.friendly_pieces() | board.hostile_pieces();

    // Row 4 (indices 24-31)
    for col in 0..8 {
        let sq = Square::try_from_u8(24 + col).unwrap();
        assert_eq!(
            all_pieces & sq.to_mask(),
            0,
            "Row 4 should be empty at {sq:?}"
        );
    }

    // Row 5 (indices 32-39)
    for col in 0..8 {
        let sq = Square::try_from_u8(32 + col).unwrap();
        assert_eq!(
            all_pieces & sq.to_mask(),
            0,
            "Row 5 should be empty at {sq:?}"
        );
    }
}
