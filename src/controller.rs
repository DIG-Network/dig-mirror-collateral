//! The bang-bang multiplier controller and the two signals that drive it.
//!
//! Exactly three outcomes are possible per epoch: step up, hold, step down. A proportional
//! controller would need a deviation-times-gain product — another division, another rounding
//! decision, and another place two implementations diverge. Simplicity is the security property
//! here, because the acceptance bar is two independent implementations agreeing forever.

use serde::{Deserialize, Serialize};

use crate::census::EpochCensus;
use crate::constants::{
    DEADBAND_HIGH_MICROS, DEADBAND_LOW_MICROS, DOWN_STEP_DENOM, MULT_CEILING_MICROS,
    MULT_FLOOR_MICROS, MULT_SCALE, PARTICIPATION_WEIGHT, SIGNAL_CAP_MICROS, SIGNAL_WEIGHT_TOTAL,
    UP_STEP_DENOM, VOLUME_WEIGHT,
};

/// The three signal values derived from one census, in multiplier micros.
///
/// They are retained on the epoch record rather than discarded, because divergence between two
/// implementations must be *auditable*, not merely detectable: a record that stores only its
/// output tells you that two nodes disagree, never where.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signals {
    /// Growth ratio of counted advertisements against the previous epoch.
    pub participation_micros: u64,

    /// Collateral actually locked, against what those same advertisements were required to post.
    pub volume_micros: u64,

    /// The weighted combination the controller reads.
    pub saturation_micros: u64,
}

/// Where a saturation reading sits relative to the dead band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    /// Strictly below [`DEADBAND_LOW_MICROS`]: the multiplier steps down.
    Low,
    /// Inside the dead band, inclusive of both edges: the multiplier holds.
    Inside,
    /// Strictly above [`DEADBAND_HIGH_MICROS`]: the multiplier steps up.
    High,
}

impl Band {
    /// Classify a saturation reading. Both edges of the dead band are inclusive.
    #[must_use]
    pub const fn of_saturation(saturation_micros: u64) -> Self {
        if saturation_micros > DEADBAND_HIGH_MICROS {
            Self::High
        } else if saturation_micros < DEADBAND_LOW_MICROS {
            Self::Low
        } else {
            Self::Inside
        }
    }
}

/// The primary signal: how the count of counted advertisements grew against the previous epoch.
///
/// A growth ratio, deliberately, rather than a retention ratio. Retention is bounded above by
/// 1.0, so it could only ever signal downward and the multiplier could never rise. Growth is
/// symmetric, and it is a revealed preference that the current price is affordable — which a
/// client-side configuration default cannot pollute.
///
/// An empty previous epoch reads as neutral rather than as an error: an empty network is not a
/// signal. In practice this branch is reachable in epoch 2 only.
#[must_use]
pub fn participation_micros(stores_now: u64, stores_prev: u64) -> u64 {
    if stores_prev == 0 {
        return MULT_SCALE;
    }
    let ratio = u128::from(stores_now) * u128::from(MULT_SCALE) / u128::from(stores_prev);
    clamp_signal(ratio)
}

/// The secondary signal: collateral locked against what the *counted* advertisements owed.
///
/// The denominator uses this epoch's advertisement count against the previous epoch's
/// requirement, because that is precisely the amount those coins had to post to qualify.
///
/// This signal is the one a client-side safety margin corrupts — every operator running a 1%
/// margin makes it read 1.01 forever, carrying no information about affordability. That is why
/// it holds only a quarter of the weight: with participation neutral, volume alone would have to
/// reach 1.40 to push saturation out of the dead band, which no supported preset can do.
#[must_use]
pub fn volume_micros(locked_now: u64, stores_now: u64, required_per_store_prev: u64) -> u64 {
    let required_total_prev = u128::from(stores_now) * u128::from(required_per_store_prev);
    if required_total_prev == 0 {
        return MULT_SCALE;
    }
    let ratio = u128::from(locked_now) * u128::from(MULT_SCALE) / required_total_prev;
    clamp_signal(ratio)
}

/// Combine the two signals into the single reading the controller bands.
#[must_use]
pub const fn saturation_micros(participation_micros: u64, volume_micros: u64) -> u64 {
    (PARTICIPATION_WEIGHT * participation_micros + VOLUME_WEIGHT * volume_micros)
        / SIGNAL_WEIGHT_TOTAL
}

/// Derive all three signals for an epoch from its census and the previous epoch's state.
#[must_use]
pub fn signals_for(
    census: &EpochCensus,
    stores_prev: u64,
    required_per_store_prev: u64,
) -> Signals {
    let participation = participation_micros(census.stores, stores_prev);
    let volume = volume_micros(census.locked, census.stores, required_per_store_prev);
    Signals {
        participation_micros: participation,
        volume_micros: volume,
        saturation_micros: saturation_micros(participation, volume),
    }
}

/// Apply one controller step to the previous multiplier.
///
/// Both steps are a fixed fraction of the *previous multiplier*, never proportional to the
/// deviation, and both are smaller than the dead-band width — so a multiplier arriving from
/// outside the band cannot be flung across it faster than the band absorbs. That is the damping
/// a PID controller would take from its derivative term.
///
/// The floor is applied *after* the step, so the clamp can never be mistaken for a signal the
/// controller acted on.
#[must_use]
pub fn step_multiplier(prev_multiplier_micros: u64, saturation_micros: u64) -> u64 {
    let stepped = match Band::of_saturation(saturation_micros) {
        Band::High => {
            let step = prev_multiplier_micros / UP_STEP_DENOM;
            prev_multiplier_micros
                .saturating_add(step)
                .min(MULT_CEILING_MICROS)
        }
        Band::Low => {
            let step = prev_multiplier_micros / DOWN_STEP_DENOM;
            prev_multiplier_micros.saturating_sub(step)
        }
        Band::Inside => prev_multiplier_micros,
    };
    stepped.max(MULT_FLOOR_MICROS)
}

/// Clamp a `u128` ratio to [`SIGNAL_CAP_MICROS`] and narrow it. The clamp happens before the
/// cast, so the narrowing is proved rather than assumed.
fn clamp_signal(ratio_micros: u128) -> u64 {
    let capped = ratio_micros.min(u128::from(SIGNAL_CAP_MICROS));
    u64::try_from(capped).unwrap_or(SIGNAL_CAP_MICROS)
}
