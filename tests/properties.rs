//! Properties of the specification, as opposed to observations about one worked table.
//!
//! Each test here pins something that must hold for *every* input, and each corresponds to a
//! failure mode the design exists to prevent. They are separate from the golden vectors on
//! purpose: a golden row that happens to exhibit a property is not the same as the property.

use dig_mirror_collateral::{
    apply_safety_margin, handicap_for_owners, required_per_store, saturation_micros,
    step_multiplier, Band, EpochCensus, EpochRecord, DEADBAND_HIGH_MICROS, DEADBAND_LOW_MICROS,
    DOWN_STEP_DENOM, EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS, HANDICAP_MAX_DIG_BASE_UNITS,
    HANDICAP_ZERO_AT_OWNERS, MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS, MULT_SCALE,
    SAFETY_MARGIN_PRESETS_BP, UP_STEP_DENOM,
};

/// The bootstrap price: the equilibrium price less the full subsidy, 1.000 DIG.
const BOOTSTRAP_PRICE: u64 = EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS - HANDICAP_MAX_DIG_BASE_UNITS;

// The constant half of property 1, checked at compile time because it is decidable there. A
// subsidy at or above the equilibrium price would hand the bootstrap price to the clamp and
// flatten the bottom of the curve, so that gaining an owner did not change the price; a bootstrap
// price sitting on the clamp would mean the clamp, not the curve, had set it. Neither can reach a
// test run, because the build fails first.
const _: () = assert!(HANDICAP_MAX_DIG_BASE_UNITS < EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS);
const _: () = assert!(BOOTSTRAP_PRICE > MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS);

/// Property 1 — the bootstrap price is set by the subsidy curve, never by the clamp.
///
/// At 1.0x with no verified owners the requirement is `EQUILIBRIUM - HANDICAP_MAX`, which is
/// 1.000 DIG. The subsidy must stay strictly *below* the equilibrium price for that to hold: a
/// subsidy at or above it would drop the first epoch onto
/// `MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS` and hand the bootstrap price to the clamp, flattening
/// the bottom of the curve so that gaining an owner did nothing.
///
/// This property was previously written as the identity
/// `EQUILIBRIUM - HANDICAP_MAX == MIN_REQUIRED`, which held only while the clamp was 1.000 DIG.
/// Lowering the clamp to a single base unit — so that it stops swallowing the multiplier's range —
/// separates the two concerns: the curve sets the price, the clamp only forbids zero. Three
/// constants still move together, so this is asserted rather than left as an arithmetic
/// coincidence a later edit could break silently.
#[test]
fn the_bootstrap_price_comes_from_the_curve_and_not_the_clamp() {
    let bootstrap_price = BOOTSTRAP_PRICE;
    assert_eq!(bootstrap_price, 1_000, "the bootstrap price is 1.000 DIG");

    // And the composed function agrees, so the property is one of the code and not only of the
    // constants.
    let epoch1 = EpochRecord::bootstrap();
    assert_eq!(epoch1.handicap_dig_base_units, HANDICAP_MAX_DIG_BASE_UNITS);
    assert_eq!(epoch1.required_per_store_dig_base_units, bootstrap_price);
    assert_eq!(
        epoch1.base_price_dig_base_units - epoch1.handicap_dig_base_units,
        bootstrap_price,
        "the bootstrap price must be reached by the subsidy, not imposed by the clamp on top of it"
    );
}

