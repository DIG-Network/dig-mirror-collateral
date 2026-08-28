//! Protocol versioning: which ruleset governs which epoch, and what to do about one you do not
//! have.
//!
//! # Why a version is harder here than in most protocols
//!
//! The collateral model is a **recurrence**. `required_per_store(n)` consumes the epoch `n-1`
//! census, which qualified its coins against `required_per_store(n-1)`, back to epoch 1. So a rule
//! change is never a local edit: verifying the *present* means replaying from genesis, and every
//! epoch along the way must be recomputed under the rules that were in force **at that epoch**.
//!
//! Three consequences follow, and each is a rule rather than a preference.
//!
//! ## Activation is by epoch, never by node version or wall-clock
//!
//! [`version_for_epoch`] dispatches on the epoch **being computed**, and on nothing else. It does
//! not read a clock, a config, or what this binary happens to be. If a node switched rulesets when
//! *it* upgraded, then during any rollout window an upgraded and an un-upgraded node would compute
//! different requirements for the same epoch — which is precisely the fork the whole design exists
//! to prevent. Every node switches at the same epoch, whenever it installs the code.
//!
//! ## Every historical ruleset stays in this crate forever
//!
//! Old versions are not deprecated and are never deleted. A node that dropped v1 when v2 activated
//! could no longer derive its own current state, because the replay from genesis would have no
//! rules for the early epochs. They are permanent, exactly like the historical consensus rules in
//! a chain client. The golden vectors of a retired version keep running for the same reason: they
//! are the regression test that a later refactor did not quietly rewrite history.
//!
//! ## An unknown version fails closed
//!
//! A node reaching an epoch governed by a version it does not implement MUST stop and say so. It
//! MUST NOT fall back to its newest known ruleset and MUST NOT extrapolate.
//!
//! Falling back is the dangerous option *precisely because it appears to work*. The node computes
//! a plausible number, silently disagrees with the network, and — since a mirror coin below the
//! real requirement is simply not counted — its stores stop earning while every surface reports
//! success. A refusal is visible; a wrong answer is not.

use serde::{Deserialize, Serialize};

use crate::error::CollateralError;

/// The identifier of a collateral ruleset.
///
/// A transparent newtype over `u16` rather than an enum, and that is deliberate: an **unknown**
/// version must be *representable*. A record or a gossiped claim tagged with a future version has
/// to deserialise, so that a node can name the version it does not implement and refuse for a
/// stated reason. An enum would fail at the parse instead, turning a precise refusal into
/// "malformed input".
///
/// Representability is not implementation. [`ProtocolVersion::implemented`] is the gate, and every
/// path that derives an epoch goes through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolVersion(pub u16);

impl ProtocolVersion {
    /// Version 1: the ruleset described by every other module in this crate, and the one that
    /// governs epoch 1 onward.
    pub const V1: Self = Self(1);

    /// Every ruleset this build implements, ascending.
    ///
    /// This list only ever grows. Removing an entry makes historical epochs underivable, which is
    /// a strictly worse failure than not having upgraded at all.
    pub const IMPLEMENTED: &'static [Self] = &[Self::V1];

    /// Whether this build carries the rules for this version.
    #[must_use]
    pub fn is_implemented(self) -> bool {
        Self::IMPLEMENTED.contains(&self)
    }

    /// This version, if this build implements it.
    ///
    /// # Errors
    ///
    /// Returns [`CollateralError::UnknownProtocolVersion`] otherwise. This is the fail-closed gate
    /// described in the module documentation: the caller must propagate the refusal, never
    /// substitute a ruleset it does have.
    pub fn implemented(self) -> Result<Self, CollateralError> {
        if self.is_implemented() {
            Ok(self)
        } else {
            Err(CollateralError::UnknownProtocolVersion { version: self.0 })
        }
    }
}

/// One row of the activation schedule: a ruleset, and the first epoch it governs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activation {
    /// The ruleset that takes effect.
    pub version: ProtocolVersion,

    /// The **first epoch computed under** `version`.
    ///
    /// The boundary semantics are stated once, here, because this is where an off-by-one would
    /// live: the new rules apply *at* this epoch, not from the epoch after it.
    ///
    /// The recurrence therefore **crosses** the boundary. `required_per_store(first_epoch)` is
    /// computed under `version`, but it consumes `required_per_store(first_epoch - 1)`, which was
    /// computed under the preceding version. A new ruleset takes the old ruleset's output as its
    /// seed and never recomputes it.
    pub first_epoch: u64,
}

