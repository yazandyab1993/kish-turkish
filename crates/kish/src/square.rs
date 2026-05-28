//! Square representation for the 8×8 Turkish Draughts board.
//!
//! This module defines the [`Square`] enum representing all 64 squares on the board,
//! using standard algebraic notation (a1-h8).
//!
//! # Board Layout
//!
//! ```text
//!     a   b   c   d   e   f   g   h
//!   +---+---+---+---+---+---+---+---+
//! 8 |A8 |B8 |C8 |D8 |E8 |F8 |G8 |H8 |  ← White promotes here
//!   +---+---+---+---+---+---+---+---+
//! 7 |A7 |B7 |C7 |D7 |E7 |F7 |G7 |H7 |
//!   +---+---+---+---+---+---+---+---+
//! ...
//!   +---+---+---+---+---+---+---+---+
//! 1 |A1 |B1 |C1 |D1 |E1 |F1 |G1 |H1 |  ← Black promotes here
//!   +---+---+---+---+---+---+---+---+
//! ```
//!
//! # Representation
//!
//! Squares are stored as `#[repr(u8)]` with values 0-63:
//! - **Index formula**: `row * 8 + column` where row and column are 0-indexed
//! - **Row 0** = Rank 1, **Row 7** = Rank 8
//! - **Column 0** = File A, **Column 7** = File H
//!
//! # Bitboard Conversion
//!
//! Each square can be efficiently converted to a bitmask for bitboard operations:
//!
//! ```rust
//! use kish::Square;
//!
//! let d4 = Square::D4;
//! let mask = d4.to_mask();  // 1u64 << 27
//! // SAFETY: mask is from a valid square, so it has exactly one bit set.
//! assert_eq!(unsafe { Square::from_mask(mask) }, d4);
//! ```
//!
//! # Parsing
//!
//! Squares can be parsed from algebraic notation (case-insensitive):
//!
//! ```rust
//! use kish::Square;
//!
//! let square: Square = "d4".parse().unwrap();
//! assert_eq!(square, Square::D4);
//!
//! let square: Square = "H8".parse().unwrap();
//! assert_eq!(square, Square::H8);
//! ```

use std::fmt;
use std::str::FromStr;

/// Represents a single square on the 8×8 board.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Square {
    A1 = 0,
    B1 = 1,
    C1 = 2,
    D1 = 3,
    E1 = 4,
    F1 = 5,
    G1 = 6,
    H1 = 7,
    A2 = 8,
    B2 = 9,
    C2 = 10,
    D2 = 11,
    E2 = 12,
    F2 = 13,
    G2 = 14,
    H2 = 15,
    A3 = 16,
    B3 = 17,
    C3 = 18,
    D3 = 19,
    E3 = 20,
    F3 = 21,
    G3 = 22,
    H3 = 23,
    A4 = 24,
    B4 = 25,
    C4 = 26,
    D4 = 27,
    E4 = 28,
    F4 = 29,
    G4 = 30,
    H4 = 31,
    A5 = 32,
    B5 = 33,
    C5 = 34,
    D5 = 35,
    E5 = 36,
    F5 = 37,
    G5 = 38,
    H5 = 39,
    A6 = 40,
    B6 = 41,
    C6 = 42,
    D6 = 43,
    E6 = 44,
    F6 = 45,
    G6 = 46,
    H6 = 47,
    A7 = 48,
    B7 = 49,
    C7 = 50,
    D7 = 51,
    E7 = 52,
    F7 = 53,
    G7 = 54,
    H7 = 55,
    A8 = 56,
    B8 = 57,
    C8 = 58,
    D8 = 59,
    E8 = 60,
    F8 = 61,
    G8 = 62,
    H8 = 63,
}

impl Square {
    /// Converts a square to its mask.
    #[inline]
    #[must_use]
    pub const fn to_mask(&self) -> u64 {
        1u64 << *self as u8
    }

