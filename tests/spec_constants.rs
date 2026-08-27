//! The constant table of `SPEC.md` section 2, checked against the constants themselves.
//!
//! The table is normative: an independent implementation is built from it rather than from this
//! source. Hand-maintained, it can drift from the crate silently, and a drifted constant is not a
//! documentation nit — it is two implementations computing different requirements forever, which
//! is the one failure this crate exists to prevent.
//!
//! In the spirit of `no_floats.rs`, the guard reads the specification itself rather than trusting
//! a second copy of the numbers.

use dig_mirror_collateral::*;

/// Every `(name, value)` row the specification is expected to carry, bound to the real constant.
///
/// The pairing is what the test cannot derive — Rust has no reflection over constant names — so it
/// is written once here and the row count is asserted below. A row added to the table without a
/// line added here fails on the count rather than passing unchecked.
fn constants() -> Vec<(&'static str, u64)> {
    vec![
        ("EQUILIBRIUM_PER_STORE_MOJOS", EQUILIBRIUM_PER_STORE_MOJOS),
        ("MULT_SCALE", MULT_SCALE),
        ("MULT_BOOTSTRAP_MICROS", MULT_BOOTSTRAP_MICROS),
        ("MULT_FLOOR_MICROS", MULT_FLOOR_MICROS),
        ("MULT_CEILING_MICROS", MULT_CEILING_MICROS),
        ("DEADBAND_LOW_MICROS", DEADBAND_LOW_MICROS),
        ("DEADBAND_HIGH_MICROS", DEADBAND_HIGH_MICROS),
        ("UP_STEP_DENOM", UP_STEP_DENOM),
        ("DOWN_STEP_DENOM", DOWN_STEP_DENOM),
        ("PARTICIPATION_WEIGHT", PARTICIPATION_WEIGHT),
        ("VOLUME_WEIGHT", VOLUME_WEIGHT),
        ("SIGNAL_CAP_MICROS", SIGNAL_CAP_MICROS),
        ("HANDICAP_MAX_MOJOS", HANDICAP_MAX_MOJOS),
        ("HANDICAP_ZERO_AT_OWNERS", HANDICAP_ZERO_AT_OWNERS),
        ("MIN_REQUIRED_PER_STORE_MOJOS", MIN_REQUIRED_PER_STORE_MOJOS),
        ("CENSUS_FINALITY_DEPTH_BLOCKS", CENSUS_FINALITY_DEPTH_BLOCKS),
        ("SYNC_MAX_SAMPLE", SYNC_MAX_SAMPLE),
        ("SYNC_MIN_POPULATION", SYNC_MIN_POPULATION),
        ("SYNC_ASSUMED_DISHONEST_DENOM", SYNC_ASSUMED_DISHONEST_DENOM),
    ]
}

/// The rows of the section-2 table, as `(name, value)` parsed from the specification.
///
/// Scanned line by line and stopped at the next heading, so the parser never reaches into a later
/// section and mistakes one of its tables for the constant table.
fn spec_rows() -> Vec<(String, u64)> {
    let spec = include_str!("../SPEC.md");
    let section = spec
        .split("## 2. Constants")
        .nth(1)
        .expect("SPEC.md has a section 2");

    section
        .lines()
        .take_while(|line| !line.starts_with("## "))
        .filter_map(|line| {
            let mut cells = line.trim().strip_prefix('|')?.split('|');
            let name = cells.next()?.trim().strip_prefix('`')?.strip_suffix('`')?;
            let value = cells.next()?.trim().strip_prefix('`')?.strip_suffix('`')?;
            let value = value.replace('_', "").parse::<u64>().ok()?;
            Some((name.to_owned(), value))
        })
        .collect()
}

/// Each constant holds exactly the value the specification publishes for it.
#[test]
fn spec_constant_table_matches_the_constants() {
    let rows = spec_rows();
    for (name, actual) in constants() {
        let (_, documented) = rows
            .iter()
            .find(|(row_name, _)| row_name == name)
            .unwrap_or_else(|| panic!("SPEC.md section 2 has no row for `{name}`"));
        assert_eq!(
            *documented, actual,
            "SPEC.md publishes {name} = {documented}, the crate defines {actual}"
        );
    }
}

/// The table carries no row the check above never looked at.
///
/// Without this the guard is one-directional: a constant renamed, removed, or added in the
/// specification alone would pass, because the loop only visits names it already knows.
#[test]
fn spec_constant_table_has_no_unchecked_rows() {
    let rows = spec_rows();
    let known = constants();

    assert_eq!(
        rows.len(),
        known.len(),
        "SPEC.md section 2 publishes {} constants and this test binds {} — add the new row to \
         `constants()` so that it is actually checked",
        rows.len(),
        known.len()
    );

    for (name, _) in &rows {
        assert!(
            known.iter().any(|(known_name, _)| known_name == name),
            "SPEC.md section 2 publishes `{name}`, which no constant in this test is bound to"
        );
    }
}

/// The parser is not vacuous: it finds the table, and it reads real values out of it.
///
/// A `filter_map` that silently matched nothing would make both tests above pass over an empty
/// list, which is exactly how a guard like this rots into a no-op.
#[test]
fn spec_table_parser_is_not_vacuous() {
    let rows = spec_rows();
    assert!(
        rows.len() >= 19,
        "parsed only {} rows from SPEC.md section 2 — the parser, not the table, is likely wrong",
        rows.len()
    );
    assert_eq!(
        rows.iter()
            .find(|(name, _)| name == "DEADBAND_HIGH_MICROS")
            .map(|(_, value)| *value),
        Some(1_100_000),
        "the parser did not read a known row correctly"
    );
}
