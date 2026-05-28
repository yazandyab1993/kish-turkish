//! Section 10: Notation tests
//!
//! Tests for the algebraic notation format in Turkish Draughts.
//! Based on the notation rules from docs/rules.md Section 10.

use kish::{ActionPath, Square};

// =============================================================================
// 10.1 Square Identification
// =============================================================================

/// Rule 10.1: Squares are identified by file (a-h) + rank (1-8)
#[test]
fn square_notation_format() {
    // Test corner squares
    let a1 = ActionPath::new_move(Square::A1, Square::A2, false);
    assert!(a1.to_notation().starts_with("a1"));

    let h1 = ActionPath::new_move(Square::H1, Square::H2, false);
    assert!(h1.to_notation().starts_with("h1"));

    let a8 = ActionPath::new_move(Square::A7, Square::A8, false);
    assert!(a8.to_notation().starts_with("a7"));

    let h8 = ActionPath::new_move(Square::H7, Square::H8, false);
    assert!(h8.to_notation().starts_with("h7"));
}

/// Rule 10.1: File letters are lowercase (a-h)
#[test]
fn files_are_lowercase() {
    for col in 0..8 {
        let square = Square::try_from_u8(col).unwrap(); // A1, B1, C1, ...
        let notation = ActionPath::new_move(square, Square::A2, false);
        let notation_str = notation.to_notation();
        let first_char = notation_str.chars().next().unwrap();
        assert!(first_char.is_ascii_lowercase(), "File should be lowercase");
        assert!(('a'..='h').contains(&first_char), "File should be a-h");
    }
}

/// Rule 10.1: Rank numbers are 1-8
#[test]
fn ranks_are_1_to_8() {
    for row in 0..8 {
        let square = Square::try_from_u8(row * 8).unwrap(); // A1, A2, A3, ...
        let notation = ActionPath::new_move(square, Square::H8, false);
        let notation_str = notation.to_notation();
        let second_char = notation_str.chars().nth(1).unwrap();
        assert!(second_char.is_ascii_digit(), "Rank should be a digit");
        assert!(('1'..='8').contains(&second_char), "Rank should be 1-8");
    }
}

// =============================================================================
// 10.2 Move Notation
// =============================================================================

/// Rule 10.2: Non-capturing move format: from-to
#[test]
fn non_capturing_move_format() {
    // e3-e4: White man moves forward from e3 to e4
    let notation = ActionPath::new_move(Square::E3, Square::E4, false);
    assert_eq!(notation.to_notation(), "e3-e4");
}

/// Rule 10.2: Sideways move format
#[test]
fn sideways_move_format() {
    // d4-e4: Man moves right from d4 to e4
    let notation = ActionPath::new_move(Square::D4, Square::E4, false);
    assert_eq!(notation.to_notation(), "d4-e4");
}

/// Rule 10.2: Single capture format: fromxto
#[test]
fn single_capture_format() {
    // d4xd6: Piece on d4 jumps over enemy on d5, landing on d6
    let notation = ActionPath::new_capture(Square::D4, &[Square::D6], false);
    assert_eq!(notation.to_notation(), "d4xd6");
}

/// Rule 10.2: Multi-capture format: fromxmidxto
#[test]
fn multi_capture_format() {
    // b3xd3xd5: Piece captures two enemies in sequence
    let notation = ActionPath::new_capture(Square::B3, &[Square::D3, Square::D5], false);
    assert_eq!(notation.to_notation(), "b3xd3xd5");
}

/// Rule 10.2: Promotion format: from-to=K
#[test]
fn promotion_format() {
    // c7-c8=K: White man reaches c8 and is promoted to king
    let notation = ActionPath::new_move(Square::C7, Square::C8, true);
    assert_eq!(notation.to_notation(), "c7-c8=K");
}

/// Rule 10.2: Capture with promotion format: fromxto=K
#[test]
fn capture_with_promotion_format() {
    // c6xc8=K: Man captures and promotes
    let notation = ActionPath::new_capture(Square::C6, &[Square::C8], true);
    assert_eq!(notation.to_notation(), "c6xc8=K");
}

// =============================================================================
// 10.3 Notation Examples (from rules)
// =============================================================================

/// Example 1: Simple move
#[test]
fn example_simple_move() {
    // e3-e4 - White man moves forward from e3 to e4
    let notation = ActionPath::new_move(Square::E3, Square::E4, false);
    assert_eq!(notation.to_notation(), "e3-e4");
}

/// Example 2: Sideways move
#[test]
fn example_sideways_move() {
    // d4-e4 - Man moves right from d4 to e4
    let notation = ActionPath::new_move(Square::D4, Square::E4, false);
    assert_eq!(notation.to_notation(), "d4-e4");
}

/// Example 3: Single capture
#[test]
fn example_single_capture() {
    // d4xd6 - Piece on d4 jumps over enemy on d5, landing on d6
    let notation = ActionPath::new_capture(Square::D4, &[Square::D6], false);
    assert_eq!(notation.to_notation(), "d4xd6");
}

