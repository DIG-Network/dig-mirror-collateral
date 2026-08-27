//! Protocol versioning: schedule invariants, boundary semantics, and the fail-closed refusal.
//!
//! Only version 1 exists, and this suite deliberately does not invent a version 2 ruleset. Where
//! multi-version behaviour has to be pinned, it is pinned against an **explicit schedule** passed
//! to `version_for_epoch_in`, whose rows name versions that are not implemented. That is exactly
//! the situation a node meets after the network activates a ruleset it has not installed — so the
//! hypothetical schedule is not a stand-in for the real test, it *is* the real case.

use dig_mirror_collateral::{
    version_for_epoch, version_for_epoch_in, Activation, CollateralError, EpochCensus, EpochRecord,
    ProtocolVersion, ACTIVATION_SCHEDULE,
};

/// A version the network might activate and this build does not implement.
const FUTURE: ProtocolVersion = ProtocolVersion(2);

/// A schedule in which a future ruleset takes over at epoch 500.
fn schedule_with_future_activation() -> Vec<Activation> {
    vec![
        Activation {
            version: ProtocolVersion::V1,
            first_epoch: 1,
        },
        Activation {
            version: FUTURE,
            first_epoch: 500,
        },
    ]
}

// ---------------------------------------------------------------------------
// Schedule invariants
// ---------------------------------------------------------------------------

/// The schedule is ordered and gapless from epoch 1, which `version_for_epoch_in` relies on.
///
/// It scans in reverse for the first row that has activated, which is correct only if the rows
/// ascend. An unordered row would silently return the wrong ruleset for a range of epochs, and
/// wrong rules are the failure this whole module exists to prevent.
#[test]
fn the_activation_schedule_is_ordered_and_starts_at_epoch_one() {
    assert!(
        !ACTIVATION_SCHEDULE.is_empty(),
        "an empty schedule governs no epoch at all"
    );
    assert_eq!(
        ACTIVATION_SCHEDULE[0].first_epoch, 1,
        "epochs are one-based, so the first row must govern epoch 1 and leave no ungoverned epoch"
    );

    for pair in ACTIVATION_SCHEDULE.windows(2) {
        assert!(
            pair[0].first_epoch < pair[1].first_epoch,
            "activation epochs must strictly ascend"
        );
        assert!(
            pair[0].version < pair[1].version,
            "versions must strictly ascend with their activation epochs"
        );
    }
}

/// Every version the schedule names is one this build can execute.
///
/// A build shipping a schedule row for rules it does not have would refuse every epoch past that
/// activation — correct, but a refusal that could have been caught here instead of in production.
#[test]
fn the_schedule_only_names_versions_this_build_implements() {
    for activation in ACTIVATION_SCHEDULE {
        assert!(
            activation.version.is_implemented(),
            "the schedule activates {:?}, which this build does not implement",
            activation.version
        );
    }
}

/// The tripwire on `EpochRecord::advance`'s dispatch point.
///
/// `advance` has no version branch, because with one implemented ruleset a branch would be
/// unreachable and therefore unprovable. This test is the guard instead: adding a version to
/// `ProtocolVersion::IMPLEMENTED` fails here, which forces the author to the dispatch site rather
/// than letting epochs governed by the new version be computed silently under v1 arithmetic.
///
/// When a second ruleset lands, replace this with a match in `advance` — do not simply widen the
/// expectation below.
#[test]
fn the_dispatch_covers_every_implemented_version() {
    assert_eq!(
        ProtocolVersion::IMPLEMENTED,
        &[ProtocolVersion::V1],
        "`EpochRecord::advance` derives every epoch under v1 with no dispatch. A version added \
         here without a dispatch arm there would compute the new ruleset's epochs under the old \
         rules and fork the network."
    );
}

// ---------------------------------------------------------------------------
// Activation is by epoch, and the boundary is inclusive
// ---------------------------------------------------------------------------

/// New rules apply **at** the activation epoch, not from the epoch after it.
///
/// This is the off-by-one the design most invites, so it is pinned from both sides of the
/// boundary rather than only from the side that happens to be convenient.
#[test]
fn a_ruleset_governs_its_own_activation_epoch() {
    let schedule = schedule_with_future_activation();

    assert_eq!(
        version_for_epoch_in(&schedule, 499),
        Ok(ProtocolVersion::V1),
        "the epoch before an activation is still the previous ruleset's"
    );
    assert_eq!(
        version_for_epoch_in(&schedule, 500),
        Ok(FUTURE),
        "the activation epoch is the FIRST epoch computed under the new rules"
    );
    assert_eq!(
        version_for_epoch_in(&schedule, 501),
        Ok(FUTURE),
        "and every epoch after it"
    );
}

