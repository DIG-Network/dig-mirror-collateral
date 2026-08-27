//! The client-side safety margin. Never consensus, never an input to any census or signal.

use crate::constants::BASIS_POINTS_SCALE;

/// How much collateral to actually post for a requirement, given a margin in basis points.
///
/// Rounds **up**, and it is the only place in this crate that does. A margin that rounds down can
/// leave a node one mojo short of the requirement, which is precisely the failure the margin
/// exists to prevent.
///
/// The result never enters a census, a signal, or a record. It is what an operator chooses to
/// over-post for their own safety, and the controller is deliberately built so that choice cannot
/// move the price on its own.
///
/// ```
/// use dig_mirror_collateral::apply_safety_margin;
/// // The 1% default over a 1_036 mojo requirement: 1_046.36, rounded up.
/// assert_eq!(apply_safety_margin(1_036, 100), 1_047);
/// // A zero margin is exact, not one mojo over.
/// assert_eq!(apply_safety_margin(1_036, 0), 1_036);
/// ```
#[must_use]
pub fn apply_safety_margin(required_per_store_mojos: u64, margin_bp: u64) -> u64 {
    let scale = u128::from(BASIS_POINTS_SCALE);
    let numerator =
        u128::from(required_per_store_mojos) * (scale + u128::from(margin_bp)) + scale - 1;
    u64::try_from(numerator / scale).unwrap_or(u64::MAX)
}
