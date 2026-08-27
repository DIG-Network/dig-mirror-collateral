//! The per-epoch record: the whole derivation for one epoch, inputs included.

use serde::{Deserialize, Serialize};

use crate::census::EpochCensus;
use crate::constants::MULT_BOOTSTRAP_MICROS;
use crate::controller::{signals_for, step_multiplier, Band, Signals};
use crate::error::CollateralError;
use crate::handicap::handicap_for_owners;
use crate::requirement::{base_per_store, required_per_store};

/// One epoch of the collateral recurrence, fully derived.
///
/// The record carries its own inputs and intermediates, not just its output. Two implementations
/// that disagree must be able to say *where* they disagree — a record holding only the final
/// requirement makes divergence detectable but not auditable, and floor divisions bite in enough
/// places that "somewhere in the chain" is not a usable answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochRecord {
    /// The epoch this record describes, one-based.
    pub epoch: u64,

    /// The census that produced it. Empty at epoch 1, where no epoch precedes.
    pub census: EpochCensus,

    /// The three derived signals, absent at epoch 1 where there is nothing to compare against.
    pub signals: Option<Signals>,

    /// Where the saturation reading sat, absent at epoch 1 for the same reason.
    pub band: Option<Band>,

    /// The multiplier in force for this epoch.
    pub multiplier_micros: u64,

    /// The bootstrap subsidy in force for this epoch, in DIG mojos.
    pub handicap_mojos: u64,

    /// Equilibrium times multiplier, before the subsidy and the clamp, in DIG mojos.
    pub base_mojos: u64,

    /// What an advertisement must post to qualify for this epoch, in DIG mojos.
    pub required_per_store_mojos: u64,
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
            census,
            signals: None,
            band: None,
            multiplier_micros,
            handicap_mojos: handicap_for_owners(census.owners),
            base_mojos: base_per_store(multiplier_micros),
            required_per_store_mojos: required_per_store(multiplier_micros, census.owners),
        }
    }

    /// Derive the next epoch from this one and its census.
    ///
    /// # Errors
    ///
    /// Returns [`CollateralError::NonSequentialEpoch`] when the census does not describe exactly
    /// the epoch after this record. The recurrence is defined only for consecutive epochs, and
    /// applying it to a gap would silently produce a requirement no other node computes. The
    /// terminal epoch `u64::MAX` has no successor and so never advances.
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

        let signals = signals_for(&census, self.census.stores, self.required_per_store_mojos);
        let multiplier_micros = step_multiplier(self.multiplier_micros, signals.saturation_micros);

        Ok(Self {
            epoch: census.epoch,
            census,
            signals: Some(signals),
            band: Some(Band::of_saturation(signals.saturation_micros)),
            multiplier_micros,
            handicap_mojos: handicap_for_owners(census.owners),
            base_mojos: base_per_store(multiplier_micros),
            required_per_store_mojos: required_per_store(multiplier_micros, census.owners),
        })
    }
}
