//! Composing a multiplier and an owner count into the per-store collateral requirement.

use crate::constants::{EQUILIBRIUM_PER_STORE_MOJOS, MIN_REQUIRED_PER_STORE_MOJOS, MULT_SCALE};
use crate::handicap::handicap_for_owners;

/// The equilibrium price scaled by the multiplier, in DIG mojos, before any subsidy or clamp.
///
/// Computed in `u128` and narrowed by saturation rather than by `expect`, so no arrangement of
/// inputs reaches a panic. A node that panics where another wraps has forked by another route.
#[must_use]
pub fn base_per_store(multiplier_micros: u64) -> u64 {
    let scaled = u128::from(EQUILIBRIUM_PER_STORE_MOJOS) * u128::from(multiplier_micros)
        / u128::from(MULT_SCALE);
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

/// The per-store collateral an advertisement must post to qualify for an epoch, in DIG mojos.
///
/// Equilibrium times multiplier, less the bootstrap subsidy, clamped up to the floor. The
/// subtraction saturates, so an oversized subsidy yields zero and is then lifted by the clamp —
/// it can never wrap into an enormous requirement.
///
/// ```
/// use dig_mirror_collateral::required_per_store;
/// // Bootstrap: 1.0x with no verified owners lands exactly on the floor.
/// assert_eq!(required_per_store(1_000_000, 0), 1_000);
/// // Fully grown: the subsidy is gone and the requirement is the equilibrium price.
/// assert_eq!(required_per_store(1_000_000, 1_000), 5_000);
/// ```
#[must_use]
pub fn required_per_store(multiplier_micros: u64, owners: u64) -> u64 {
    let base = base_per_store(multiplier_micros);
    let subsidised = base.saturating_sub(handicap_for_owners(owners));
    subsidised.max(MIN_REQUIRED_PER_STORE_MOJOS)
}
