//! The collapse region: the two guards in `required_per_store` that only a contracting network
//! reaches.
//!
//! The golden vectors never drive the multiplier below `1_251_412` micros, so the region where
//! `MIN_REQUIRED_PER_STORE_MOJOS` and the `saturating_sub` first bind sits outside that fixture by
//! construction — no conformance vector can reach it, and neither guard was pinned by anything.
//! Both are on the money path: one is the Sybil floor, the other is what keeps a subsidy larger
//! than the base price from wrapping into an enormous requirement.
//!
//! The two tests below are deliberately *separately* revert-proof. One test that fails under both
//! mutations would say only "something in the collapse region broke"; these say which.

use dig_mirror_collateral::{EpochCensus, EpochRecord, MIN_REQUIRED_PER_STORE_MOJOS};

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

/// The Sybil floor binds after a single contracting epoch, and it is the floor that is observed.
///
/// At 50 verified owners the subsidy is 3.800 DIG against a base of 4.687 DIG, so the subtraction
/// is ordinary — nothing here exercises the saturation, which is what makes this test specific to
/// the clamp. Deleting `.max(MIN_REQUIRED_PER_STORE_MOJOS)` yields 887 mojos: identity-splitting
/// grows cheaper exactly as the network contracts, which is the phase the floor exists for.
#[test]
fn the_sybil_floor_binds_after_one_contracting_epoch() {
    let record = contract_for(1, 50);

    // Pin the fixture into the region under test rather than trusting the arithmetic above: if a
    // constant ever moves the region, this fails loudly instead of passing vacuously.
    assert_eq!(
        record.multiplier_micros, 937_500,
        "one down-step of prev / 16"
    );
    assert_eq!(record.base_mojos, 4_687);
    assert_eq!(record.handicap_mojos, 3_800);
    assert!(
        record.base_mojos > record.handicap_mojos,
        "this test must reach the clamp without touching the saturation"
    );

    assert_eq!(
        record.required_per_store_mojos, MIN_REQUIRED_PER_STORE_MOJOS,
        "the subsidised requirement is 887 mojos and must be lifted to the floor"
    );
}

/// The subtraction saturates once the subsidy exceeds the base price, four contracting epochs in.
///
/// The assertion is deliberately one-sided. Removing the floor clamp leaves zero here, which is
/// still `<= MIN_REQUIRED_PER_STORE_MOJOS` and so passes — that mutation is the other test's job.
/// Replacing `saturating_sub` with `-` panics under `debug` and wraps to roughly `u64::MAX` under
/// `release`, and the clamp then preserves the wrapped value rather than hiding it, so an upper
/// bound catches both profiles.
#[test]
fn the_subsidy_saturates_once_it_exceeds_the_base_price() {
    let record = contract_for(4, 10);

    assert_eq!(
        record.multiplier_micros, 772_478,
        "four down-steps of prev / 16"
    );
    assert_eq!(record.base_mojos, 3_862);
    assert_eq!(record.handicap_mojos, 3_960);
    assert!(
        record.base_mojos < record.handicap_mojos,
        "the subsidy must exceed the base price, or the saturation is never exercised"
    );

    assert!(
        record.required_per_store_mojos <= MIN_REQUIRED_PER_STORE_MOJOS,
        "an oversized subsidy must saturate to zero, never wrap into an enormous requirement: got {}",
        record.required_per_store_mojos
    );
}
