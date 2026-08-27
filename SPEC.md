# dig-mirror-collateral — normative specification

This document is the contract. An independent reimplementation in any language, built against this
document and `tests/fixtures/golden_vectors.json` alone, MUST produce byte-identical results for
every epoch, forever.

Normative keywords MUST, MUST NOT, SHOULD and MAY are used in their usual sense.

---

## 1. Units and types

- All collateral amounts are **DIG CAT base units (mojos)**. `1 DIG = 1_000 mojos`; DIG is a CAT
  with `decimals = 3`.
- The multiplier and every saturation signal are fixed-point micros: `1_000_000 == 1.0`.
- Every value in the consensus path is a non-negative integer. Implementations MUST use unsigned
  64-bit values for stored quantities and unsigned 128-bit values for the intermediates named in
  section 8.
- **Every division in the consensus path is floor division on non-negative integers.** No other
  rounding mode appears anywhere in sections 2 through 6. The single exception is the client-side
  safety margin of section 7, which rounds **up** and is not consensus.
- **Floating-point types MUST NOT appear anywhere.** One unit in the last place of divergence
  between two implementations propagates into every later epoch, because each epoch's census
  qualifies coins against the previous epoch's requirement. This is a fork, not a rounding
  nuisance.

## 2. Constants

| name | value | category |
|---|---|---|
| `EQUILIBRIUM_PER_STORE_MOJOS` | `5_000` | load-bearing |
| `MULT_SCALE` | `1_000_000` | arbitrary but fixed |
| `MULT_BOOTSTRAP_MICROS` | `1_000_000` | arbitrary but fixed |
| `MULT_FLOOR_MICROS` | `1_000` | arbitrary but fixed |
| `MULT_CEILING_MICROS` | `1_000_000_000_000` | arbitrary but fixed |
| `DEADBAND_LOW_MICROS` | `950_000` | load-bearing |
| `DEADBAND_HIGH_MICROS` | `1_100_000` | load-bearing |
| `UP_STEP_DENOM` | `8` | load-bearing |
| `DOWN_STEP_DENOM` | `16` | load-bearing |
| `PARTICIPATION_WEIGHT` | `3` | load-bearing |
| `VOLUME_WEIGHT` | `1` | load-bearing |
| `SIGNAL_CAP_MICROS` | `100_000_000` | arbitrary but fixed |
| `HANDICAP_MAX_MOJOS` | `4_000` | load-bearing |
| `HANDICAP_ZERO_AT_OWNERS` | `1_000` | load-bearing |
| `MIN_REQUIRED_PER_STORE_MOJOS` | `1_000` | load-bearing |
| `CENSUS_FINALITY_DEPTH_BLOCKS` | `32` | arbitrary but fixed |
| `SYNC_MAX_SAMPLE` | `9` | arbitrary but fixed, not consensus |
| `SYNC_MIN_POPULATION` | `20` | arbitrary but fixed, not consensus |
| `SYNC_ASSUMED_DISHONEST_DENOM` | `5` | load-bearing for the confidence claim |

**Load-bearing** means a different value changes the economics of the network. **Arbitrary but
fixed** means any value would have worked, but every node MUST agree on the one that was picked.
Neither category may be changed casually; the second buys nothing by changing and forks by doing
so.

## 3. Census input

For each epoch `n` a census yields three non-negative integers:

- `stores(n)` — the count of distinct qualifying `(owner_puzzle_hash, store_id, root)` triples. It
  is an advertisement count, not a store count: one owner publishing two roots for one store id
  contributes two, each paid for in full.
- `owners(n)` — the count of distinct owner puzzle hashes across those triples. It is **not** a
  node count and **not** an operator count. Every surface displaying it MUST say "collateralised
  owners".
- `locked(n)` — the sum, in mojos, of the amounts of the coins selected per triple.

`stores(1) = owners(1) = locked(1) = 0` by definition: no epoch precedes epoch 1, so no coin can
declare it.

A coin that does not meet `required_per_store(n-1)` MUST NOT contribute to any of the three. An
under-collateralised coin is invisible to the controller and MUST NOT be readable as evidence that
the network cannot afford the current requirement. The qualifying rules themselves are specified in
`dig-mirror-coin`; this crate consumes their result.

