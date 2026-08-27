//! Profile-independence at the top of the input range.
//!
//! Every function here is public and re-exported, so a caller can reach it with any `u64` the
//! type admits — not merely the clamped values the recurrence feeds it. Unchecked arithmetic on
//! that surface panics under `debug` and wraps under `release`, which is the crate's central
//! prohibition wearing a different hat: a node that panics where another wraps has forked. These
//! tests assert the saturated answer, so they fail under *both* profiles without the widening —
//! by panic under one and by a wrong number under the other.

use dig_mirror_collateral::{
    apply_safety_margin, saturation_micros, EpochCensus, EpochRecord, MULT_SCALE, SIGNAL_CAP_MICROS,
};

/// The weighted sum overflows `u64` above `u64::MAX / 3`, so the largest admissible participation
/// reading is enough on its own — no help from the volume term.
///
/// The three values that distinguish this from a wrong fix are all here. Unchecked `u64` wraps to
/// `4_611_686_018_427_387_903`; a `u128` widening narrowed by a bare `as` cast rather than a clamp
/// yields `13_835_058_055_282_163_711`, which fits `u64` and so passes silently; and a fix that
/// merely makes the function private fails to compile this file at all, since a test target is an
/// external crate.
#[test]
fn saturation_saturates_instead_of_diverging_by_profile() {
    assert_eq!(saturation_micros(u64::MAX, 0), SIGNAL_CAP_MICROS);
    assert_eq!(saturation_micros(0, u64::MAX), SIGNAL_CAP_MICROS);
    assert_eq!(saturation_micros(u64::MAX, u64::MAX), SIGNAL_CAP_MICROS);
}

/// The control for the test above: inside the reachable domain the widening changes nothing.
///
/// `signals_for` clamps both arguments to [`SIGNAL_CAP_MICROS`] before they arrive, so the whole
/// reachable domain is `[0, SIGNAL_CAP_MICROS]^2`. The bound is taken from the crate's own limit
/// rather than picked, and it is asserted from both sides: at the bound the answer is unchanged,
/// and only strictly beyond it does the clamp engage. Were this to move, the golden vectors would
/// move with it.
#[test]
fn saturation_is_unchanged_across_the_whole_reachable_domain() {
    // At the bound: the weighted mean of two capped signals is exactly the cap.
    assert_eq!(
        saturation_micros(SIGNAL_CAP_MICROS, SIGNAL_CAP_MICROS),
        SIGNAL_CAP_MICROS
    );

    // Strictly beyond it is where the clamp first has anything to do.
    assert_eq!(
        saturation_micros(SIGNAL_CAP_MICROS + 1, SIGNAL_CAP_MICROS + 1),
        SIGNAL_CAP_MICROS
    );

    // And below it the plain weighted mean stands, truncation included.
    assert_eq!(saturation_micros(1_010_000, 980_000), 1_002_500);
    assert_eq!(saturation_micros(1_000_001, 1_000_000), 1_000_000);
    for participation in [0, 1, 999_999, MULT_SCALE, 2_500_000, SIGNAL_CAP_MICROS] {
        for volume in [0, 1, 999_999, MULT_SCALE, 2_500_000, SIGNAL_CAP_MICROS] {
            let expected = (3 * u128::from(participation) + u128::from(volume)) / 4;
            assert_eq!(
                u128::from(saturation_micros(participation, volume)),
                expected,
                "reachable domain moved at ({participation}, {volume})"
            );
        }
    }
}

/// `EpochRecord` has no private fields, so the terminal epoch is constructible and `epoch + 1` is
/// reachable from outside the crate.
///
/// The release-mode failure is the dangerous one and it is not merely a wrong number: `u64::MAX`
/// wraps to `0`, which then *equals* a census for epoch 0, so the sequence guard waves through a
/// census that follows nothing and derives a record from it. The second case pins the successor
/// guard specifically — a fix that only swaps in `saturating_add` still lets the terminal epoch
/// advance onto itself, because there `expected` and `self.epoch` are the same value.
#[test]
fn terminal_epoch_refuses_to_advance_under_both_profiles() {
    let mut terminal = EpochRecord::bootstrap();
    terminal.epoch = u64::MAX;

    let wrapped_successor = EpochCensus {
        epoch: 0,
        stores: 0,
        owners: 0,
        locked: 0,
    };
    assert!(
        terminal.advance(wrapped_successor).is_err(),
        "a census for epoch 0 does not follow the terminal epoch"
    );

    let itself = EpochCensus {
        epoch: u64::MAX,
        ..wrapped_successor
    };
    assert!(
        terminal.advance(itself).is_err(),
        "the terminal epoch has no successor, least of all itself"
    );

    // The control: an ordinary record still advances, so the guard has not simply closed.
    let mut ordinary = EpochRecord::bootstrap();
    ordinary.epoch = 100;
    assert!(ordinary
        .advance(EpochCensus {
            epoch: 101,
            ..wrapped_successor
        })
        .is_ok());
}

/// The margin multiplies a `u64` requirement by a `u64` margin, which leaves `u128` at the top of
/// the range.
///
/// This one is directional: the margin exists to stop a node landing a mojo short, so a wrap that
/// turns the largest conceivable margin into a *smaller* posted amount fails in precisely the
/// direction the function must never fail in. Unchecked, this returns
/// `18_443_054_724_894_809_705` under `release` — lower than the requirement it was given.
#[test]
fn safety_margin_saturates_instead_of_diverging_by_profile() {
    assert_eq!(apply_safety_margin(u64::MAX, u64::MAX), u64::MAX);
    assert_eq!(apply_safety_margin(u64::MAX, 500), u64::MAX);

    // The control: the documented preset arithmetic is untouched, round-up included.
    assert_eq!(apply_safety_margin(1_036, 100), 1_047);
    assert_eq!(apply_safety_margin(1_036, 0), 1_036);
}
