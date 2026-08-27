//! Conformance against the golden vectors in `tests/fixtures/golden_vectors.json`.
//!
//! The fixture is the cross-language contract, so these tests read it as data rather than
//! restating any of its numbers. A number that appears in this file and not in the fixture is a
//! number a second implementation cannot check itself against.

use dig_mirror_collateral::{apply_safety_margin, Band, EpochCensus, EpochRecord, Signals};
use serde::Deserialize;

const FIXTURE: &str = include_str!("fixtures/golden_vectors.json");

#[derive(Debug, Deserialize)]
struct GoldenFile {
    epochs: Vec<GoldenEpoch>,
}

#[derive(Debug, Deserialize)]
struct GoldenEpoch {
    epoch: u64,
    census: EpochCensus,
    signals: Option<Signals>,
    band: Option<Band>,
    multiplier_micros: u64,
    handicap_mojos: u64,
    base_mojos: u64,
    required_per_store_mojos: u64,
    posted_each: u64,
}

fn golden() -> Vec<GoldenEpoch> {
    let file: GoldenFile = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(file.epochs.len(), 10, "the worked table covers ten epochs");
    file.epochs
}

/// Walk the whole recurrence from the bootstrap anchor and check every derived field of every
/// epoch. Field-by-field rather than whole-record, so a divergence names the term that diverged.
#[test]
fn recurrence_reproduces_every_golden_epoch() {
    let expected = golden();
    let mut record = EpochRecord::bootstrap();

    for want in &expected {
        if want.epoch > 1 {
            record = record
                .advance(want.census)
                .expect("golden epochs are consecutive");
        }

        let at = format!("epoch {}", want.epoch);
        assert_eq!(record.epoch, want.epoch, "{at}: epoch");
        assert_eq!(record.census, want.census, "{at}: census");
        assert_eq!(record.signals, want.signals, "{at}: signals");
        assert_eq!(record.band, want.band, "{at}: band");
        assert_eq!(
            record.multiplier_micros, want.multiplier_micros,
            "{at}: multiplier"
        );
        assert_eq!(record.handicap_mojos, want.handicap_mojos, "{at}: handicap");
        assert_eq!(record.base_mojos, want.base_mojos, "{at}: base");
        assert_eq!(
            record.required_per_store_mojos, want.required_per_store_mojos,
            "{at}: required"
        );
    }
}

/// The fixture's `locked` inputs are not free parameters: each is the previous epoch's requirement
/// at the 1% default margin, times this epoch's advertisement count. Checking that closes the loop
/// between the consensus arithmetic and the client-side margin, and it is what makes the fixture a
/// coherent scenario rather than ten unrelated rows.
#[test]
fn golden_locked_amounts_follow_from_the_default_margin() {
    let epochs = golden();

    for want in &epochs {
        assert_eq!(
            want.posted_each,
            apply_safety_margin(want.required_per_store_mojos, 100),
            "epoch {}: posted_each is the 1% margin over this epoch's requirement",
            want.epoch
        );
    }

    for pair in epochs.windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        assert_eq!(
            next.census.locked,
            next.census.stores * prev.posted_each,
            "epoch {}: locked is this epoch's advertisements at last epoch's posted amount",
            next.epoch
        );
    }
}

/// Every epoch of the fixture is reachable in one step from its predecessor, and no other epoch
/// number is accepted in that step. The recurrence is defined only over consecutive epochs.
#[test]
fn advance_refuses_a_non_consecutive_census() {
    let epoch1 = EpochRecord::bootstrap();

    let skipped = EpochCensus {
        epoch: 3,
        stores: 12,
        owners: 9,
        locked: 12_120,
    };
    let err = epoch1
        .advance(skipped)
        .expect_err("epoch 3 does not follow 1");
    assert_eq!(
        err,
        dig_mirror_collateral::CollateralError::NonSequentialEpoch {
            expected: 2,
            found: 3
        }
    );

    let repeated = EpochCensus {
        epoch: 1,
        ..skipped
    };
    assert!(epoch1.advance(repeated).is_err(), "an epoch cannot repeat");
}

/// The shock epoch is the one the table was built around, so it gets its own named check: a 55%
/// collapse in participation moves the multiplier by exactly one down-step and no more.
#[test]
fn the_shock_epoch_moves_exactly_one_down_step() {
    let epochs = golden();
    let before = &epochs[6]; // epoch 7
    let shock = &epochs[7]; // epoch 8

    assert_eq!(shock.band, Some(Band::Low));
    assert!(
        shock.census.stores * 2 < before.census.stores,
        "the fixture's shock is more than a halving of counted advertisements"
    );
    assert_eq!(
        shock.multiplier_micros,
        before.multiplier_micros - before.multiplier_micros / 16,
        "a collapse of any size still moves the multiplier by exactly one -6.25% step"
    );
}
