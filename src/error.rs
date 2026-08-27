//! The one way this crate can refuse to compute.

use thiserror::Error;

/// A refusal to derive an epoch record.
///
/// Deliberately narrow. Almost nothing here can fail: every arithmetic path saturates or is
/// proved in range over the whole domain its signature admits, and an empty network is neutral
/// rather than erroneous. What remains is the one input a caller can get wrong in a way that
/// would silently produce a requirement no other node computes.
///
/// Marked `#[non_exhaustive]` because the set of ways this crate may refuse is not fixed by the
/// specification and may grow. The data types are deliberately *not* marked so: their field and
/// variant sets are part of the consensus contract — section 10 requires an implementation to
/// reproduce every field of every epoch, and a census must stay constructible by the caller that
/// reads the chain — so advertising future growth there would contradict the specification rather
/// than leave room in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
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

    /// An epoch is governed by a ruleset this build does not implement.
    ///
    /// This is the fail-closed refusal of [`crate::version`], and it must be propagated rather
    /// than handled by substituting the newest known ruleset. Falling back looks like success: the
    /// node computes a plausible requirement, disagrees with the network, and — because a coin
    /// below the real requirement is simply not counted — its operator's stores stop earning while
    /// every surface reports health. A refusal is visible; a wrong answer is not.
    #[error(
        "epoch is governed by protocol version {version}, which this build does not implement"
    )]
    UnknownProtocolVersion {
        /// The version the epoch or the record is governed by.
        version: u16,
    },

    /// No row of the activation schedule covers this epoch.
    ///
    /// Epochs are one-based and the schedule's first row governs epoch 1, so in practice this is
    /// epoch 0: an epoch that does not exist rather than one whose rules are missing.
    #[error("no protocol version governs epoch {epoch}")]
    EpochNotGoverned {
        /// The epoch no ruleset was found for.
        epoch: u64,
    },
}
