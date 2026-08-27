//! The one way this crate can refuse to compute.

use thiserror::Error;

/// A refusal to derive an epoch record.
///
/// Deliberately narrow. Almost nothing here can fail: every arithmetic path saturates or is
/// proved in range, and an empty network is neutral rather than erroneous. What remains is the
/// one input a caller can get wrong in a way that would silently produce a requirement no other
/// node computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CollateralError {
    /// A census was applied to a record it does not immediately follow.
    ///
    /// The recurrence is defined only over consecutive epochs; skipping one would derive a
    /// multiplier from signals that were never compared against the right predecessor.
    #[error("census is for epoch {found}, but epoch {expected} must be derived next")]
    NonSequentialEpoch {
        /// The epoch that must be derived next.
        expected: u64,
        /// The epoch the supplied census actually describes.
        found: u64,
    },
}
