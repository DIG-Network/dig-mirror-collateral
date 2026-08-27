//! The census input: what the chain says about an epoch.
//!
//! This crate does not read the chain. A caller — in practice `dig-mirror-coin` — applies the
//! qualifying rules C1 through C9 of the specification and hands the three resulting integers
//! down here. That split is what keeps this crate at 00-foundation with no DIG dependency.

use serde::{Deserialize, Serialize};

/// The three chain-derived quantities the controller consumes for one epoch.
///
/// Every field counts only *qualifying* units. In particular an under-collateralised coin is
/// invisible: it contributes to none of these three, so it can never be read as evidence that
/// the network cannot afford the current requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochCensus {
    /// The epoch this census describes, one-based.
    pub epoch: u64,

    /// The count of distinct qualifying `(owner, store, root)` triples.
    ///
    /// Named for continuity with the epic; it is an advertisement count. One owner publishing
    /// two roots for one store id contributes two, each paid for in full.
    pub stores: u64,

    /// The count of distinct owner puzzle hashes across those qualifying triples.
    ///
    /// This is *not* a node count and *not* an operator count: one operator may hold many owner
    /// hashes, and one owner hash may back many nodes. Every surface that displays it must say
    /// "collateralised owners", never "nodes".
    pub owners: u64,

    /// The sum, in DIG mojos, of the amounts of the coins selected per triple by rule C9.
    pub locked: u64,
}

impl EpochCensus {
    /// The census of epoch 1, which is empty by definition: no epoch precedes it, so no coin can
    /// yet declare it.
    #[must_use]
    pub const fn bootstrap() -> Self {
        Self {
            epoch: 1,
            stores: 0,
            owners: 0,
            locked: 0,
        }
    }
}
