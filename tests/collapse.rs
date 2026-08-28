//! The collapse region: the two guards in `required_per_store` that only a contracting network
//! reaches.
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
//! by mutation: deleting the clamp fails only the first, and replacing `saturating_sub` with a
//! wrapping one fails only the second.
//!
//! `the_deepest_contraction_costs_one_base_unit` is deliberately *not* part of that pair. It
//! asserts the exact value and so fails under either mutation, which is what makes it a statement
//! about the model's cheapest reachable price rather than about one guard.

use dig_mirror_collateral::{
    required_per_store, EpochCensus, EpochRecord, MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS,
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

/// The clamp does not swallow the bottom of the multiplier's range.
///
/// This is the property the 0.001 DIG floor exists for, and the one a 1.000 DIG floor destroyed:
/// with the clamp applied *after* the multiplier, any floor above a base unit makes every
/// multiplier below `floor / equilibrium` compare equal. At 1.000 DIG that was everything under
/// `0.200x` — so `MULT_FLOOR_MICROS`, the stated `0.001x` bound, was unreachable and three orders
/// of magnitude of the multiplier's range expressed a single price.
///
/// The owner count is `HANDICAP_ZERO_AT_OWNERS`, so the subsidy is zero and the multiplier is the
/// only thing varying — otherwise the handicap could flatten these values on its own and the test
/// would pass without the clamp having anything to do with it.
#[test]
fn multipliers_across_the_whole_floor_range_stay_distinguishable() {
    let mature = 1_000;

    let at_mult_floor = required_per_store(1_000, mature); // 0.001x
    let five_hundredths = required_per_store(50_000, mature); // 0.050x
    let one_fifth = required_per_store(200_000, mature); // 0.200x

    assert_eq!(at_mult_floor, 5, "0.001x of 5.000 DIG is 0.005 DIG");
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

/// The lowest requirement the model can express is one base unit, and it is reachable.
///
/// Both bounds bind at once here: the multiplier floor is the lowest price the controller can
/// reach, and the maximum subsidy takes it below zero from there. A deeply contracted, brand-new
/// network is the intended reading of this number, not an accident.
#[test]
fn the_deepest_contraction_costs_one_base_unit() {
    assert_eq!(
        required_per_store(1_000, 0),
        MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS,
        "0.001x with the full subsidy is the cheapest the model goes: 0.001 DIG"
    );
}
