//! The bootstrap handicap: a subsidy that shrinks as the network gains verified owners.

use crate::constants::{HANDICAP_MAX_DIG_BASE_UNITS, HANDICAP_ZERO_AT_OWNERS};

/// The per-store subsidy, in DIG base units, at a given count of verified owners.
///
/// The curve is linear from [`HANDICAP_MAX_DIG_BASE_UNITS`] at zero owners to zero at
/// [`HANDICAP_ZERO_AT_OWNERS`], and flat at zero above that.
///
/// Linear rather than convex or concave: a convex curve withdraws the subsidy hardest across the
/// first hundred owners, which is the phase the subsidy exists to protect; a concave one holds it
/// near-full then drops it off a cliff. Linear is monotone, has no inflection an implementer can
/// misplace, and makes the composed requirement a clean line an operator can predict without
/// running the code.
///
/// The saturation above [`HANDICAP_ZERO_AT_OWNERS`] is expressed as a `min()` on the owner count
/// rather than a subtraction corrected afterwards, so there is no branch to get wrong and no
/// arrangement of inputs under which a subsidy becomes a surcharge.
///
/// ```
/// use dig_mirror_collateral::handicap_for_owners;
/// assert_eq!(handicap_for_owners(0), 4_000);
/// assert_eq!(handicap_for_owners(500), 2_000);
/// assert_eq!(handicap_for_owners(1_000), 0);
/// assert_eq!(handicap_for_owners(u64::MAX), 0);
/// ```
#[must_use]
pub const fn handicap_for_owners(owners: u64) -> u64 {
    let counted = if owners < HANDICAP_ZERO_AT_OWNERS {
        owners
    } else {
        HANDICAP_ZERO_AT_OWNERS
    };
    let remaining = HANDICAP_ZERO_AT_OWNERS - counted;
    HANDICAP_MAX_DIG_BASE_UNITS * remaining / HANDICAP_ZERO_AT_OWNERS
}