## 4. Signals

```
participation_micros(n) =
    if stores(n-1) == 0 then MULT_SCALE
    else min(SIGNAL_CAP_MICROS, floor(stores(n) * MULT_SCALE / stores(n-1)))

required_total_prev(n) = stores(n) * required_per_store(n-1)

volume_micros(n) =
    if required_total_prev(n) == 0 then MULT_SCALE
    else min(SIGNAL_CAP_MICROS, floor(locked(n) * MULT_SCALE / required_total_prev(n)))

saturation_micros(n) =
    floor( (PARTICIPATION_WEIGHT * participation_micros(n)
          + VOLUME_WEIGHT        * volume_micros(n))
         / (PARTICIPATION_WEIGHT + VOLUME_WEIGHT) )
```

Both degenerate denominators MUST yield exactly `MULT_SCALE`. An empty network is neutral, never an
error and never a collapse signal.

Participation is a **growth** ratio, never a retention ratio. A retention ratio is bounded above by
`1.0`, so it could only ever signal downward and the multiplier could never rise.

Each signal MUST be clamped to `SIGNAL_CAP_MICROS` **before** any narrowing conversion.

## 5. Controller

```
band(n) = High   if saturation_micros(n) >  DEADBAND_HIGH_MICROS
          Low    if saturation_micros(n) <  DEADBAND_LOW_MICROS
          Inside otherwise

m(n) = High   -> min(MULT_CEILING_MICROS, multiplier(n-1) + floor(multiplier(n-1) / UP_STEP_DENOM))
       Low    -> multiplier(n-1) - floor(multiplier(n-1) / DOWN_STEP_DENOM)   [saturating]
       Inside -> multiplier(n-1)

multiplier_micros(n) = max(MULT_FLOOR_MICROS, m(n))
multiplier_micros(1) = MULT_BOOTSTRAP_MICROS
```

Both dead-band edges are **inclusive**: a saturation of exactly `DEADBAND_LOW_MICROS` or exactly
`DEADBAND_HIGH_MICROS` holds the multiplier flat.

The floor MUST be applied **after** the step, never before and never instead of it.

Exactly three outcomes are possible per epoch. Both step fractions MUST remain strictly smaller
than the dead-band width, which is what rules out oscillation across an edge. The asymmetry between
them is itself normative: the multiplier rises more readily than it falls, because downward is the
direction an attacker benefits from and every step down cheapens every future Sybil identity.

## 6. Handicap and requirement

```
handicap(n) = floor( HANDICAP_MAX_MOJOS
                   * (HANDICAP_ZERO_AT_OWNERS - min(owners(n), HANDICAP_ZERO_AT_OWNERS))
                   / HANDICAP_ZERO_AT_OWNERS )

base(n) = floor(EQUILIBRIUM_PER_STORE_MOJOS * multiplier_micros(n) / MULT_SCALE)

required_per_store(n) = max( MIN_REQUIRED_PER_STORE_MOJOS,
                             saturating_sub(base(n), handicap(n)) )
```

The handicap curve is **linear** and MUST NOT be negative at any owner count. The `min()` on the
owner count is the normative form: it removes the branch in which a subsidy could become a
surcharge.

`EQUILIBRIUM_PER_STORE_MOJOS - HANDICAP_MAX_MOJOS == MIN_REQUIRED_PER_STORE_MOJOS` is a required
identity of the constant set: at zero verified owners the requirement lands exactly on the floor, so
the subsidy is maximal with none of it wasted below the clamp.

At a multiplier of `1.0` the whole schedule collapses to `required = 1_000 + 4 * owners` mojos for
owners below `1_000`.

### Well-foundedness

`required_per_store(n)` depends on the epoch `n-1` census, which qualifies coins against
`required_per_store(n-1)`. This is **not circular**: `required_per_store(1)` depends on nothing, so
the recurrence is well-founded by induction on the epoch number. An implementation that "breaks the
cycle" by counting coins regardless of amount has removed the single most important anti-spam
property of the design.

## 7. Safety margin — client-side, never consensus