/// Example 4: Multi-capture sequence
#[test]
fn example_multi_capture() {
    // b3xd3xd5 - Piece captures two enemies: first moving right (b3 to d3), then forward (d3 to d5)
    let notation = ActionPath::new_capture(Square::B3, &[Square::D3, Square::D5], false);
    assert_eq!(notation.to_notation(), "b3xd3xd5");
}

/// Example 5: King long-range move
#[test]
fn example_king_long_move() {
    // a1-a7 - King moves from a1 to a7 (six squares forward)
    let notation = ActionPath::new_move(Square::A1, Square::A7, false);
    assert_eq!(notation.to_notation(), "a1-a7");
}

/// Example 6: Promotion
#[test]
fn example_promotion() {
    // c7-c8=K - White man reaches c8 and is promoted to king
    let notation = ActionPath::new_move(Square::C7, Square::C8, true);
    assert_eq!(notation.to_notation(), "c7-c8=K");
}

// =============================================================================
// Additional notation tests
// =============================================================================

/// Test complex multi-capture with many jumps
#[test]
fn complex_multi_capture() {
    // Multi-capture with 4 captures
    let notation = ActionPath::new_capture(
        Square::A1,
        &[Square::A3, Square::C3, Square::C5, Square::E5],
        false,
    );
    assert_eq!(notation.to_notation(), "a1xa3xc3xc5xe5");
    assert_eq!(notation.path_len(), 5);
}

/// Test complex multi-capture ending in promotion
#[test]
fn complex_multi_capture_with_promotion() {
    // Multi-capture ending in promotion
    let notation = ActionPath::new_capture(Square::A5, &[Square::A7, Square::C7, Square::C8], true);
    assert_eq!(notation.to_notation(), "a5xa7xc7xc8=K");
}

/// Test notation accessors
#[test]
fn notation_accessors() {
    let notation = ActionPath::new_capture(Square::B3, &[Square::D3, Square::D5], false);

    assert_eq!(notation.source(), Square::B3);
    assert_eq!(notation.destination(), Square::D5);
    assert!(notation.is_capture());
    assert!(!notation.is_promotion());
    assert_eq!(notation.path(), &[Square::B3, Square::D3, Square::D5]);
}

/// Test notation accessors for promotion move
#[test]
fn notation_accessors_promotion() {
    let notation = ActionPath::new_move(Square::C7, Square::C8, true);

    assert_eq!(notation.source(), Square::C7);
    assert_eq!(notation.destination(), Square::C8);
    assert!(!notation.is_capture());
    assert!(notation.is_promotion());
    assert_eq!(notation.path_len(), 2);
}

/// Test Display trait implementation
#[test]
fn notation_display() {
    let notation = ActionPath::new_capture(Square::D4, &[Square::D6], false);
    assert_eq!(format!("{notation}"), "d4xd6");
}

/// Test `write_notation` for zero-allocation usage
#[test]
fn notation_write_to_buffer() {
    let notation = ActionPath::new_capture(Square::B3, &[Square::D3, Square::D5], false);
    let mut buf = [0u8; 52];
    let len = notation.write_notation(&mut buf);

    let result = std::str::from_utf8(&buf[..len]).unwrap();
    assert_eq!(result, "b3xd3xd5");
}

/// Test all files have correct letter
#[test]
fn all_files_correct() {
    let expected_files = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
    for (col, expected_file) in expected_files.iter().enumerate() {
        let square = Square::try_from_u8(col as u8).unwrap();
        let notation = ActionPath::new_move(square, Square::A8, false);
        let file = notation.to_notation().chars().next().unwrap();
        assert_eq!(
            file, *expected_file,
            "File {col} should be '{expected_file}'"
        );
    }
}

/// Test all ranks have correct number
#[test]
fn all_ranks_correct() {
    let expected_ranks = ['1', '2', '3', '4', '5', '6', '7', '8'];
    for (row, expected_rank) in expected_ranks.iter().enumerate() {
        let square = Square::try_from_u8((row * 8) as u8).unwrap(); // A1, A2, A3, ...
        let notation = ActionPath::new_move(square, Square::H8, false);
        let rank = notation.to_notation().chars().nth(1).unwrap();
        assert_eq!(
            rank, *expected_rank,
            "Rank {row} should be '{expected_rank}'"
        );
    }
}

/// Test notation equality
#[test]
fn notation_equality() {
    let notation1 = ActionPath::new_move(Square::E3, Square::E4, false);
    let notation2 = ActionPath::new_move(Square::E3, Square::E4, false);
    let notation3 = ActionPath::new_move(Square::E3, Square::E5, false);

    assert_eq!(notation1, notation2);
    assert_ne!(notation1, notation3);
}

/// Test notation cloning
#[test]
fn notation_clone() {
    let notation1 = ActionPath::new_capture(Square::D4, &[Square::D6], false);
    let notation2 = notation1;

    assert_eq!(notation1, notation2);
    assert_eq!(notation1.to_notation(), notation2.to_notation());
}