/// Property 2 — no safety-margin preset can move the multiplier in a stable network.
///
/// This is the ratchet the design exists to prevent, and it is the one that fails *silently*: a
/// margin every operator runs makes the volume signal read permanently above 1.0, and a
/// controller that reads that as demand raises the price every epoch forever, compounding.
///
/// The fixture is built so the margin is the *only* thing that varies: the advertisement count is
/// held exactly constant, so participation reads exactly 1.0 and the whole deviation in
/// saturation is attributable to the preset. Twenty epochs, because one epoch of no movement
/// cannot distinguish "does not ratchet" from "has not ratcheted yet".
#[test]
fn no_safety_margin_preset_moves_the_multiplier() {
    const STORES: u64 = 500;
    const OWNERS: u64 = 1_200; // past the handicap, so the subsidy cannot mask a drift
    const EPOCHS: u64 = 20;

    for margin_bp in SAFETY_MARGIN_PRESETS_BP {
        let mut record = stable_seed(STORES, OWNERS);
        let start_multiplier = record.multiplier_micros;

        for epoch in (record.epoch + 1)..=(record.epoch + EPOCHS) {
            let posted = apply_safety_margin(record.required_per_store_dig_base_units, margin_bp);
            record = record
                .advance(EpochCensus {
                    epoch,
                    stores: STORES,
                    owners: OWNERS,
                    locked: STORES * posted,
                })
                .expect("consecutive");

            let signals = record.signals.expect("derived epoch has signals");

            // The control: the margin really is visible in the signal it corrupts. Without this
            // the test could pass because the margin had no effect at all, which would prove
            // nothing about the dead band.
            assert_eq!(
                signals.participation_micros, MULT_SCALE,
                "the fixture holds participation at exactly neutral so the margin is isolated"
            );
            if margin_bp > 0 {
                assert!(
                    signals.volume_micros > MULT_SCALE,
                    "margin {margin_bp} bp must actually show up in the volume signal, else this \
                     test proves nothing"
                );
            }

            assert_eq!(
                record.band,
                Some(Band::Inside),
                "margin {margin_bp} bp read as a demand signal at epoch {epoch}"
            );
            assert_eq!(
                record.multiplier_micros, start_multiplier,
                "margin {margin_bp} bp ratcheted the multiplier by epoch {epoch}"
            );
        }
    }
}

/// Property 2b — and the largest preset has real headroom, not a one-mojo escape.
///
/// The dead band's upper edge is the single most load-bearing constant here. Placing it at the
/// largest preset exactly would let any noise above that preset fire the ratchet, so the margin
/// between the two is asserted directly.
#[test]
fn the_dead_band_has_headroom_above_the_largest_preset() {
    let largest = SAFETY_MARGIN_PRESETS_BP
        .iter()
        .copied()
        .max()
        .expect("presets are non-empty");

    // Saturation when participation is neutral and volume carries the largest preset.
    let volume = MULT_SCALE + largest * MULT_SCALE / 10_000;
    let saturation = saturation_micros(MULT_SCALE, volume);

    assert!(
        saturation < DEADBAND_HIGH_MICROS,
        "the largest preset must sit inside the band, not on its edge"
    );
    assert!(
        DEADBAND_HIGH_MICROS - saturation > (DEADBAND_HIGH_MICROS - DEADBAND_LOW_MICROS) / 4,
        "the headroom above the largest preset must be a real margin, not a rounding accident"
    );
}

/// Property 3 — neither step can carry the multiplier across the dead band.
///
/// This is what rules out oscillation: a multiplier arriving from outside the band cannot be
/// flung past it, so the controller settles rather than hunting across an edge. Both steps are
/// fractions of the previous multiplier and the band is a fraction of 1.0, so the comparison is
/// scale-free and is made at the fixed-point scale.
#[test]
fn neither_step_can_cross_the_dead_band() {
    let band_width = DEADBAND_HIGH_MICROS - DEADBAND_LOW_MICROS;
    let up_step = MULT_SCALE / UP_STEP_DENOM;
    let down_step = MULT_SCALE / DOWN_STEP_DENOM;

    assert!(
        up_step < band_width,
        "up-step {up_step} must be strictly smaller than the band width {band_width}"
    );
    assert!(
        down_step < band_width,
        "down-step {down_step} must be strictly smaller than the band width {band_width}"
    );

    // The asymmetry itself, which is load-bearing independently of the magnitudes: the direction
    // an attacker wants is the slower one.
    assert!(
        down_step < up_step,
        "the controller must rise more readily than it falls"
    );
}

/// Property 3b — the step behaves as a step: exactly three outcomes, and each is bounded.
#[test]
fn the_controller_has_exactly_three_outcomes() {
    let prev = 1_600_000;

    assert_eq!(
        step_multiplier(prev, DEADBAND_HIGH_MICROS + 1),
        prev + prev / UP_STEP_DENOM
    );
    assert_eq!(
        step_multiplier(prev, DEADBAND_LOW_MICROS - 1),
        prev - prev / DOWN_STEP_DENOM
    );

    // Both edges are inside the band, which is what makes the band contain 1.00 through 1.05.
    assert_eq!(step_multiplier(prev, DEADBAND_HIGH_MICROS), prev);
    assert_eq!(step_multiplier(prev, DEADBAND_LOW_MICROS), prev);
    assert_eq!(step_multiplier(prev, MULT_SCALE), prev);
}

