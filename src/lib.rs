//! Deterministic per-epoch collateral requirement for DIG mirror coins.
//!
//! The requirement an advertisement must post to be counted in an epoch is
//!
//! ```text
//! required(n) = max( floor, floor(equilibrium * multiplier(n) / scale) - handicap(n) )
//! ```
//!
//! where `multiplier(n)` is produced by a bang-bang controller reading the previous epoch's
//! chain census, and `handicap(n)` is a bootstrap subsidy that shrinks linearly to zero as the
//! network gains verified owners.
//!
//! # Units
//!
//! Every amount is an integer count of **DIG base units**. DIG is a CAT with `decimals = 3`, so
//! `1 DIG = 1_000` base units and the smallest expressible amount is `0.001 DIG`. A DIG base unit
//! is never called a mojo: a mojo is XCH's base unit, `10^-12` XCH, nine orders of magnitude away,
//! and this is a money path.
//!
//! # Protocol versions
//!
//! The ruleset governing an epoch is selected by [`version_for_epoch`] from the epoch **being
//! computed** — never from what this build happens to be — so an upgrade cannot fork the network
//! mid-rollout. Historical rulesets are permanent, and an epoch governed by an unimplemented
//! version is refused rather than guessed at. See [`version`] for why each of those is a rule.
//!
//! # What this crate is for
//!
//! Two independent implementations of this arithmetic must agree on every epoch, forever. A
//! single differing DIG base unit in one epoch propagates into every later one, because each epoch's
//! census qualifies coins against the previous epoch's requirement. Everything unusual about
//! this crate follows from that one requirement:
//!
//! - **No floating point anywhere.** One ULP of divergence is a fork. `f32` and `f64` do not
//!   appear in the source, and a test in `tests/no_floats.rs` reads the crate's own source to
//!   keep it that way.
//! - **Floor division everywhere**, on non-negative integers, never a language default and never
//!   round-half-up. The single exception is [`apply_safety_margin`], which rounds up and is not
//!   consensus.
//! - **`u128` intermediates and saturating narrowing.** An overflow that panics on one node and
//!   wraps on another is the same fork by another route, so there is no panicking path — not on
//!   the recurrence, and not on any argument the public signatures admit. The widening sits at the
//!   boundary of each public function rather than at the boundary of the values the recurrence
//!   happens to produce, because a caller reaching one of them directly is doing exactly what its
//!   signature invites. `tests/overflow.rs` asserts the saturated answers, so it fails under
//!   `debug` by panic and under `release` by a wrong number.
//! - **An empty network is neutral, not an error.** Both degenerate denominators return exactly
//!   1.0x, so a network with nothing in it holds the price rather than refusing to compute one.
//!
//! # Scope
//!
//! Pure arithmetic and nothing else: no chain reads, no I/O, no async, and no DIG or Chia
//! dependency. The census itself — applying the qualifying rules to real coins — lives in
//! `dig-mirror-coin`, which hands a plain [`EpochCensus`] down into this crate.
//!
//! # Example
//!
//! ```
//! use dig_mirror_collateral::{EpochCensus, EpochRecord};
//!
//! // Epoch 1 depends on nothing: it is the anchor of the recurrence.
//! let epoch1 = EpochRecord::bootstrap();
//! assert_eq!(epoch1.required_per_store_dig_base_units, 1_000); // 1.000 DIG
//!
//! // Epoch 2, from what the chain says about it.
//! let epoch2 = epoch1
//!     .advance(EpochCensus { epoch: 2, stores: 12, owners: 9, locked: 12_120 })
//!     .expect("epoch 2 follows epoch 1");
//! assert_eq!(epoch2.required_per_store_dig_base_units, 1_036); // 1.036 DIG
//! ```

#![forbid(clippy::float_arithmetic)]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod census;
pub mod constants;
pub mod controller;
pub mod error;
pub mod handicap;
pub mod margin;
pub mod record;
pub mod requirement;
pub mod sync;
pub mod version;

pub use census::EpochCensus;
pub use constants::*;
pub use controller::{
    participation_micros, saturation_micros, signals_for, step_multiplier, volume_micros, Band,
    Signals,
};
pub use error::CollateralError;
pub use handicap::handicap_for_owners;
pub use margin::apply_safety_margin;
pub use record::EpochRecord;
pub use requirement::{base_per_store, required_per_store};
pub use sync::{sync_sample_plan, SyncSamplePlan};
pub use version::{
    schedule_is_strictly_ascending, version_for_epoch, version_for_epoch_in, Activation,
    ProtocolVersion, ACTIVATION_SCHEDULE,
};
