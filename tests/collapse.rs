//! The collapse region: the two guards in `required_per_store` that only a contracting network
//! reaches, and the price the multiplier floor sets once it gets there.
//!
//! The golden vectors never drive the multiplier below `1_251_412` micros, so the region where
//! `MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS` and the `saturating_sub` first bind sits outside that
//! fixture by construction — no conformance vector can reach it, and neither guard was pinned by
//! anything. Both are on the money path: one forbids a requirement of zero, the other is what
//! keeps a subsidy larger than the base price from wrapping into an enormous requirement.
//!
//! # Why every assertion here is one-sided
//!
//! At a floor of one base unit the two guards bind in the **same** region: once the subsidy
//! exceeds the base price the subtraction saturates to zero and the clamp then lifts that zero to
//! one. A test asserting the resulting value exactly would fail under *either* mutation and could
//! only report "something in the collapse region broke".
//!
//! So the two *diagnostic* tests below bound the outcome from one side only, in opposite
//! directions:
//!
//! | mutation | observed | `>= MIN` (floor test) | `<= MIN` (saturation test) |
//! |---|---|---|---|
//! | none | `1` | passes | passes |
//! | clamp deleted | `0` | **fails** | passes |
//! | `saturating_sub` -> `-` | `u64::MAX`-ish (release) | passes | **fails** |
//!
//! Together the two bounds pin the value exactly; separately each names its own guard. Preserve
//! that property — collapsing them into one `assert_eq!` collapses two proofs into one. Verified
//! by mutation, across this pair: deleting the clamp fails only the first, and replacing
//! `saturating_sub` with a wrapping one fails only the second.
//!
//! The claim is about *these two* tests. `bootstrap_at_the_floor_still_costs_one_base_unit`
//! below pins the same value exactly and so responds to both mutations; that is deliberate and
//! is not a defect to tidy away. Making it one-sided would restore the vacuity its concrete
//! assertion exists to prevent.
//!
//! # The two regimes the floor produces, and why both are asserted
//!
//! [`MULT_FLOOR_MICROS`] and [`MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS`] are both floors, and they
//! bind in *different* regimes. The bottom two tests hold one against the other:
//!
//! - **mature** (subsidy zero): the multiplier floor decides the price, and it must be a real cost.
//! - **bootstrap** (subsidy at maximum): the subsidy swallows the scaled price whatever the
//!   multiplier is, so the amount clamp decides, and the price must stay one base unit.
//!
//! Asserting only the mature figure would not distinguish raising the *multiplier* floor from
//! raising the *amount* floor to the same resulting price — both make the mature number correct.
//! Only the bootstrap assertion separates them: raising the amount floor would make a brand-new
//! network a hundred times more expensive, and that is the change this crate is *not* making.

use dig_mirror_collateral::{
    required_per_store, EpochCensus, EpochRecord, EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS,
    HANDICAP_ZERO_AT_OWNERS, MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS, MULT_FLOOR_MICROS, MULT_SCALE,
};

/// Advertisements held constant with nothing collateralised: the simplest census that bands `Low`
/// every epoch.
///
/// Participation reads exactly neutral because the count does not move, volume reads zero because
/// nothing is locked, and the 3:1 weighting puts saturation at `750_000` — below
/// `DEADBAND_LOW_MICROS` — so the multiplier steps down by `prev / 16` each epoch without the
/// fixture having to model a shrinking network as well.
const STORES: u64 = 200;

/// Drive the recurrence forward `epochs` times from bootstrap with a constant contracting census.
fn contract_for(epochs: u64, owners: u64) -> EpochRecord {
    let mut record = EpochRecord::bootstrap();
    for epoch in 2..=(epochs + 1) {
        record = record
            .advance(EpochCensus {
                epoch,
                stores: STORES,
                owners,
                locked: 0,
            })
            .expect("each census follows the epoch before it");
    }
    record
}

