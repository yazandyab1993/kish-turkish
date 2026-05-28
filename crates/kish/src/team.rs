//! Team representation for Turkish Draughts.
//!
//! This module defines the [`Team`] enum representing the two players in Turkish Draughts:
//! White and Black. White always moves first in a standard game.
//!
//! # Representation
//!
//! Teams use `#[repr(usize)]` for efficient array indexing:
//! - `Team::White = 0`
//! - `Team::Black = 1`
//!
//! This allows direct use of teams as array indices (e.g., `pieces[team.to_usize()]`).
//!
//! # Example
//!
//! ```rust
//! use kish::Team;
//!
//! let white = Team::White;
//! let black = white.opponent();
//!
//! assert_eq!(white.to_usize(), 0);
//! assert_eq!(black.to_usize(), 1);
//! assert_eq!(black.opponent(), white);
//! ```

use std::fmt;
use std::ops::Not;

/// Represents a team (player) in the game.
#[repr(usize)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Team {
    /// The white team.
    #[default]
    White = 0,
    /// The black team.
    Black = 1,
}

impl Team {
    /// Returns the opponent team.
    #[inline]
    #[must_use]
    pub const fn opponent(&self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    /// Converts a team to its index.
    #[inline]
    #[must_use]
    pub const fn to_usize(&self) -> usize {
        *self as usize
    }

    /// Converts an index to a team.
    ///
    /// # Panics
    /// Panics in debug builds if `index >= 2`. In release builds, invalid indices
    /// cause undefined behavior.
    ///
    /// For a safe alternative, use [`Team::try_from`].
    #[inline]
    #[must_use]
    pub(crate) const fn from_usize(index: usize) -> Self {
        debug_assert!(index < 2, "index must be in the range [0, 1]");
        // SAFETY: The enum is #[repr(usize)] with variants White=0 and Black=1.
        // The debug_assert ensures index < 2, making this transmute valid.
        unsafe { std::mem::transmute(index) }
    }
}

impl From<Team> for usize {
    #[inline]
    fn from(team: Team) -> Self {
        team.to_usize()
    }
}

/// Error returned when converting an invalid index to a [`Team`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTeamIndex(pub usize);

impl fmt::Display for InvalidTeamIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid team index: {} (expected 0 or 1)", self.0)
    }
}

impl std::error::Error for InvalidTeamIndex {}

impl TryFrom<usize> for Team {
    type Error = InvalidTeamIndex;

    #[inline]
    fn try_from(index: usize) -> Result<Self, Self::Error> {
        match index {
            0 => Ok(Self::White),
            1 => Ok(Self::Black),
            _ => Err(InvalidTeamIndex(index)),
        }
    }
}

impl fmt::Display for Team {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::White => write!(f, "White"),
            Self::Black => write!(f, "Black"),
        }
    }
}

impl Not for Team {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        self.opponent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    // ==================== Core methods ====================

    #[test_case(Team::White => Team::Black ; "white")]
    #[test_case(Team::Black => Team::White ; "black")]
    fn opponent(team: Team) -> Team {
        team.opponent()
    }

    #[test_case(Team::White => 0 ; "white")]
    #[test_case(Team::Black => 1 ; "black")]
    fn to_usize(team: Team) -> usize {
        team.to_usize()
    }

    #[test_case(Team::White => 0 ; "white")]
    #[test_case(Team::Black => 1 ; "black")]
    fn into_usize(team: Team) -> usize {
        team.into()
    }

    #[test_case(0 => Team::White ; "white")]
    #[test_case(1 => Team::Black ; "black")]
    fn from_usize(index: usize) -> Team {
        Team::from_usize(index)
    }

    #[test_case(2 => panics "index must be in the range [0, 1]" ; "two")]
    #[test_case(usize::MAX => panics "index must be in the range [0, 1]" ; "max")]
    fn from_usize_panics(index: usize) {
        let _ = Team::from_usize(index);
    }

    #[test_case(0 => Ok(Team::White) ; "white")]
    #[test_case(1 => Ok(Team::Black) ; "black")]
    #[test_case(2 => Err(InvalidTeamIndex(2)) ; "two")]
    #[test_case(usize::MAX => Err(InvalidTeamIndex(usize::MAX)) ; "max")]
    fn try_from(index: usize) -> Result<Team, InvalidTeamIndex> {
        Team::try_from(index)
    }

    // ==================== Derived traits ====================

    #[test]
    fn default() {
        assert_eq!(Team::default(), Team::White);
    }

    #[test_case(Team::White ; "white")]
    #[test_case(Team::Black ; "black")]
    fn clone(team: Team) {
        assert_eq!(team.clone(), team);
    }

    #[test_case(Team::White, Team::White => true ; "same")]
    #[test_case(Team::White, Team::Black => false ; "different")]
    fn eq(a: Team, b: Team) -> bool {
        a == b
    }

    #[test]
    fn ord() {
        assert!(Team::White < Team::Black);
    }

    #[test]
    fn hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn compute(team: Team) -> u64 {
            let mut hasher = DefaultHasher::new();
            team.hash(&mut hasher);
            hasher.finish()
        }

        assert_eq!(compute(Team::White), compute(Team::White));
        assert_eq!(compute(Team::Black), compute(Team::Black));
        assert_ne!(compute(Team::White), compute(Team::Black));
    }

    // ==================== Format traits ====================

    #[test_case(Team::White => "White" ; "white")]
    #[test_case(Team::Black => "Black" ; "black")]
    fn display(team: Team) -> String {
        team.to_string()
    }

    #[test_case(Team::White => "White" ; "white")]
    #[test_case(Team::Black => "Black" ; "black")]
    fn debug(team: Team) -> String {
        format!("{team:?}")
    }

    // ==================== Operators ====================

    #[test_case(Team::White => Team::Black ; "white")]
    #[test_case(Team::Black => Team::White ; "black")]
    fn not(team: Team) -> Team {
        !team
    }

    // ==================== Invariants ====================

    #[test_case(Team::White ; "white")]
    #[test_case(Team::Black ; "black")]
    fn opponent_is_involution(team: Team) {
        assert_eq!(team.opponent().opponent(), team);
    }

    // ==================== InvalidTeamIndex ====================

    #[test_case(2 => "invalid team index: 2 (expected 0 or 1)" ; "two")]
    #[test_case(usize::MAX => format!("invalid team index: {} (expected 0 or 1)", usize::MAX) ; "max")]
    fn error_display(index: usize) -> String {
        InvalidTeamIndex(index).to_string()
    }

    #[test]
    fn error_debug() {
        assert_eq!(
            format!("{:?}", InvalidTeamIndex(42)),
            "InvalidTeamIndex(42)"
        );
    }

    #[test]
    fn error_source() {
        use std::error::Error;
        assert!(InvalidTeamIndex(0).source().is_none());
    }
}
