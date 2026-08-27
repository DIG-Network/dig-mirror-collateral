//! The complete constant set of the collateral specification.
//!
//! Every value here is part of the consensus contract. Two categories exist, and the
//! distinction matters more than the numbers do:
//!
//! - **Load-bearing** — a different value changes the economics of the network. Changing one is
//!   an economic re-decision, not a tuning exercise.
//! - **Arbitrary but fixed** — any value would have worked, but every node must agree on the one
//!   that was picked. Changing one is a hard fork that buys nothing.
//!
//! Each constant below says which it is. Do not "improve" a number in either category.

// ---------------------------------------------------------------------------
// Price level
// ---------------------------------------------------------------------------

/// Per-store collateral at a multiplier of exactly 1.0x, in DIG mojos (5.000 DIG).
///
/// LOAD-BEARING: this sets the entire price level of the network.
pub const EQUILIBRIUM_PER_STORE_MOJOS: u64 = 5_000;

/// The absolute lower bound on a per-store requirement, in DIG mojos (1.000 DIG).
///
/// LOAD-BEARING: it is the Sybil floor during bootstrap, when the handicap is at its maximum and
/// would otherwise drive the requirement toward zero — making identity-splitting free in exactly
/// the phase where the owner count drives the subsidy.
pub const MIN_REQUIRED_PER_STORE_MOJOS: u64 = 1_000;

// ---------------------------------------------------------------------------
// Multiplier fixed-point
// ---------------------------------------------------------------------------

/// Fixed-point scale of the multiplier and of every saturation signal: `1_000_000 == 1.0`.
///
/// ARBITRARY BUT FIXED. Any scale works; changing it is a hard fork.
pub const MULT_SCALE: u64 = 1_000_000;

/// The epoch-1 multiplier anchor, from which the whole recurrence unrolls.
///
/// ARBITRARY BUT FIXED.
pub const MULT_BOOTSTRAP_MICROS: u64 = 1_000_000;

/// Absolute multiplier floor (0.001x), clamped *after* the step is applied.
///
/// ARBITRARY BUT FIXED, and effectively unreachable because [`MIN_REQUIRED_PER_STORE_MOJOS`]
/// binds first.
pub const MULT_FLOOR_MICROS: u64 = 1_000;

/// Multiplier saturation bound (1e6x).
///
/// ARBITRARY BUT FIXED, and a *representational* saturation rather than an economic ceiling:
/// reaching it from 1.0x needs 118 consecutive maximum up-steps. Stating it explicitly is
/// strictly better than an unstated `u64` wrap.
pub const MULT_CEILING_MICROS: u64 = 1_000_000_000_000;

// ---------------------------------------------------------------------------
// Controller
// ---------------------------------------------------------------------------

/// Saturation at or above this value, and at or below [`DEADBAND_HIGH_MICROS`], holds the
/// multiplier flat. Strictly below it, the multiplier steps down.
///
/// LOAD-BEARING: it decides how much contraction the network tolerates before the price falls.
pub const DEADBAND_LOW_MICROS: u64 = 950_000;

/// The upper edge of the dead band; saturation strictly above it steps the multiplier up.
///
/// LOAD-BEARING, and the single most load-bearing value in this crate. It must stay strictly
/// above the saturation a universally-adopted safety margin can produce, with headroom.
///
/// The comparison is against the *saturation*, not against the margin itself. A margin moves only
/// [`VOLUME_WEIGHT`], which is one quarter of the reading, so a network where every operator runs
/// the most generous 5% preset reads a volume signal of `1_050_000` and a saturation of
/// `1_012_500` — leaving `87_500` of headroom here, and needing a 40% margin (4000 bp), eight
/// times the largest preset, to reach this edge at all. `properties.rs` asserts that headroom
/// from the presets themselves rather than from these numbers.
///
/// Stating that correctly matters in the safe direction: the 3:1 weighting buys considerably more
/// protection than a reading of the volume signal alone suggests, so this is a wider margin than
/// it looks, not a narrower one. Lowering this constant toward `1_012_500` would spend it, and let
/// a network of margin defaults ratchet the price upward forever on a signal that carries no
/// information about affordability.
pub const DEADBAND_HIGH_MICROS: u64 = 1_100_000;

/// Denominator of the per-epoch up-step: the multiplier rises by at most `prev / 8` (+12.5%).
///
/// LOAD-BEARING. Together with [`DOWN_STEP_DENOM`] it bounds how fast a capital-rich attacker
/// can displace the price, and both steps must stay smaller than the dead-band width.
pub const UP_STEP_DENOM: u64 = 8;