/// The clamp forbids a requirement of zero once the subsidy has swallowed the whole base price.
///
/// Four contracting epochs at ten verified owners put the subsidy above the scaled price, so the
/// subtraction reaches zero and only the clamp lifts it. Deleting `.max(..)` yields a requirement
/// of **zero**: an advertisement qualifying for free, in exactly the phase where the owner count
/// drives the subsidy and identity-splitting is cheapest.
///
/// The bound is `>=` rather than `==` on purpose. A wrapping subtraction also produces a value
/// that is not `MIN`, and diagnosing that is the other test's job — see the module note.
#[test]
fn the_clamp_forbids_a_free_advertisement() {
    let record = contract_for(4, 10);

    // Pin the fixture into the region under test rather than trusting the arithmetic above: if a
    // constant ever moves the region, this fails loudly instead of passing vacuously.
    assert_eq!(
        record.multiplier_micros, 772_478,
        "four down-steps of prev / 16"
    );
    assert_eq!(record.base_price_dig_base_units, 3_862);
    assert_eq!(record.handicap_dig_base_units, 3_960);
    assert!(
        record.base_price_dig_base_units < record.handicap_dig_base_units,
        "the subsidy must swallow the base price, or the clamp is never what lifts the result"
    );

    assert!(
        record.required_per_store_dig_base_units >= MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS,
        "the subsidised requirement is zero here and must be lifted to the floor, never left at \
         zero: got {}",
        record.required_per_store_dig_base_units
    );
}

/// The subtraction saturates once the subsidy exceeds the base price, four contracting epochs in.
///
/// The assertion is deliberately one-sided in the opposite direction. Removing the clamp leaves
/// zero here, which is still `<= MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS` and so passes — that
/// mutation is the other test's job. Replacing `saturating_sub` with `-` panics under `debug` and
/// wraps to roughly `u64::MAX` under `release`, and the clamp then preserves the wrapped value
/// rather than hiding it, so an upper bound catches both profiles.
#[test]
fn the_subsidy_saturates_once_it_exceeds_the_base_price() {
    let record = contract_for(4, 10);

    assert_eq!(
        record.multiplier_micros, 772_478,
        "four down-steps of prev / 16"
    );
    assert_eq!(record.base_price_dig_base_units, 3_862);
    assert_eq!(record.handicap_dig_base_units, 3_960);
    assert!(
        record.base_price_dig_base_units < record.handicap_dig_base_units,
        "the subsidy must exceed the base price, or the saturation is never exercised"
    );

    assert!(
        record.required_per_store_dig_base_units <= MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS,
        "an oversized subsidy must saturate to zero, never wrap into an enormous requirement: got {}",
        record.required_per_store_dig_base_units
    );
}

/// The amount clamp does not swallow the bottom of the multiplier's range.
///
/// This is the property the 0.001 DIG amount floor exists for, and the one a 1.000 DIG floor
/// destroyed: with the clamp applied *after* the multiplier, any amount floor above a base unit
/// makes every multiplier below `floor / equilibrium` compare equal. At 1.000 DIG that was
/// everything under `0.200x` — the whole bottom of the multiplier's range expressing a single
/// price.
///
/// The owner count is `HANDICAP_ZERO_AT_OWNERS`, so the subsidy is zero and the multiplier is the
/// only thing varying — otherwise the handicap could flatten these values on its own and the test
/// would pass without the clamp having anything to do with it.
#[test]
fn multipliers_across_the_whole_floor_range_stay_distinguishable() {
    let mature = HANDICAP_ZERO_AT_OWNERS;

    let at_mult_floor = required_per_store(MULT_FLOOR_MICROS, mature);
    let five_hundredths = required_per_store(50_000, mature); // 0.050x
    let one_fifth = required_per_store(200_000, mature); // 0.200x

    assert_eq!(at_mult_floor, 100, "0.020x of 5.000 DIG is 0.100 DIG");
    assert_eq!(five_hundredths, 250, "0.050x of 5.000 DIG is 0.250 DIG");
    assert_eq!(one_fifth, 1_000, "0.200x of 5.000 DIG is 1.000 DIG");

    // The old 1.000 DIG clamp made all three of these equal to 1_000. Stated as an ordering so a
    // future clamp of any size above one base unit fails here rather than only the exact old one.
    assert!(
        at_mult_floor < five_hundredths && five_hundredths < one_fifth,
        "a clamp applied after the multiplier must not collapse distinct multipliers onto one \
         price: got {at_mult_floor}, {five_hundredths}, {one_fifth}"
    );
}

