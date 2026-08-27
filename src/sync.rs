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
/// population. At the plateau, six agreeing responses out of nine gives 99.7% confidence under
/// the stated 80%-honest assumption; the finite-population correction only helps, and at a
/// population of 20 it makes six dishonest responses outright impossible.
///
/// ```
/// use dig_mirror_collateral::sync_sample_plan;
/// let small = sync_sample_plan(12);
/// assert_eq!(small.sample_size, 12); // the whole population
/// assert!(small.advisory_only);
///
/// let plateau = sync_sample_plan(27);
/// assert_eq!(plateau.sample_size, 9);
/// assert_eq!(plateau.agreement_threshold, 6);
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

/// How many of a `sample_size` sample must agree: two thirds, rounded up.
///
/// **The specification is internally inconsistent here and this resolves it deliberately.**
/// Section 9 of the decision writes `threshold(k) = floor(2 * k / 3) + 1` and annotates it
/// `// 6 when k = 9`, but that expression yields **7** at `k = 9`, because `2 * 9 / 3` divides
/// exactly and the `+ 1` then overshoots. The two readings differ only when `k` is a multiple of
/// three, which is precisely the plateau case.
///
/// The value 6 is taken, because it is what the rest of the section is built on: the confidence
/// table computes `P(X >= 6) = 0.0031` for the chosen threshold, and the epoch-8 worked example
/// argues that six dishonest responses cannot be drawn from five dishonest owners. Encoding 7
/// would leave the published 99.7% figure describing a threshold the code does not use, and
/// section 14 says these numbers are fixed so the confidence claim stays *auditable*.
///
/// This is not consensus — the sampled value is recomputable from chain, so a node using the
/// other reading is differently confident rather than forked. Reported upstream regardless.
const fn agreement_threshold(sample_size: u64) -> u64 {
    let two_thirds_rounded_up = (2 * sample_size).div_ceil(3);
    if two_thirds_rounded_up == 0 {
        1
    } else {
        two_thirds_rounded_up
    }
}