```
locked_target = ceil( required_per_store(n) * (BASIS_POINTS_SCALE + margin_bp) / BASIS_POINTS_SCALE )
```

Presets: **1 bp** (0.01%), **100 bp** (1%, the default), **500 bp** (5%).

This is the only ceiling division in the crate. A margin that rounds down can leave a node one mojo
short of the requirement, which is exactly the failure it exists to prevent.

This value MUST NOT enter any census, signal, or record. The controller is deliberately built so
that no supported preset can move the multiplier on its own: with participation neutral, volume
would have to reach `1.40` to leave the dead band.

## 8. Overflow

Implementations MUST compute the following in at least 128 bits, and MUST narrow by saturation
rather than by a fallible or panicking conversion:

| intermediate | bound |
|---|---|
| `stores(n) * MULT_SCALE` | ~4.3e15 |
| `locked(n) * MULT_SCALE` | ~1e21 — exceeds 64 bits, and is why all ratio math is 128-bit |
| `required_total_prev(n)` | ~4.3e24 |
| `EQUILIBRIUM_PER_STORE_MOJOS * multiplier_micros(n)` | ~5e15 |

An overflow that panics on one node and wraps on another is a fork by another route. There MUST be
no panicking path.

`MULT_CEILING_MICROS` is a **representational** saturation bound, not an economic ceiling: reaching
it from `1.0x` requires 118 consecutive maximum up-steps.

## 9. Sync sampling — an optimisation, never authority

```
D_max(N)     = floor(N / SYNC_ASSUMED_DISHONEST_DENOM)
k(N)         = if N < SYNC_MIN_POPULATION then N else min(N, SYNC_MAX_SAMPLE)
threshold(k) = floor(2 * k / 3) + 1     // a strict supermajority; never less than 1
```

`N` is `owners(n)`, the chain-derived count of distinct collateralised owner hashes. There is
exactly one definition of "verified node" in this design and the handicap, the sampling population
and the displayed owner count all share it.

At the plateau `k = 9`, `threshold = 7`, giving **99.97% confidence under the assumption that at
most 20% of the chain-derived population is dishonest**. That assumption travels with the figure
wherever the figure appears: a confidence number without it is not a claim. The finite-population
correction only helps.

`floor(2 * k / 3) + 1` is the strict-supermajority form: it requires *more* than two thirds, where
`ceil(2 * k / 3)` would accept exactly two thirds. The two readings differ only when `k` is a
multiple of three, which is precisely the plateau. The strict form is chosen because failing to
converge is safe here — the sample is advisory and chain is the source of truth, so an over-strict
threshold costs a re-derivation, never a wrong answer.

The `+ 1` also makes the floor of 1 structural: at `k = 0` the literal two-thirds reading yields a
threshold of 0, which reads as *adopt anything, on no evidence*.

For small `k` the strict form demands near-unanimity — `k = 3` requires all three. This is
acceptable **because** populations below `SYNC_MIN_POPULATION` are advisory-only: the node derives
the epoch from chain regardless, so a sample that cannot converge there costs a re-derivation it
was already going to perform.

For `N < SYNC_MIN_POPULATION` the sample is **advisory only**: it is the assumption that fails
rather than the statistics, and the node MUST derive the epoch from chain regardless.

The sampled data is **derived, not authoritative**. Chain is the source of truth. A node that
disagrees with its sample MUST prefer its own computation. The sample buys only the ability to skip
an expensive historical re-derivation; it never buys the right to be wrong.

## 10. Conformance

`tests/fixtures/golden_vectors.json` is the conformance contract. It carries ten epochs — bootstrap,
growth, a 55% participation shock and recovery — with every intermediate for each. An implementation
conforms when it reproduces every field of every epoch from the census inputs alone.

An epoch record MUST retain its inputs and intermediates, not merely its output: divergence between
two implementations has to be auditable, not merely detectable, and floor divisions bite in enough
places that "somewhere in the chain" is not a usable answer.

## 11. Scope

This crate is pure arithmetic. It performs no chain reads, no I/O, and depends on no DIG or Chia
crate. The census (`dig-mirror-coin`), the per-epoch database and the gossip service (`dig-node`),
and the margin UI (`dign`, DIG App) are specified elsewhere.
