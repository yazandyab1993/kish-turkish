//! Section 7: Promotion tests
//!
//! Tests for pawn promotion to king on the back row.

use kish::{Board, Square, Team};

// =============================================================================
// 7.1 Basic Promotion
// =============================================================================

/// Rule 7.1: White promotes on row 8
#[test]
fn white_pawn_promotes_on_row_8() {
    let board = Board::from_squares(Team::White, &[Square::D7], &[Square::H8], &[]);
    let actions = board.actions();

    // Find move to D8
    let promotion_action = actions
        .iter()
        .find(|a| a.delta.pieces[Team::White.to_usize()] & Square::D8.to_mask() != 0);

    assert!(promotion_action.is_some(), "Should be able to move to D8");
    let action = promotion_action.unwrap();
    assert_ne!(
        action.delta.kings & Square::D8.to_mask(),
        0,
        "Pawn should promote to king at D8"
    );
}

/// Rule 7.1: Black promotes on row 1
#[test]
fn black_pawn_promotes_on_row_1() {
    let board = Board::from_squares(Team::Black, &[Square::A8], &[Square::D2], &[]);
    let actions = board.actions();

    // Find move to D1
    let promotion_action = actions
        .iter()
        .find(|a| a.delta.pieces[Team::Black.to_usize()] & Square::D1.to_mask() != 0);

    assert!(promotion_action.is_some(), "Should be able to move to D1");
    let action = promotion_action.unwrap();
    assert_ne!(
        action.delta.kings & Square::D1.to_mask(),
        0,
        "Black pawn should promote to king at D1"
    );
}

/// Rule 7.1: Promotion happens for any column reaching back row
#[test]
fn promotion_works_for_all_columns() {
    // Test columns A, D, H for white
    for (src, dest) in [
        (Square::A7, Square::A8),
        (Square::D7, Square::D8),
        (Square::H7, Square::H8),
    ] {
        let board = Board::from_squares(Team::White, &[src], &[Square::A1], &[]);
        let actions = board.actions();

        let promotion = actions
            .iter()
            .find(|a| a.delta.pieces[Team::White.to_usize()] & dest.to_mask() != 0);

        assert!(promotion.is_some(), "Should promote at {dest:?}");
        assert_ne!(
            promotion.unwrap().delta.kings & dest.to_mask(),
            0,
            "Should become king at {dest:?}"
        );
    }
}

/// Rule 7.1: Promotion gives enhanced movement (king moves)
#[test]
fn promoted_piece_has_king_movement() {
    // Create a position where a king exists
    let board = Board::from_squares(
        Team::White,
        &[Square::D4],
        &[Square::H8],
        &[Square::D4], // D4 is a king
    );
    let actions = board.actions();

    // King should be able to move backward (pawns cannot)
    let backward_move = actions.iter().any(|a| {
        let dest = a.delta.pieces[Team::White.to_usize()] & !Square::D4.to_mask();
        dest == Square::D3.to_mask() || dest == Square::D2.to_mask() || dest == Square::D1.to_mask()
    });
    assert!(backward_move, "King should be able to move backward");
}

// =============================================================================
// 7.2 Promotion During Capture Sequences
// =============================================================================

/// Rule 7.2: Pawn promotes after capture landing on promotion row
#[test]
fn pawn_promotes_after_capture_on_promotion_row() {
    // White pawn at B6, captures B7, lands at B8
    let board = Board::from_squares(Team::White, &[Square::B6], &[Square::B7], &[]);
    let actions = board.actions();

    assert_eq!(actions.len(), 1);
    let action = &actions[0];

    // Verify capture happened
    assert_eq!(
        action.delta.pieces[Team::Black.to_usize()],
        Square::B7.to_mask()
    );

    // Verify promotion happened at B8
    assert_ne!(
        action.delta.kings & Square::B8.to_mask(),
        0,
        "Pawn should promote at B8 after capture"
    );
}

