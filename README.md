# dig-mirror-collateral

How much DIG a mirror-coin advertisement must lock to be counted in an epoch.

```
required(n) = max( floor, equilibrium x multiplier(n) - handicap(n) )
```

The multiplier comes from a bang-bang controller reading the previous epoch's on-chain census; the
handicap is a bootstrap subsidy that shrinks linearly to zero as the network gains verified owners.
An operator joins for **1.000 DIG** in the first epoch and pays the **5.000 DIG** equilibrium once a
thousand collateralised owners exist.

```rust
use dig_mirror_collateral::{EpochCensus, EpochRecord};

let epoch1 = EpochRecord::bootstrap();
assert_eq!(epoch1.required_per_store_dig_base_units, 1_000); // 1.000 DIG

let epoch2 = epoch1
    .advance(EpochCensus { epoch: 2, stores: 12, owners: 9, locked: 12_120 })
    .unwrap();
assert_eq!(epoch2.required_per_store_dig_base_units, 1_036); // 1.036 DIG
```

## Why this crate looks the way it does

Two independent implementations of this arithmetic must agree on every epoch, forever. A single
differing DIG base unit propagates into every later epoch, because each epoch's census qualifies coins
against the previous epoch's requirement. Everything unusual here follows from that:

- **No floating point.** One ULP of divergence is a fork. A test reads the crate's own source to
  keep it that way.
- **Floor division everywhere**, never a language default. The one exception is the client-side
  safety margin, which rounds up and is not consensus.
- **128-bit intermediates, saturating narrowing, no panicking path.** An overflow that panics on one
  node and wraps on another is the same fork by another route.
- **An empty network is neutral, not an error.**
- **No dependencies beyond `serde` and `thiserror`** — no DIG crate, no Chia crate, no I/O, no async
  runtime. That is what makes it reachable from every layer above.

## Conformance

[`tests/fixtures/golden_vectors.json`](tests/fixtures/golden_vectors.json) is the cross-language
contract: ten epochs covering bootstrap, growth, a 55% participation shock and recovery, with every
intermediate. A reimplementation in any language conforms when it reproduces every field from the
census inputs alone.

## What is not here

Reading the chain (`dig-mirror-coin`), the per-epoch database and gossip (`dig-node`), and the
margin UI (`dign`, DIG App). This crate is the arithmetic and nothing else.

## Reference

[`SPEC.md`](SPEC.md) is normative, and derives from the decision on
[`DIG-Network/dig_ecosystem#3173`](https://github.com/DIG-Network/dig_ecosystem/issues/3173).

Where this crate departs from that decision, the departure is recorded in the source at its point
of use rather than collected in `SPEC.md` — a divergence is only useful next to the expression it
concerns. The sample-agreement threshold in [`src/sync.rs`](src/sync.rs) is the worked example: it
keeps the formula from section 9 and corrects the annotation beside it, with the reasoning for
which of the two was wrong.

## License

MIT