/// Denominator of the per-epoch down-step: the multiplier falls by at most `prev / 16` (-6.25%).
///
/// LOAD-BEARING, and the *asymmetry* against [`UP_STEP_DENOM`] is itself load-bearing. Downward
/// is the direction an attacker wants, because every step down cheapens every future Sybil
/// identity; halving the downward rate doubles both the sustained cost of such a campaign and
/// the warning honest operators receive.
pub const DOWN_STEP_DENOM: u64 = 16;

/// Weight of the participation signal when combining signals into saturation.
///
/// LOAD-BEARING: the 3:1 ratio against [`VOLUME_WEIGHT`] is what keeps a client-side safety
/// margin from moving the controller on its own.
pub const PARTICIPATION_WEIGHT: u64 = 3;

/// Weight of the volume signal when combining signals into saturation.
///
/// LOAD-BEARING; see [`PARTICIPATION_WEIGHT`].
pub const VOLUME_WEIGHT: u64 = 1;

/// Sum of the signal weights. The weighted signal sum is floor-divided by this.
pub const SIGNAL_WEIGHT_TOTAL: u64 = PARTICIPATION_WEIGHT + VOLUME_WEIGHT;

/// Per-signal clamp (100x) applied before signals are combined.
///
/// ARBITRARY BUT FIXED: an overflow guard roughly 50x beyond any signal a plausible network
/// produces.
pub const SIGNAL_CAP_MICROS: u64 = 100_000_000;

// ---------------------------------------------------------------------------
// Bootstrap handicap
// ---------------------------------------------------------------------------

/// The bootstrap subsidy at zero verified owners, in DIG mojos (4.000 DIG).
///
/// LOAD-BEARING, and chosen so that at zero owners the requirement lands *exactly* on
/// [`MIN_REQUIRED_PER_STORE_MOJOS`]: maximal subsidy, none of it wasted below the floor, and no
/// flat region where the curve does nothing.
pub const HANDICAP_MAX_MOJOS: u64 = 4_000;

/// The verified-owner count at which the subsidy reaches zero.
///
/// LOAD-BEARING. "Verified owner" means a distinct collateralised owner puzzle hash — it is not
/// a node count and not an operator count.
pub const HANDICAP_ZERO_AT_OWNERS: u64 = 1_000;

// ---------------------------------------------------------------------------
// Census finality
// ---------------------------------------------------------------------------

/// Blocks of confirmation depth before an epoch census is final.
///
/// ARBITRARY BUT FIXED: at roughly 18.75 s per block this is about ten minutes, i.e. 0.1% of a
/// seven-day epoch, so the lag is free.
pub const CENSUS_FINALITY_DEPTH_BLOCKS: u64 = 32;

// ---------------------------------------------------------------------------
// Sync sampling (not consensus — see the `sync` module)
// ---------------------------------------------------------------------------

/// The sample size the plan plateaus at, once the population is large enough to need a sample.
///
/// ARBITRARY BUT FIXED, and not consensus.
pub const SYNC_MAX_SAMPLE: u64 = 9;

/// Below this population the confidence *assumption* fails, so the sample is advisory only.
///
/// ARBITRARY BUT FIXED, and not consensus.
pub const SYNC_MIN_POPULATION: u64 = 20;

/// At most `N / 5` of the population is assumed dishonest — an assumed honest fraction of 80%.
///
/// LOAD-BEARING for the confidence claim, though not for consensus: every confidence number in
/// [`crate::sync`] is conditional on it, so it must never change silently.
pub const SYNC_ASSUMED_DISHONEST_DENOM: u64 = 5;

// ---------------------------------------------------------------------------
// Client-side safety margin (not consensus — see the `margin` module)
// ---------------------------------------------------------------------------

/// Basis-point denominator: `10_000 bp == 100%`.
pub const BASIS_POINTS_SCALE: u64 = 10_000;

/// Tightest safety-margin preset: 1 bp (0.01%).
pub const SAFETY_MARGIN_BP_TIGHT: u64 = 1;

/// Default safety-margin preset: 100 bp (1%).
pub const SAFETY_MARGIN_BP_DEFAULT: u64 = 100;

/// Most generous safety-margin preset: 500 bp (5%).
pub const SAFETY_MARGIN_BP_GENEROUS: u64 = 500;

/// The three presets a client surfaces, tightest first.
pub const SAFETY_MARGIN_PRESETS_BP: [u64; 3] = [
    SAFETY_MARGIN_BP_TIGHT,
    SAFETY_MARGIN_BP_DEFAULT,
    SAFETY_MARGIN_BP_GENEROUS,
];