/// Rule 7.2: Pawn promotes immediately during multi-capture
/// and then continues mandatory captures as a king.
#[test]
fn pawn_promotes_immediately_and_continues_as_king() {
    // White pawn at D6 captures D7, lands at D8 and promotes immediately.
    // As a king it can then fly left over B8 and land on A8.
    let board = Board::from_squares(Team::White, &[Square::D6], &[Square::D7, Square::B8], &[]);
    let actions = board.actions();

    assert_eq!(actions.len(), 1, "Should have one maximum 2-capture action");
    let action = &actions[0];

    assert_eq!(
        action.delta.pieces[Team::Black.to_usize()],
        Square::D7.to_mask() | Square::B8.to_mask(),
        "Should capture before and after promotion"
    );

    let final_pos = action.delta.pieces[Team::White.to_usize()] & !Square::D6.to_mask();
    assert_eq!(
        final_pos,
        Square::A8.to_mask(),
        "Should end as a king on A8"
    );

    assert_ne!(
        action.delta.kings & Square::A8.to_mask(),
        0,
        "The final piece should be a king after promoting mid-capture"
    );
    assert_eq!(action.capture_count(Team::White), 2);
    assert_eq!(
        action.to_detailed(Team::White, &board.state).to_notation(),
        "d6xd8xa8=K"
    );
}

/// Rule 7.2: Sideways capture from promotion row continues after immediate promotion
#[test]
fn sideways_capture_from_promotion_row_after_immediate_promotion() {
    // White pawn at D6 captures D7 and promotes on D8.
    // It must still continue by capturing E8 horizontally as a king.
    let board = Board::from_squares(Team::White, &[Square::D6], &[Square::D7, Square::E8], &[]);
    let actions = board.actions();

    // Should capture both
    let max_captures = actions
        .iter()
        .map(|a| a.delta.pieces[Team::Black.to_usize()].count_ones())
        .max()
        .unwrap_or(0);

    assert_eq!(max_captures, 2, "Should capture both in sequence");
}

/// Rule 7.2: Immediate promotion enables king-only continuation routes
#[test]
fn promotion_continuation_can_use_king_flying_capture() {
    let board = Board::from_squares(Team::White, &[Square::D6], &[Square::D7, Square::B8], &[]);
    let actions = board.actions();
    let action = &actions[0];

    assert_eq!(
        action.delta.pieces[Team::Black.to_usize()].count_ones(),
        2,
        "The promoted king must continue and capture the distant B8 piece"
    );
    assert_eq!(
        action.delta.kings,
        Square::A8.to_mask(),
        "The final landing square, not the intermediate D8 square, remains king in the final delta"
    );
}

/// Rule 7.2: Black pawn promotion during capture
#[test]
fn black_pawn_promotes_during_capture() {
    let board = Board::from_squares(
        Team::Black,
        &[Square::D2], // White piece
        &[Square::D3], // Black pawn
        &[],
    );
    let actions = board.actions();

    // Black pawn at D3 captures D2, lands at D1 and promotes
    assert_eq!(actions.len(), 1);
    let action = &actions[0];

    assert_eq!(
        action.delta.pieces[Team::White.to_usize()],
        Square::D2.to_mask(),
        "Should capture D2"
    );
    assert_ne!(
        action.delta.kings & Square::D1.to_mask(),
        0,
        "Black should promote at D1"
    );
}

/// Rule 7.2: Black pawn also promotes immediately and continues as king
#[test]
fn black_pawn_promotes_immediately_and_continues_as_king() {
    let board = Board::from_squares(Team::Black, &[Square::D2, Square::B1], &[Square::D3], &[]);
    let actions = board.actions();

    assert_eq!(actions.len(), 1, "Should have one maximum 2-capture action");
    let action = &actions[0];

    assert_eq!(
        action.delta.pieces[Team::White.to_usize()],
        Square::D2.to_mask() | Square::B1.to_mask(),
        "Should capture before and after black promotion"
    );

    let final_pos = action.delta.pieces[Team::Black.to_usize()] & !Square::D3.to_mask();
    assert_eq!(
        final_pos,
        Square::A1.to_mask(),
        "Should end as a king on A1"
    );
    assert_ne!(action.delta.kings & Square::A1.to_mask(), 0);
    assert_eq!(action.capture_count(Team::Black), 2);
    assert_eq!(
        action.to_detailed(Team::Black, &board.state).to_notation(),
        "d3xd1xa1=K"
    );
}
