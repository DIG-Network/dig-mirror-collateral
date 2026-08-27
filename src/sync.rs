//! How many peers to sample when adopting an epoch history, and when not to trust the answer.
//!
//! None of this is consensus. The sampled value is recomputable from the chain, so a node using
//! different numbers here is not forked, only differently confident. The sample buys exactly one
//! thing: the ability to skip an expensive historical re-derivation. It never buys the right to
//! be wrong — a node that disagrees with its sample MUST prefer its own computation.

use serde::{Deserialize, Serialize};

use crate::constants::{SYNC_ASSUMED_DISHONEST_DENOM, SYNC_MAX_SAMPLE, SYNC_MIN_POPULATION};

/// A sampling plan for one epoch, against a chain-derived population.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncSamplePlan {
    /// The population the sample is drawn from: the count of distinct collateralised owner
    /// hashes at the census height. Because it is chain-derived rather than assumed, the sample
    /// is drawn from a known finite population — and hearing from more distinct owners than the
    /// chain says exist is not noise, it is a detectable lie.
    pub population: u64,

    /// How many distinct owners to sample.
    pub sample_size: u64,

    /// How many must agree before the sampled history may be adopted.
    pub agreement_threshold: u64,

    /// The most dishonest owners the confidence claim assumes the population can contain.
    pub max_assumed_dishonest: u64,

    /// When true the sample informs but never decides: the node derives from chain regardless.
    ///
    /// Set below [`SYNC_MIN_POPULATION`], where it is the *assumption* that fails rather than
    /// the statistics — with a population of three, one adversarial owner is 33% of the network,
    /// well past the 20% every confidence number here is conditional on.
    pub advisory_only: bool,
}

/// The sampling plan for a chain-derived population of `population` collateralised owners.
///
/// The sample size plateaus at [`SYNC_MAX_SAMPLE`] and never grows, because the hypergeometric
/// tail is bounded above by the binomial one and the binomial one does not depend on the
/// population. At the plateau, seven agreeing responses out of nine gives 99.97% confidence
/// **under the assumption that at most 20% of the chain-derived population is dishonest** — the
/// figure is meaningless without that assumption beside it. The finite-population correction only
/// helps, and at a population of 20 it makes seven dishonest responses outright impossible.
///
/// ```
/// use dig_mirror_collateral::sync_sample_plan;
/// let small = sync_sample_plan(12);
/// assert_eq!(small.sample_size, 12); // the whole population
/// assert!(small.advisory_only);
///
/// let plateau = sync_sample_plan(27);
/// assert_eq!(plateau.sample_size, 9);
/// assert_eq!(plateau.agreement_threshold, 7);
/// assert!(!plateau.advisory_only);
/// ```
#[must_use]
pub fn sync_sample_plan(population: u64) -> SyncSamplePlan {
    let advisory_only = population < SYNC_MIN_POPULATION;
    let sample_size = if advisory_only {
        population
    } else {
        population.min(SYNC_MAX_SAMPLE)
    };
    SyncSamplePlan {
        population,
        sample_size,
        agreement_threshold: agreement_threshold(sample_size),
        max_assumed_dishonest: population / SYNC_ASSUMED_DISHONEST_DENOM,
        advisory_only,
    }
}

/// How many of a `sample_size` sample must agree: a strict two-thirds supermajority.
///
/// `threshold(k) = floor(2 * k / 3) + 1`, clamped to at least 1. This is the recognised
/// strict-supermajority form: `ceil(2 * k / 3)` would accept *exactly* two thirds, and the two
/// readings differ precisely when `k` is a multiple of three — which is the plateau case `k = 9`.
///
/// Section 9 of the decision on `DIG-Network/dig_ecosystem#3173` originally annotated the formula
/// `// 6 when k = 9`, which the expression contradicts. The ruling on that issue kept the formula
/// and corrected the documentation, because failing to converge is safe here: the sample is
/// advisory and chain is the source of truth, so an over-strict threshold costs a re-derivation
/// rather than a wrong answer. At the plateau the threshold is 7 of 9, giving 99.97% confidence
/// under the assumption that at most 20% of the population is dishonest.
///
/// The floor of 1 is load-bearing and here it is structural: the `+ 1` makes `k = 0` yield 1
/// rather than 0, where 0 would read as *adopt anything, on no evidence*. A future
/// simplification to `ceil(2 * k / 3)` would silently reintroduce that, so the degenerate case is
/// pinned by test rather than left to the reader.
///
/// For small `k` the strict form demands near-unanimity — `k = 3` needs all three. That is
/// acceptable **because** populations below [`SYNC_MIN_POPULATION`] are advisory-only: a node
/// there derives the epoch from chain regardless, so a sample that cannot converge costs nothing.
const fn agreement_threshold(sample_size: u64) -> u64 {
    // Floor division, then `+ 1`: strictly more than two thirds, and never zero.
    (2 * sample_size) / 3 + 1
}
