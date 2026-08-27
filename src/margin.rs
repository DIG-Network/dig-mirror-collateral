//! The client-side safety margin. Never consensus, never an input to any census or signal.

use crate::constants::BASIS_POINTS_SCALE;

/// How much collateral to actually post for a requirement, given a margin in basis points.
///
/// Rounds **up**, and it is the only place in this crate that does. A margin that rounds down can
/// leave a node one DIG base unit short of the requirement, which is precisely the failure the margin
/// exists to prevent.
///
/// The result never enters a census, a signal, or a record. It is what an operator chooses to
/// over-post for their own safety, and the controller is deliberately built so that choice cannot
/// move the price on its own.
///
/// Both the product and the round-up addend saturate. A `u64` requirement multiplied by a `u64`
/// margin exceeds `u128` at the top of the range, where an unchecked product would panic under
/// `debug` and wrap under `release` — turning the largest conceivable margin into the smallest
/// amount posted, which is the one direction this function must never fail in.
///
/// ```
/// use dig_mirror_collateral::apply_safety_margin;
/// // The 1% default over a 1.036 DIG requirement: 1_046.36, rounded up.
/// assert_eq!(apply_safety_margin(1_036, 100), 1_047);
/// // A zero margin is exact, not one base unit over.
/// assert_eq!(apply_safety_margin(1_036, 0), 1_036);
/// ```
#[must_use]
pub fn apply_safety_margin(required_per_store_dig_base_units: u64, margin_bp: u64) -> u64 {
    let scale = u128::from(BASIS_POINTS_SCALE);
    let numerator = u128::from(required_per_store_dig_base_units)
        .saturating_mul(scale + u128::from(margin_bp))
        .saturating_add(scale - 1);
    u64::try_from(numerator / scale).unwrap_or(u64::MAX)
}
