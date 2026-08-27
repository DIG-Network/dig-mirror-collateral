//! The per-epoch record: the whole derivation for one epoch, inputs included.

use serde::{Deserialize, Serialize};

use crate::census::EpochCensus;
use crate::constants::MULT_BOOTSTRAP_MICROS;
use crate::controller::{signals_for, step_multiplier, Band, Signals};
use crate::error::CollateralError;
use crate::handicap::handicap_for_owners;
use crate::requirement::{base_per_store, required_per_store};
use crate::version::{version_for_epoch, ProtocolVersion, ACTIVATION_SCHEDULE};

/// The ruleset governing epoch 1, taken from the head of the activation schedule rather than
/// named again here.
///
/// Reading it from the schedule keeps [`EpochRecord::bootstrap`] infallible without asserting a
/// second copy of the answer: there is exactly one place that says which version governs the
/// genesis epoch. `tests/version.rs` pins that the schedule's first row activates at epoch 1, so
/// this and [`version_for_epoch(1)`](version_for_epoch) cannot disagree.
const GENESIS_VERSION: ProtocolVersion = ACTIVATION_SCHEDULE[0].version;

/// One epoch of the collateral recurrence, fully derived.
///
/// The record carries its own inputs and intermediates, not just its output. Two implementations
/// that disagree must be able to say *where* they disagree — a record holding only the final
/// requirement makes divergence detectable but not auditable, and floor divisions bite in enough
/// places that "somewhere in the chain" is not a usable answer.
///
/// All amounts are DIG base units: `1 DIG = 1_000` of them, and the smallest expressible amount is
/// 0.001 DIG. They are never mojos, which are XCH's base unit and nine orders of magnitude smaller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochRecord {
    /// The epoch this record describes, one-based.
    pub epoch: u64,

    /// The ruleset this epoch was computed under.
    ///
    /// Recorded rather than inferred, and it travels with the record over gossip. A disagreement
    /// about *which rules applied* is then a named mismatch between two versions, instead of an
    /// unexplained difference between two numbers.
    pub protocol_version: ProtocolVersion,

    /// The census that produced it. Empty at epoch 1, where no epoch precedes.
    pub census: EpochCensus,

    /// The three derived signals, absent at epoch 1 where there is nothing to compare against.
    pub signals: Option<Signals>,

    /// Where the saturation reading sat, absent at epoch 1 for the same reason.
    pub band: Option<Band>,

    /// The multiplier in force for this epoch.
    pub multiplier_micros: u64,

    /// The bootstrap subsidy in force for this epoch, in DIG base units.
    pub handicap_dig_base_units: u64,

    /// Equilibrium times multiplier, before the subsidy and the clamp, in DIG base units.
    pub base_price_dig_base_units: u64,

    /// What an advertisement must post to qualify for this epoch, in DIG base units.
    pub required_per_store_dig_base_units: u64,
}

impl EpochRecord {
    /// The epoch-1 record, which depends on nothing: the anchor the whole recurrence unrolls
    /// from, and the base case that makes the apparent circularity between a requirement and the
    /// census that qualifies against it well-founded by induction on the epoch number.
    #[must_use]
    pub fn bootstrap() -> Self {
        let census = EpochCensus::bootstrap();
        let multiplier_micros = MULT_BOOTSTRAP_MICROS;
        Self {
            epoch: census.epoch,
            protocol_version: GENESIS_VERSION,
            census,
            signals: None,
            band: None,
            multiplier_micros,
            handicap_dig_base_units: handicap_for_owners(census.owners),
            base_price_dig_base_units: base_per_store(multiplier_micros),
            required_per_store_dig_base_units: required_per_store(multiplier_micros, census.owners),
        }
    }

    /// Derive the next epoch from this one and its census.
    ///
    /// The ruleset is selected from the **epoch being computed**, never from what this build
    /// prefers, so every node switches at the same epoch whenever it happens to have upgraded.
    ///
    /// The new ruleset seeds from the old one's output: this record's requirement and
    /// advertisement count feed the signals even when the epoch being derived is governed by a
    /// later version. That is the boundary rule of [`crate::version::Activation`], and it is why
    /// the predecessor's own version is checked too — extending a record computed under rules this
    /// build does not have would silently substitute different arithmetic for its seed.
    ///
    /// # Errors
    ///
    /// - [`CollateralError::NonSequentialEpoch`] when the census does not describe exactly the
    ///   epoch after this record. The recurrence is defined only for consecutive epochs, and
    ///   applying it to a gap would silently produce a requirement no other node computes. The
    ///   terminal epoch `u64::MAX` has no successor and so never advances.
    /// - [`CollateralError::EpochNotGoverned`] when no row of the activation schedule covers the
    ///   epoch being derived.
    /// - [`CollateralError::UnknownProtocolVersion`] when either that epoch or this record is
    ///   governed by a ruleset this build does not implement. This refusal is deliberate and must
    ///   be propagated: computing the epoch under the newest *known* rules instead would produce a
    ///   plausible number that disagrees with the network, and since an under-collateralised coin
    ///   is simply not counted, the operator's stores would stop earning while every surface
    ///   reported success.
    pub fn advance(&self, census: EpochCensus) -> Result<Self, CollateralError> {
        // Saturating rather than `+ 1`: every field of `EpochRecord` is `pub`, so a caller can
        // hand us the terminal epoch directly. There `+ 1` panics under `debug` and wraps to 0
        // under `release` — and a wrapped 0 then *equals* a census for epoch 0, so the guard
        // below would wave through a census that follows nothing. The second condition holds
        // only at saturation, where it keeps the terminal epoch from advancing onto itself.
        let expected = self.epoch.saturating_add(1);
        if census.epoch != expected || expected == self.epoch {
            return Err(CollateralError::NonSequentialEpoch {
                expected,
                found: census.epoch,
            });
        }

        // Fail closed, before any arithmetic: the seed must be one we can reproduce, and the
        // epoch must be one we have rules for. Both refusals travel to the caller unchanged.
        self.protocol_version.implemented()?;
        let protocol_version = version_for_epoch(census.epoch)?.implemented()?;

        // The dispatch point. Exactly one ruleset is implemented, so the derivation below *is*
        // the v1 ruleset and there is no branch to take — adding a version turns this into a
        // match whose v1 arm is these same lines, unchanged forever, because the epochs v1
        // governed must stay derivable.
        //
        // A dead `if version != V1` guard here would be unreachable and so could never be shown
        // to work. `tests/version.rs::the_dispatch_covers_every_implemented_version` is the
        // tripwire instead: it fails the moment a version is added to
        // `ProtocolVersion::IMPLEMENTED`, which is the edit that would otherwise let a v2 epoch
        // be computed silently under v1 arithmetic.

        let signals = signals_for(
            &census,
            self.census.stores,
            self.required_per_store_dig_base_units,
        );
        let multiplier_micros = step_multiplier(self.multiplier_micros, signals.saturation_micros);

        Ok(Self {
            epoch: census.epoch,
            protocol_version,
            census,
            signals: Some(signals),
            band: Some(Band::of_saturation(signals.saturation_micros)),
            multiplier_micros,
            handicap_dig_base_units: handicap_for_owners(census.owners),
            base_price_dig_base_units: base_per_store(multiplier_micros),
            required_per_store_dig_base_units: required_per_store(multiplier_micros, census.owners),
        })
    }
}