    /// Converts a mask to a square.
    ///
    /// # Safety
    /// The mask must have exactly one bit set in positions 0-63.
    /// Passing zero, multiple bits, or a bit >= 64 causes undefined behavior.
    #[inline]
    #[must_use]
    pub const unsafe fn from_mask(mask: u64) -> Self {
        debug_assert!(mask.is_power_of_two(), "mask must have exactly one bit set");
        // SAFETY: trailing_zeros of a u64 returns 0-64, and caller guarantees it's a power of two
        // (exactly one bit set), so result is 0-63 which maps to valid Square variants.
        // The enum is #[repr(u8)] with variants 0-63, making this transmute valid.
        std::mem::transmute(mask.trailing_zeros() as u8)
    }

    /// Converts a square to its index.
    #[inline]
    #[must_use]
    pub const fn to_u8(&self) -> u8 {
        *self as u8
    }

    /// Converts an index to a square.
    ///
    /// # Safety
    /// The index must be in the range 0..64.
    /// Passing an index >= 64 causes undefined behavior.
    ///
    /// Use [`try_from_u8`](Self::try_from_u8) for a safe alternative.
    #[inline]
    #[must_use]
    pub const unsafe fn from_u8(index: u8) -> Self {
        debug_assert!(index < 64, "index must be in the range [0, 63]");
        // SAFETY: The enum is #[repr(u8)] with variants 0-63.
        // Caller guarantees index < 64, making this transmute valid.
        std::mem::transmute(index)
    }

    /// Converts a square to its array index.
    #[inline]
    #[must_use]
    pub const fn to_usize(&self) -> usize {
        *self as usize
    }

    /// Converts an array index to a square.
    ///
    /// # Safety
    /// The index must be in the range 0..64.
    /// Passing an index >= 64 causes undefined behavior.
    ///
    /// Use [`try_from_usize`](Self::try_from_usize) for a safe alternative.
    #[inline]
    #[must_use]
    pub const unsafe fn from_usize(index: usize) -> Self {
        debug_assert!(index < 64, "index must be in the range [0, 63]");
        // SAFETY: The enum is #[repr(u8)] with variants 0-63.
        // Caller guarantees index < 64, making this transmute valid.
        std::mem::transmute(index as u8)
    }

    /// Returns the row of the square (0-7).
    #[inline]
    #[must_use]
    pub const fn row(&self) -> u8 {
        self.to_u8() >> 3
    }

    /// Returns the column of the square (0-7).
    #[inline]
    #[must_use]
    pub const fn column(&self) -> u8 {
        self.to_u8() & 7
    }

    /// Returns the manhattan distance between two squares.
    #[inline]
    #[must_use]
    pub const fn manhattan(&self, other: Self) -> u8 {
        u8::abs_diff(self.row(), other.row()) + u8::abs_diff(self.column(), other.column())
    }

    /// Creates a square from row and column indices.
    ///
    /// Row 0 is rank 1, row 7 is rank 8.
    /// Column 0 is file a, column 7 is file h.
    ///
    /// # Safety
    /// Both row and column must be in the range 0..8.
    /// Passing row >= 8 or column >= 8 causes undefined behavior.
    #[inline]
    #[must_use]
    pub const unsafe fn from_row_column(row: u8, column: u8) -> Self {
        debug_assert!(row < 8, "row must be in the range [0, 7]");
        debug_assert!(column < 8, "column must be in the range [0, 7]");
        // SAFETY: Caller guarantees row < 8 and column < 8, so row * 8 + column < 64.
        Self::from_u8(row * 8 + column)
    }

    /// Tries to convert an index to a square.
    ///
    /// Returns `Some(Square)` if the index is in range [0, 63], otherwise `None`.
    #[inline]
    #[must_use]
    pub const fn try_from_u8(index: u8) -> Option<Self> {
        if index < 64 {
            // SAFETY: We just checked that index < 64.
            Some(unsafe { Self::from_u8(index) })
        } else {
            None
        }
    }