/// Property 4 — the handicap is a subsidy at every owner count and never becomes a surcharge.
///
/// A negative handicap would invert the sign of the whole bootstrap term: instead of paying less
/// during growth, a mature network would pay more than the equilibrium price. Checked past the
/// zero point, well past it, and at the representational extreme.
#[test]
fn the_handicap_never_inverts() {
    for owners in 0..=(HANDICAP_ZERO_AT_OWNERS * 3) {
        let handicap = handicap_for_owners(owners);
        assert!(
            handicap <= HANDICAP_MAX_DIG_BASE_UNITS,
            "handicap at {owners} owners exceeds the maximum subsidy"
        );
        if owners >= HANDICAP_ZERO_AT_OWNERS {
            assert_eq!(handicap, 0, "the subsidy must be gone at {owners} owners");
        }
        assert!(
            required_per_store(MULT_SCALE, owners) <= EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS,
            "at 1.0x the requirement can never exceed the equilibrium price"
        );
    }

    assert_eq!(handicap_for_owners(u64::MAX), 0);
    assert_eq!(
        required_per_store(MULT_SCALE, u64::MAX),
        EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS
    );
}

/// Property 4b — the subsidy shrinks monotonically, so gaining an owner never lowers the price.
///
/// A non-monotone curve would let an operator make the network cheaper by joining it, which is a
/// direct incentive to Sybil rather than a cost of it.
#[test]
fn the_requirement_never_falls_as_owners_are_gained() {
    let mut previous = required_per_store(MULT_SCALE, 0);
    for owners in 1..=(HANDICAP_ZERO_AT_OWNERS + 50) {
        let current = required_per_store(MULT_SCALE, owners);
        assert!(
            current >= previous,
            "gaining the {owners}th owner lowered the requirement {previous} -> {current}"
        );
        previous = current;
    }
}

/// The rounding mode is part of the contract, not an implementation detail.
///
/// Two of the golden values are truncations that round-half-up would move by one micro. That is
/// enough to make two records hash differently forever, so it is asserted directly against the
/// alternative rather than only implied by the fixture.
#[test]
fn floor_division_is_the_rounding_mode() {
    let epoch3 = EpochRecord::bootstrap()
        .advance(EpochCensus {
            epoch: 2,
            stores: 12,
            owners: 9,
            locked: 12_120,
        })
        .and_then(|r| {
            r.advance(EpochCensus {
                epoch: 3,
                stores: 30,
                owners: 22,
                locked: 31_410,
            })
        })
        .expect("consecutive");

    let volume = epoch3.signals.expect("derived").volume_micros;
    assert_eq!(volume, 1_010_617, "floor");
    assert_ne!(volume, 1_010_618, "round-half-up would produce this");

    // The exact quotient, reconstructed: 31_410 * 1e6 / 31_080 has a non-zero remainder, so the
    // two rounding modes genuinely differ here and this is not a vacuous comparison.
    let numerator: u128 = 31_410 * u128::from(MULT_SCALE);
    let denominator: u128 = 31_080;
    assert_ne!(
        numerator % denominator,
        0,
        "the fixture value must be a real truncation for this test to distinguish anything"
    );
}

/// A stable, mature network at 1.0x, built so its own consistency is asserted rather than assumed.
fn stable_seed(stores: u64, owners: u64) -> EpochRecord {
    let mut record = EpochRecord::bootstrap();
    record.epoch = 100;
    record.census = EpochCensus {
        epoch: 100,
        stores,
        owners,
        locked: stores * required_per_store(MULT_SCALE, owners),
    };
    record.multiplier_micros = MULT_SCALE;
    record.handicap_dig_base_units = handicap_for_owners(owners);
    record.base_price_dig_base_units = EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS;
    record.required_per_store_dig_base_units = required_per_store(MULT_SCALE, owners);

    assert_eq!(
        record.required_per_store_dig_base_units, EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS,
        "the seed is past the handicap, so its requirement is the equilibrium price"
    );
    record
}
