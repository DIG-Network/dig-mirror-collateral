//! Degenerate and extreme inputs: the places where "neutral" and "saturating" are the contract.

use dig_mirror_collateral::{
    apply_safety_margin, base_per_store, participation_micros, required_per_store,
    sync_sample_plan, volume_micros, Band, EpochCensus, EpochRecord, MULT_CEILING_MICROS,
    MULT_FLOOR_MICROS, MULT_SCALE, SIGNAL_CAP_MICROS, SYNC_MAX_SAMPLE, SYNC_MIN_POPULATION,
};

/// An empty network is not a signal. Both degenerate denominators read as exactly neutral, so a
/// network with nothing in it holds the price rather than refusing to compute one or reading its
/// own emptiness as collapse.
#[test]
fn empty_denominators_read_as_neutral() {
    assert_eq!(participation_micros(0, 0), MULT_SCALE);
    assert_eq!(participation_micros(500, 0), MULT_SCALE);

    // `required_total_prev` is zero whenever either factor is, and both are reachable: no
    // advertisements this epoch, or a previous requirement of zero.
    assert_eq!(volume_micros(0, 0, 5_000), MULT_SCALE);
    assert_eq!(volume_micros(10_000, 500, 0), MULT_SCALE);

    // And it composes: epoch 2 is the epoch where the previous store count is zero by definition.
    let epoch2 = EpochRecord::bootstrap()
        .advance(EpochCensus {
            epoch: 2,
            stores: 0,
            owners: 0,
            locked: 0,
        })
        .expect("consecutive");
    assert_eq!(epoch2.band, Some(Band::Inside));
    assert_eq!(epoch2.multiplier_micros, MULT_SCALE);
}

/// Both signals clamp before they are narrowed, so an implausible ratio cannot reach the
/// combining step at a width that could overflow it.
#[test]
fn signals_clamp_rather_than_overflow() {
    assert_eq!(participation_micros(u64::MAX, 1), SIGNAL_CAP_MICROS);
    assert_eq!(volume_micros(u64::MAX, 1, 1), SIGNAL_CAP_MICROS);

    // The cap is above every signal a plausible network produces, so it never binds in practice.
    assert!(participation_micros(1_500, 1_000) < SIGNAL_CAP_MICROS);
}

/// The multiplier saturates at both ends without panicking and without wrapping.
#[test]
fn the_multiplier_saturates_at_both_ends() {
    // Repeated maximum up-steps stop at the representational ceiling.
    let mut multiplier = MULT_SCALE;
    for _ in 0..1_000 {
        multiplier = dig_mirror_collateral::step_multiplier(multiplier, SIGNAL_CAP_MICROS);
    }
    assert_eq!(multiplier, MULT_CEILING_MICROS);

    // Repeated maximum down-steps stop at the floor, which is applied after the step.
    for _ in 0..100_000 {
        multiplier = dig_mirror_collateral::step_multiplier(multiplier, 0);
    }
    assert_eq!(multiplier, MULT_FLOOR_MICROS);
    assert_eq!(
        dig_mirror_collateral::step_multiplier(MULT_FLOOR_MICROS, 0),
        MULT_FLOOR_MICROS
    );
}

/// Even at the representational ceiling the composed requirement stays a real number rather than
/// panicking or wrapping — the property that makes the ceiling a saturation rather than a bug.
#[test]
fn the_requirement_is_finite_at_the_ceiling() {
    assert_eq!(base_per_store(MULT_CEILING_MICROS), 5_000_000_000);
    assert_eq!(
        required_per_store(MULT_CEILING_MICROS, 0),
        5_000_000_000 - 4_000
    );
    assert_eq!(base_per_store(u64::MAX), 92_233_720_368_547_758);
}

/// The margin rounds up, which is the whole reason it exists — a margin that rounds down can
/// leave a node one mojo short of the requirement it was meant to clear.
#[test]
fn the_margin_rounds_up_and_never_short() {
    for required in [1u64, 999, 1_000, 1_036, 3_351, 5_000] {
        for bp in [0u64, 1, 100, 500] {
            let posted = apply_safety_margin(required, bp);
            assert!(
                posted >= required,
                "margin {bp} bp left {required} short at {posted}"
            );
            // Rounding up rather than down: the exact product, floored, is never greater.
            let floored = u128::from(required) * (10_000 + u128::from(bp)) / 10_000;
            assert!(u128::from(posted) >= floored);
        }
    }

    // The smallest preset over the smallest requirement still moves by a whole mojo, so the
    // tightest preset is not silently a no-op.
    assert_eq!(apply_safety_margin(1_000, 1), 1_001);
    assert_eq!(apply_safety_margin(u64::MAX, 500), u64::MAX);
}

/// The sampling plan plateaus, and says so honestly below the population its assumption needs.
#[test]
fn the_sample_plan_plateaus_and_flags_its_own_assumption() {
    let tiny = sync_sample_plan(3);
    assert_eq!(
        tiny.sample_size, 3,
        "query the whole population when it is tiny"
    );
    assert!(tiny.advisory_only);

    let boundary = sync_sample_plan(SYNC_MIN_POPULATION);
    assert!(
        !boundary.advisory_only,
        "the boundary is inclusive-from-below"
    );
    assert_eq!(boundary.sample_size, SYNC_MAX_SAMPLE);
    assert_eq!(boundary.agreement_threshold, 6);
    assert_eq!(boundary.max_assumed_dishonest, 4);
    assert!(
        boundary.agreement_threshold > boundary.max_assumed_dishonest,
        "at the smallest non-advisory population the assumed dishonest set cannot reach the \
         threshold at all, so the failure probability there is exactly zero"
    );

    assert!(sync_sample_plan(SYNC_MIN_POPULATION - 1).advisory_only);

    // The plateau: past the boundary the sample never grows, however large the network gets.
    for population in [21u64, 27, 1_000, u64::MAX] {
        let plan = sync_sample_plan(population);
        assert_eq!(plan.sample_size, SYNC_MAX_SAMPLE);
        assert_eq!(plan.agreement_threshold, 6);
        assert!(!plan.advisory_only);
    }

    // An empty population degenerates without panicking.
    let empty = sync_sample_plan(0);
    assert_eq!(empty.sample_size, 0);
    assert!(empty.advisory_only);
    assert_eq!(
        empty.agreement_threshold, 1,
        "an empty population must never yield a threshold of zero, which would read as: adopt \
         anything, on no evidence"
    );
}

/// The record serialises and round-trips, because it is gossiped between nodes and compared.
#[test]
fn a_record_round_trips_through_serde() {
    let record = EpochRecord::bootstrap()
        .advance(EpochCensus {
            epoch: 2,
            stores: 12,
            owners: 9,
            locked: 12_120,
        })
        .expect("consecutive");

    let encoded = serde_json::to_string(&record).expect("serialises");
    let decoded: EpochRecord = serde_json::from_str(&encoded).expect("deserialises");
    assert_eq!(decoded, record);
}