    /// Tries to convert an index to a square.
    ///
    /// Returns `Some(Square)` if the index is in range [0, 63], otherwise `None`.
    #[inline]
    #[must_use]
    pub const fn try_from_usize(index: usize) -> Option<Self> {
        if index < 64 {
            // SAFETY: We just checked that index < 64.
            Some(unsafe { Self::from_usize(index) })
        } else {
            None
        }
    }
}

impl From<Square> for u8 {
    #[inline]
    fn from(square: Square) -> Self {
        square.to_u8()
    }
}

impl TryFrom<u8> for Square {
    type Error = SquareIndexError;

    #[inline]
    fn try_from(index: u8) -> Result<Self, Self::Error> {
        Self::try_from_u8(index).ok_or(SquareIndexError(index as usize))
    }
}

impl From<Square> for usize {
    #[inline]
    fn from(square: Square) -> Self {
        square.to_usize()
    }
}

impl TryFrom<usize> for Square {
    type Error = SquareIndexError;

    #[inline]
    fn try_from(index: usize) -> Result<Self, Self::Error> {
        Self::try_from_usize(index).ok_or(SquareIndexError(index))
    }
}

impl fmt::Display for Square {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let file = (b'A' + self.column()) as char;
        let rank = (b'1' + self.row()) as char;
        write!(f, "{}{}", file, rank)
    }
}

/// Error type for parsing a square from a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseSquareError {
    /// The input string has an invalid length (expected 2 characters).
    Length,
    /// The file character is invalid (expected A-H or a-h).
    File,
    /// The rank character is invalid (expected 1-8).
    Rank,
}

/// Error type for converting an index to a square.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SquareIndexError(pub usize);

impl fmt::Display for ParseSquareError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Length => write!(f, "invalid length, expected 2 characters"),
            Self::File => write!(f, "invalid file, expected A-H or a-h"),
            Self::Rank => write!(f, "invalid rank, expected 1-8"),
        }
    }
}

impl std::error::Error for ParseSquareError {}

impl fmt::Display for SquareIndexError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid square index {}, expected 0-63", self.0)
    }
}

impl std::error::Error for SquareIndexError {}

impl FromStr for Square {
    type Err = ParseSquareError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 2 {
            return Err(ParseSquareError::Length);
        }

        let bytes = s.as_bytes();
        let file_char = bytes[0];
        let rank_char = bytes[1];

        let column = match file_char {
            b'A'..=b'H' => file_char - b'A',
            b'a'..=b'h' => file_char - b'a',
            _ => return Err(ParseSquareError::File),
        };

        let row = match rank_char {
            b'1'..=b'8' => rank_char - b'1',
            _ => return Err(ParseSquareError::Rank),
        };

