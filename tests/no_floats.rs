//! A floating-point value anywhere in this crate is a consensus fork, so the source is read.
//!
//! `#![forbid(clippy::float_arithmetic)]` in `lib.rs` catches float *arithmetic*, but only when
//! clippy runs, and it says nothing about a float that is merely stored, parsed or printed. One
//! ULP of divergence between two implementations propagates into every later epoch, because each
//! epoch's census qualifies coins against the previous epoch's requirement — so this reads the
//! crate's own source and refuses the type outright.

use std::path::Path;

/// Token-ish occurrences of the two float types, in source rather than in prose.
fn float_type_hits(source: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_number, raw) in source.lines().enumerate() {
        let line = raw.trim_start();
        // Doc comments and ordinary comments discuss floats deliberately; code must not use them.
        if line.starts_with("//") {
            continue;
        }
        for needle in ["f32", "f64"] {
            if contains_token(raw, needle) {
                hits.push(format!("line {}: {}", line_number + 1, raw.trim()));
            }
        }
    }
    hits
}

/// Match `needle` only where it is not part of a longer identifier, so `deadbeef32` and a hex
/// literal do not read as a float type.
fn contains_token(haystack: &str, needle: &str) -> bool {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    haystack.match_indices(needle).any(|(index, _)| {
        let before_ok = index == 0 || !haystack[..index].chars().next_back().is_some_and(is_ident);
        let after = index + needle.len();
        let after_ok = !haystack[after..].chars().next().is_some_and(is_ident);
        before_ok && after_ok
    })
}

#[test]
fn no_source_file_mentions_a_float_type() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut files_read = 0usize;

    for entry in std::fs::read_dir(&src).expect("src/ is readable") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        files_read += 1;
        let source = std::fs::read_to_string(&path).expect("source is readable");
        for hit in float_type_hits(&source) {
            offenders.push(format!("{}: {hit}", path.display()));
        }
    }

    assert!(
        files_read >= 9,
        "the guard read only {files_read} source files — it must cover the whole crate, and a \
         guard that silently reads nothing is worse than no guard"
    );
    assert!(
        offenders.is_empty(),
        "floating point in a consensus path forks the network:\n{}",
        offenders.join("\n")
    );
}

/// The guard itself is load-bearing, so it is checked against source it must reject and source it
/// must not. Without this, a scanner that matched nothing at all would look identical to a clean
/// crate.
#[test]
fn the_guard_detects_a_float_and_ignores_a_look_alike() {
    assert!(!float_type_hits("let x: f64 = 1.0;").is_empty());
    assert!(!float_type_hits("fn f(v: f32) -> u64 { 0 }").is_empty());
    assert!(float_type_hits("// f64 is banned here").is_empty());
    assert!(float_type_hits("let deadbeef32 = 1u64;").is_empty());
    assert!(float_type_hits("const XF64: u64 = 1;").is_empty());
}