/// The activation schedule: which ruleset governs which epoch, for all time.
///
/// Ordered by `first_epoch`, strictly ascending, with strictly ascending versions. That ordering is
/// a **precondition of [`version_for_epoch_in`]**, not a stylistic preference, and it is enforced at
/// compile time by [`schedule_is_strictly_ascending`] below: an unordered row would make the reverse
/// scan return the wrong ruleset for a range of epochs, which is a fork.
///
/// Epochs are one-based, so the first row governs epoch 1 and there is no ungoverned epoch above
/// it. A future ruleset is added as a row here with an activation epoch far enough ahead that every
/// operator can upgrade before it arrives: the epoch is a deadline for the whole network, epochs
/// are seven days, and a short lead time is not recoverable.
pub const ACTIVATION_SCHEDULE: &[Activation] = &[Activation {
    version: ProtocolVersion::V1,
    first_epoch: 1,
}];

/// Whether `schedule` ascends strictly in both activation epoch and version.
///
/// This is the precondition [`version_for_epoch_in`] relies on, written as a `const fn` so that the
/// schedule this build ships can be checked *before the build exists* rather than by a test that
/// happens to run. The distinction matters: with one row today, any `windows(2)` assertion over the
/// real schedule is vacuous, so the person who adds version 2 is the person who would first trip an
/// unchecked precondition — and what an unordered row produces is not an error but a silently wrong
/// ruleset for a range of epochs.
///
/// Strictness is deliberate on both fields. Equal `first_epoch` rows would resolve last-writer-wins
/// with no complaint, and a version that did not ascend with its activation epoch would mean a
/// later epoch was governed by an earlier ruleset.
#[must_use]
pub const fn schedule_is_strictly_ascending(schedule: &[Activation]) -> bool {
    let mut i = 1;
    while i < schedule.len() {
        let previous = schedule[i - 1];
        let current = schedule[i];
        if previous.first_epoch >= current.first_epoch || previous.version.0 >= current.version.0 {
            return false;
        }
        i += 1;
    }
    true
}

// The precondition of `version_for_epoch_in`, evaluated by the compiler against the schedule this
// build actually ships. It lives in `src` rather than in a test so that `cargo build` and
// `cargo package` evaluate it too — a schedule that cannot be published is stronger than one that
// merely fails a suite someone has to run.
const _: () = assert!(
    schedule_is_strictly_ascending(ACTIVATION_SCHEDULE),
    "ACTIVATION_SCHEDULE rows must ascend strictly in both first_epoch and version"
);

// Epochs are one-based, so the first row must govern epoch 1: an ungoverned epoch below the first
// activation is a refusal for a real epoch, not for the non-existent epoch 0.
const _: () = assert!(
    !ACTIVATION_SCHEDULE.is_empty() && ACTIVATION_SCHEDULE[0].first_epoch == 1,
    "ACTIVATION_SCHEDULE must be non-empty and its first row must govern epoch 1"
);

/// The ruleset that governs `epoch`, under the schedule this build carries.
///
/// # Errors
///
/// Returns [`CollateralError::EpochNotGoverned`] when no row of the schedule covers `epoch` — in
/// practice only epoch 0, which does not exist in a one-based numbering.
///
/// Note what this does **not** do: it never reports the version this build prefers, and it never
/// falls back. A version it returns may still be one this build cannot execute; that is
/// [`ProtocolVersion::implemented`]'s question, and separating the two is what lets a node say
/// *"epoch 900 is governed by v2 and I only implement v1"* rather than *"something went wrong"*.
pub fn version_for_epoch(epoch: u64) -> Result<ProtocolVersion, CollateralError> {
    version_for_epoch_in(ACTIVATION_SCHEDULE, epoch)
}

/// [`version_for_epoch`] against an explicit schedule.
///
/// Exposed so that boundary behaviour is testable against a schedule with more than one row while
/// only one ruleset exists. A caller in production passes [`ACTIVATION_SCHEDULE`].
///
/// # Errors
///
/// Returns [`CollateralError::EpochNotGoverned`] when `schedule` has no row with
/// `first_epoch <= epoch`, including when the schedule is empty.
pub fn version_for_epoch_in(
    schedule: &[Activation],
    epoch: u64,
) -> Result<ProtocolVersion, CollateralError> {
    // The last row that has already activated. `<=` is the boundary rule of `Activation`: the
    // activation epoch is governed by the new version, not by the one it replaces.
    schedule
        .iter()
        .rev()
        .find(|activation| activation.first_epoch <= epoch)
        .map(|activation| activation.version)
        .ok_or(CollateralError::EpochNotGoverned { epoch })
}