        // SAFETY: row is 0-7 (from '1'-'8') and column is 0-7 (from 'A'-'H' or 'a'-'h').
        Ok(unsafe { Self::from_row_column(row, column) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    // Roundtrip and conversion tests
    #[test]
    fn u8_roundtrip() {
        for i in 0u8..64 {
            // SAFETY: i is in range 0..64.
            let sq = unsafe { Square::from_u8(i) };
            assert_eq!(sq.to_u8(), i);
            assert_eq!(u8::from(sq), i);
            assert_eq!(Square::try_from(i).unwrap(), sq);
        }
    }

    #[test]
    fn usize_roundtrip() {
        for i in 0usize..64 {
            // SAFETY: i is in range 0..64.
            let sq = unsafe { Square::from_usize(i) };
            assert_eq!(sq.to_usize(), i);
            assert_eq!(usize::from(sq), i);
            assert_eq!(Square::try_from(i).unwrap(), sq);
        }
    }

    #[test]
    fn try_from_u8_trait_invalid() {
        assert_eq!(Square::try_from(64u8).unwrap_err(), SquareIndexError(64));
        assert_eq!(Square::try_from(100u8).unwrap_err(), SquareIndexError(100));
        assert_eq!(Square::try_from(255u8).unwrap_err(), SquareIndexError(255));
    }

    #[test]
    fn try_from_usize_trait_invalid() {
        assert_eq!(Square::try_from(64usize).unwrap_err(), SquareIndexError(64));
        assert_eq!(
            Square::try_from(100usize).unwrap_err(),
            SquareIndexError(100)
        );
    }

    #[test]
    fn mask_roundtrip() {
        for i in 0u8..64 {
            // SAFETY: i is in range 0..64.
            let sq = unsafe { Square::from_u8(i) };
            let mask = 1u64 << i;
            assert_eq!(sq.to_mask(), mask);
            // SAFETY: mask has exactly one bit set.
            assert_eq!(unsafe { Square::from_mask(mask) }, sq);
        }
    }

    #[test_case(0b00 => panics "mask must have exactly one bit set" ; "zero_bits")]
    #[test_case(0b11 => panics "mask must have exactly one bit set" ; "two_bits")]
    fn invalid_from_mask(mask: u64) {
        // SAFETY: Intentionally testing invalid input in debug mode.
        let _ = unsafe { Square::from_mask(mask) };
    }

    #[test_case(64 => panics "index must be in the range [0, 63]" ; "just_above_max")]
    #[test_case(100 => panics "index must be in the range [0, 63]" ; "above_max")]
    #[test_case(255 => panics "index must be in the range [0, 63]" ; "u8_max")]
    fn invalid_from_u8(index: u8) {
        // SAFETY: Intentionally testing invalid input in debug mode.
        let _ = unsafe { Square::from_u8(index) };
    }

    #[test_case(64 => panics "index must be in the range [0, 63]" ; "just_above_max")]
    #[test_case(100 => panics "index must be in the range [0, 63]" ; "above_max")]
    #[test_case(usize::MAX => panics "index must be in the range [0, 63]" ; "usize_max")]
    fn invalid_from_usize(index: usize) {
        // SAFETY: Intentionally testing invalid input in debug mode.
        let _ = unsafe { Square::from_usize(index) };
    }

    // Row and column tests
    #[test]
    fn row_and_column() {
        for i in 0u8..64 {
            // SAFETY: i is in range 0..64.
            let sq = unsafe { Square::from_u8(i) };
            assert_eq!(sq.row(), i >> 3);
            assert_eq!(sq.column(), i & 7);
        }
    }

    #[test]
    fn from_row_column_roundtrip() {
        for row in 0u8..8 {
            for col in 0u8..8 {
                // SAFETY: row and col are in range 0..8.
                let sq = unsafe { Square::from_row_column(row, col) };
                assert_eq!(sq.row(), row);
                assert_eq!(sq.column(), col);
                assert_eq!(sq.to_u8(), row * 8 + col);
            }
        }
    }

    #[test_case(8, 0 => panics "row must be in the range [0, 7]" ; "row_too_large")]
    #[test_case(0, 8 => panics "column must be in the range [0, 7]" ; "column_too_large")]
    #[test_case(8, 8 => panics "row must be in the range [0, 7]" ; "both_too_large")]
    fn invalid_from_row_column(row: u8, column: u8) {
        // SAFETY: Intentionally testing invalid input in debug mode.
        let _ = unsafe { Square::from_row_column(row, column) };
    }

    // Manhattan distance tests
    #[test_case(Square::D5, Square::D5, 0; "same square")]
    #[test_case(Square::D3, Square::D4, 1u8; "right")]
    #[test_case(Square::D3, Square::D6, 3u8; "far right")]
    #[test_case(Square::E6, Square::E5, 1u8; "left")]
    #[test_case(Square::H8, Square::H1, 7u8; "far left")]
    #[test_case(Square::C6, Square::C7, 1u8; "up")]
    #[test_case(Square::C3, Square::C7, 4u8; "far up")]
    #[test_case(Square::B4, Square::B3, 1u8; "down")]
    #[test_case(Square::B4, Square::B2, 2u8; "far down")]
    #[test_case(Square::B4, Square::F6, 6u8; "diagonal")]
    #[test_case(Square::A1, Square::H8, 14u8; "far diagonal")]
    #[test_case(Square::A1, Square::C2, 3u8; "L shape short")]
    #[test_case(Square::A1, Square::B4, 4u8; "L shape tall")]
    #[test_case(Square::E4, Square::H6, 5u8; "off diagonal")]
    #[test_case(Square::B2, Square::G5, 8u8; "far off diagonal")]
    fn manhattan(src_square: Square, dest_square: Square, expected: u8) {
        assert_eq!(src_square.manhattan(dest_square), expected);
    }

    // Display and FromStr tests
    #[test]
    fn display_all_squares() {
        for row in 0u8..8 {
            for col in 0u8..8 {
                // SAFETY: row and col are in range 0..8.
                let sq = unsafe { Square::from_row_column(row, col) };
                let expected_file = (b'A' + col) as char;
                let expected_rank = (b'1' + row) as char;
                let expected = format!("{}{}", expected_file, expected_rank);
                assert_eq!(format!("{}", sq), expected);
            }
        }
    }

    #[test]
    fn from_str_uppercase() {
        for row in 0u8..8 {
            for col in 0u8..8 {
                // SAFETY: row and col are in range 0..8.
                let sq = unsafe { Square::from_row_column(row, col) };
                let s = format!("{}", sq);
                assert_eq!(s.parse::<Square>().unwrap(), sq);
            }
        }
    }

    #[test]
    fn from_str_lowercase() {
        assert_eq!("a1".parse::<Square>().unwrap(), Square::A1);
        assert_eq!("h8".parse::<Square>().unwrap(), Square::H8);
        assert_eq!("d4".parse::<Square>().unwrap(), Square::D4);
        assert_eq!("e5".parse::<Square>().unwrap(), Square::E5);
    }

    #[test_case("" => ParseSquareError::Length ; "empty")]
    #[test_case("A" => ParseSquareError::Length ; "too_short")]
    #[test_case("A1B" => ParseSquareError::Length ; "too_long")]
    #[test_case("I1" => ParseSquareError::File ; "invalid_file_upper")]
    #[test_case("i1" => ParseSquareError::File ; "invalid_file_lower")]
    #[test_case("A0" => ParseSquareError::Rank ; "rank_zero")]
    #[test_case("A9" => ParseSquareError::Rank ; "rank_nine")]
    fn from_str_invalid(s: &str) -> ParseSquareError {
        s.parse::<Square>().unwrap_err()
    }

    // try_from tests
    #[test]
    fn try_from_u8_valid() {
        for i in 0u8..64 {
            // SAFETY: i is in range 0..64.
            assert_eq!(Square::try_from_u8(i), Some(unsafe { Square::from_u8(i) }));
        }
    }

    #[test]
    fn try_from_u8_invalid() {
        for i in 64u8..=255 {
            assert_eq!(Square::try_from_u8(i), None);
        }
    }

    #[test]
    fn try_from_usize_valid() {
        for i in 0usize..64 {
            // SAFETY: i is in range 0..64.
            assert_eq!(
                Square::try_from_usize(i),
                Some(unsafe { Square::from_usize(i) })
            );
        }
    }

    #[test]
    fn try_from_usize_invalid() {
        assert_eq!(Square::try_from_usize(64), None);
        assert_eq!(Square::try_from_usize(100), None);
        assert_eq!(Square::try_from_usize(usize::MAX), None);
    }

    // ParseSquareError Display test
    #[test]
    fn parse_square_error_display() {
        assert_eq!(
            format!("{}", ParseSquareError::Length),
            "invalid length, expected 2 characters"
        );
        assert_eq!(
            format!("{}", ParseSquareError::File),
            "invalid file, expected A-H or a-h"
        );
        assert_eq!(
            format!("{}", ParseSquareError::Rank),
            "invalid rank, expected 1-8"
        );
    }

    // SquareIndexError Display test
    #[test]
    fn square_index_error_display() {
        assert_eq!(
            format!("{}", SquareIndexError(64)),
            "invalid square index 64, expected 0-63"
        );
        assert_eq!(
            format!("{}", SquareIndexError(255)),
            "invalid square index 255, expected 0-63"
        );
    }
}