/// Historical epochs keep their own ruleset forever, however many activations follow.
///
/// This is what makes replay from genesis well-defined: an early epoch is not recomputed under
/// whatever rules are current, or the present state would change every time the network upgraded.
#[test]
fn an_early_epoch_keeps_its_ruleset_after_later_activations() {
    let mut schedule = schedule_with_future_activation();
    schedule.push(Activation {
        version: ProtocolVersion(3),
        first_epoch: 900,
    });

    for epoch in [1, 2, 17, 499] {
        assert_eq!(
            version_for_epoch_in(&schedule, epoch),
            Ok(ProtocolVersion::V1),
            "epoch {epoch} was computed under v1 and must stay v1 forever"
        );
    }
}

/// An epoch no row covers is refused rather than assigned the earliest ruleset.
#[test]
fn an_ungoverned_epoch_is_refused() {
    assert_eq!(
        version_for_epoch(0),
        Err(CollateralError::EpochNotGoverned { epoch: 0 }),
        "epoch 0 does not exist in a one-based numbering"
    );
    assert_eq!(
        version_for_epoch_in(&[], 7),
        Err(CollateralError::EpochNotGoverned { epoch: 7 }),
        "an empty schedule governs nothing, and must say so rather than pick a default"
    );
}

// ---------------------------------------------------------------------------
// Fail closed
// ---------------------------------------------------------------------------

/// An epoch governed by an unimplemented ruleset is refused, and the refusal names the version.
///
/// The alternative — falling back to the newest known ruleset — is the dangerous branch precisely
/// because it looks like success: a plausible requirement, a silent disagreement with the network,
/// and stores that stop earning while every surface reports health.
#[test]
fn an_unimplemented_version_is_refused_and_named() {
    let schedule = schedule_with_future_activation();

    let governing = version_for_epoch_in(&schedule, 500).expect("epoch 500 is governed");
    assert_eq!(governing, FUTURE);

    assert_eq!(
        governing.implemented(),
        Err(CollateralError::UnknownProtocolVersion { version: 2 }),
        "the refusal must name the version so an operator learns what to install"
    );
    assert!(!governing.is_implemented());

    // The refusal must not degrade into the newest known ruleset.
    assert_ne!(
        governing.implemented(),
        Ok(ProtocolVersion::V1),
        "falling back to v1 is the failure this gate exists to prevent"
    );
}

/// A record computed under an unimplemented ruleset cannot be extended.
///
/// This is the fail-closed gate on the recurrence itself, and it is reachable today: a record
/// arriving over gossip carries its own version, so a peer running a newer ruleset hands this node
/// a seed it cannot reproduce. Continuing from it would substitute v1 arithmetic for the seed and
/// produce a requirement no other node computes.
#[test]
fn a_record_from_an_unimplemented_ruleset_cannot_be_advanced() {
    let mut foreign = EpochRecord::bootstrap();
    foreign.protocol_version = FUTURE;

    let err = foreign
        .advance(EpochCensus {
            epoch: 2,
            stores: 12,
            owners: 9,
            locked: 12_120,
        })
        .expect_err("a seed this build cannot reproduce must not be extended");

    assert_eq!(err, CollateralError::UnknownProtocolVersion { version: 2 });
}

/// The same census on an ordinary record still advances, so the guard above is not vacuous.
///
/// Without this, a mistake that made `advance` refuse *everything* would look identical to a
/// correctly targeted refusal.
#[test]
fn an_ordinary_record_still_advances() {
    let epoch2 = EpochRecord::bootstrap()
        .advance(EpochCensus {
            epoch: 2,
            stores: 12,
            owners: 9,
            locked: 12_120,
        })
        .expect("epoch 2 follows epoch 1 under v1");

    assert_eq!(epoch2.protocol_version, ProtocolVersion::V1);
    assert_eq!(epoch2.required_per_store_dig_base_units, 1_036);
}

// ---------------------------------------------------------------------------
// The version is part of the record
// ---------------------------------------------------------------------------

/// Every derived record carries the version that computed it, and it survives serialisation.
///
/// The version travels with the record over gossip so that a disagreement about *which rules
/// applied* is a named mismatch rather than an unexplained difference between two numbers.
#[test]
fn the_version_is_recorded_and_round_trips() {
    let epoch1 = EpochRecord::bootstrap();
    assert_eq!(epoch1.protocol_version, ProtocolVersion::V1);

    let json = serde_json::to_string(&epoch1).expect("a record serialises");
    assert!(
        json.contains("\"protocol_version\":1"),
        "the version must be a plain number on the wire, not a struct: {json}"
    );

    let back: EpochRecord = serde_json::from_str(&json).expect("a record round-trips");
    assert_eq!(back, epoch1);
}

/// An unknown version deserialises rather than failing to parse.
///
/// Representability is what lets a node say *"epoch 500 is governed by v2 and I implement v1"*
/// instead of *"malformed input"*. An enum would reject the record at the parse and turn a precise,
/// actionable refusal into a mystery.
#[test]
fn an_unknown_version_parses_so_it_can_be_named() {
    let parsed: ProtocolVersion = serde_json::from_str("2").expect("an unknown version parses");
    assert_eq!(parsed, FUTURE);
    assert_eq!(
        parsed.implemented(),
        Err(CollateralError::UnknownProtocolVersion { version: 2 })
    );
}
