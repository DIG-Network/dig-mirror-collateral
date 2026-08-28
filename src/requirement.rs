//! Composing a multiplier and an owner count into the per-store collateral requirement.

use crate::constants::{
    EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS, MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS, MULT_SCALE,
};
use crate::handicap::handicap_for_owners;

/// The equilibrium price scaled by the multiplier, in DIG base units, before any subsidy or clamp.
///
/// Computed in `u128` and narrowed by saturation rather than by `expect`, so no arrangement of
/// inputs reaches a panic. A node that panics where another wraps has forked by another route.
#[must_use]
pub fn base_per_store(multiplier_micros: u64) -> u64 {
    let scaled = u128::from(EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS) * u128::from(multiplier_micros)
        / u128::from(MULT_SCALE);
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

/// The per-store collateral an advertisement must post to qualify for an epoch, in DIG base units.
///
/// Equilibrium times multiplier, less the bootstrap subsidy, clamped up to
/// [`MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS`]. The subtraction saturates, so an oversized subsidy
/// yields zero and is then lifted by the clamp — it can never wrap into an enormous requirement.
///
/// # The two guards are separate, and only one of them is about price
///
/// The clamp is a single base unit, so it does nothing but forbid a requirement of zero. It is
/// deliberately not a price floor: applied after the multiplier, any larger value would flatten
/// the bottom of the multiplier's range and make requirements that differ by three orders of
/// magnitude compare equal. The price a contracting network falls to is decided by
/// [`crate::constants::MULT_FLOOR_MICROS`], which is the bound that is *about* price.
///
/// The saturation is about a different failure: a subsidy larger than the scaled price, which a
/// plain subtraction would wrap into a requirement near `u64::MAX` — turning the cheapest phase of
/// the network into an unpayable one. `tests/collapse.rs` mutates each guard separately, because a
/// single test covering both would say only that something in this region broke.
///
/// ```
/// use dig_mirror_collateral::required_per_store;
/// // Bootstrap: 1.0x with no verified owners is the equilibrium price less the full subsidy.
/// assert_eq!(required_per_store(1_000_000, 0), 1_000); // 1.000 DIG
/// // Fully grown: the subsidy is gone and the requirement is the equilibrium price.
/// assert_eq!(required_per_store(1_000_000, 1_000), 5_000); // 5.000 DIG
/// // Deep contraction stays expressible: 0.05x and 0.001x are not the same price.
/// assert_eq!(required_per_store(50_000, 1_000), 250); // 0.250 DIG
/// assert_eq!(required_per_store(1_000, 1_000), 5); // 0.005 DIG
/// ```
#[must_use]
pub fn required_per_store(multiplier_micros: u64, owners: u64) -> u64 {
    let base = base_per_store(multiplier_micros);
    let subsidised = base.saturating_sub(handicap_for_owners(owners));
    subsidised.max(MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS)
}