/// A mature network resting on the multiplier floor still charges a real price per identity.
///
/// This is what `MULT_FLOOR_MICROS` is *for*. The subsidy is gone by definition in this regime, so
/// the floor alone decides what one counted advertisement costs — and the census signals the
/// controller reads are only as trustworthy as that cost. A floor that prices an identity at a
/// twentieth of a base unit of value would let a contracted network's own inputs be manufactured,
/// at exactly the moment it is least able to resist it.
///
/// The bound is `>=` because the load-bearing claim is the *direction*: the floor price must not
/// fall below a tenth of a DIG. The exact figure is pinned separately, below, so that a change to
/// the floor is reported as a changed price rather than as a silently loosened bound.
#[test]
fn the_mature_floor_state_still_costs_a_real_price() {
    let at_floor = required_per_store(MULT_FLOOR_MICROS, HANDICAP_ZERO_AT_OWNERS);

    assert!(
        at_floor >= 100,
        "a mature network at the multiplier floor must charge at least 0.100 DIG per store, or \
         census identities are cheap enough to manufacture: got {at_floor}"
    );

    // Derived from the constants rather than restated, so the two cannot drift apart: the mature
    // floor price *is* equilibrium scaled by the floor, with no subsidy and no clamp involved.
    assert_eq!(
        at_floor,
        EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS * MULT_FLOOR_MICROS / MULT_SCALE,
        "the mature floor price is the scaled equilibrium, unclamped"
    );
}

/// Bootstrap at the multiplier floor still costs exactly one base unit.
///
/// This is the assertion that makes the one above a statement about the *multiplier* floor rather
/// than about the price. Raising `MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS` to `100` would satisfy
/// `the_mature_floor_state_still_costs_a_real_price` identically while making a brand-new network
/// a hundred times more expensive to join — the opposite of what the handicap exists to do. Only
/// this test tells the two apart.
///
/// The regime is genuine rather than contrived: with no verified owners the subsidy is at its
/// maximum, it exceeds the scaled price at every multiplier at or near the floor, and the amount
/// clamp is therefore the only thing setting the result.
/// The concrete `1` is asserted **beside** the symbolic form rather than instead of it, and the
/// two catch different things. The symbolic assertion says the clamp is what produced the value;
/// the concrete one says the clamp is still a single base unit. Only the concrete assertion
/// survives a mutation that raises `MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS` itself — a purely
/// symbolic comparison moves with the constant under test and passes, which is exactly the wrong
/// fix this test exists to catch. Found by mutation: with only the symbolic form, raising that
/// constant to `100` left this test green.
#[test]
fn bootstrap_at_the_floor_still_costs_one_base_unit() {
    let at_bootstrap = required_per_store(MULT_FLOOR_MICROS, 0);

    assert_eq!(
        at_bootstrap, MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS,
        "the multiplier floor must not reach the bootstrap regime: the full subsidy still \
         saturates the scaled price to zero, and the amount clamp lifts it to the amount floor"
    );
    assert_eq!(
        at_bootstrap, 1,
        "and that amount floor is still one base unit (0.001 DIG): raising it is the wrong lever \
         for the floor-state price, and would make joining a new network dramatically dearer"
    );
}
