# dig-mirror-collateral — normative specification

This document is the contract. An independent reimplementation in any language, built against this
document and the per-version fixtures in `tests/fixtures/` alone, MUST produce byte-identical
results for every epoch, forever.

Normative keywords MUST, MUST NOT, SHOULD and MAY are used in their usual sense.

Sections 3 through 8 specify **protocol version 1**. Section 2a specifies how a version is selected
and what an implementation MUST do about one it does not have.

---

## 1. Units and types

- All collateral amounts are **DIG base units**. DIG is a CAT with `decimals = 3`, so
  `1 DIG = DIG_BASE_UNITS_PER_DIG = 1_000` base units and the smallest expressible amount is
  **0.001 DIG**.
- A DIG base unit MUST NOT be called a *mojo*. A mojo is XCH's base unit, `10^-12` XCH; a DIG base
  unit is `10^-3` DIG. The two differ by nine orders of magnitude, and every amount specified here
  is on a money path.
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
| `DIG_BASE_UNITS_PER_DIG` | `1_000` | arbitrary but fixed (the CAT denomination) |
| `EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS` | `5_000` | load-bearing |
| `MULT_SCALE` | `1_000_000` | arbitrary but fixed |
| `MULT_BOOTSTRAP_MICROS` | `1_000_000` | arbitrary but fixed |
| `MULT_FLOOR_MICROS` | `20_000` | load-bearing |
| `MULT_CEILING_MICROS` | `1_000_000_000_000` | arbitrary but fixed |
| `DEADBAND_LOW_MICROS` | `950_000` | load-bearing |
| `DEADBAND_HIGH_MICROS` | `1_100_000` | load-bearing |
| `UP_STEP_DENOM` | `8` | load-bearing |
| `DOWN_STEP_DENOM` | `16` | load-bearing |
| `PARTICIPATION_WEIGHT` | `3` | load-bearing |
| `VOLUME_WEIGHT` | `1` | load-bearing |
| `SIGNAL_CAP_MICROS` | `100_000_000` | arbitrary but fixed |
| `HANDICAP_MAX_DIG_BASE_UNITS` | `4_000` | load-bearing |
| `HANDICAP_ZERO_AT_OWNERS` | `1_000` | load-bearing |
| `MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS` | `1` | load-bearing |
| `CENSUS_FINALITY_DEPTH_BLOCKS` | `32` | arbitrary but fixed |
| `SYNC_MAX_SAMPLE` | `9` | arbitrary but fixed, not consensus |
| `SYNC_MIN_POPULATION` | `20` | arbitrary but fixed, not consensus |
| `SYNC_ASSUMED_DISHONEST_DENOM` | `5` | load-bearing for the confidence claim |

**Load-bearing** means a different value changes the economics of the network. **Arbitrary but
fixed** means any value would have worked, but every node MUST agree on the one that was picked.
Neither category may be changed casually; the second buys nothing by changing and forks by doing
so.

`MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS` is **one base unit** and MUST NOT be raised to act as a
price floor. It is applied after the multiplier, so any larger value collapses every multiplier
below `value / EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS` onto a single price. At `1_000` that was every
multiplier under `0.200x`, which made `MULT_FLOOR_MICROS` unreachable and the whole bottom of the
stated multiplier range indistinguishable. The lever for how far a contracting network price
may fall is `MULT_FLOOR_MICROS`.

`MULT_FLOOR_MICROS` is `20_000` (`0.020x`), and the two floors are load-bearing in **different
regimes**. Where the handicap has fully decayed the multiplier floor alone sets the price, so a
per-store requirement at the floor is `0.100` DIG. Where the handicap is at its maximum it exceeds
the scaled price at any multiplier near the floor, so `MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS` sets
the price instead, and a bootstrapping network at the floor pays `0.001` DIG. An implementation
that reproduces one of those two figures but not the other has applied the wrong floor.

The value is `0.020x` rather than a lower bound because the floor state must retain a nonzero cost
per counted advertisement. The census is the controller's only input, and a floor that priced an
identity near zero would make those inputs forgeable in exactly the deeply contracted state that
reaching the floor implies. `0.020x` is arbitrary within roughly `0.010x`-`0.050x`; what is
load-bearing is that `MULT_FLOOR_MICROS * EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS / MULT_SCALE`
remains well above `MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS`.

## 2a. Protocol versions

The model is a recurrence, so a rule change is never a local edit: verifying the present means
replaying from genesis with each epoch recomputed under the rules in force **at that epoch**.

An **activation schedule** maps each protocol version to the first epoch it governs. Rows MUST
ascend strictly in both activation epoch and version, and the first row MUST govern epoch 1. An
implementation MUST reject a schedule that violates either requirement rather than select from it:
the selection rule below reads the *last* row that has activated, so an unordered row does not
produce an error, it produces a wrong ruleset for a range of epochs.

```
version(n) = the version of the last schedule row with first_epoch <= n
```

- The version MUST be selected from the **epoch being computed**, and from nothing else. An
  implementation MUST NOT select rules by its own build version, by wall-clock time, or by
  configuration. Selecting by node version would make upgraded and un-upgraded nodes compute
  different requirements for the same epoch during any rollout, which is the fork this design
  exists to prevent.
- An activation `first_epoch` is the **first epoch computed under** that version: the new rules
  apply *at* the activation epoch, not from the epoch after it.
- The recurrence therefore **crosses** the boundary. `required_per_store(first_epoch)` is computed
  under the new version but consumes `required_per_store(first_epoch - 1)`, computed under the
  previous one. A new ruleset takes the old ruleset output as its seed and MUST NOT recompute it.
- **Every historical ruleset MUST remain implemented, permanently.** Versions are never deprecated
  and never removed. An implementation that drops a retired ruleset can no longer derive its own
  current state, because the replay from genesis has no rules for the early epochs.
- Each epoch record MUST carry the version that computed it, and that version MUST travel with the
  record over the wire, so a disagreement about which rules applied is a named mismatch rather than
  an unexplained difference between two numbers.
- An unknown version MUST be **representable**: a record tagged with a future version MUST parse, so
  that an implementation can name the version it lacks.

### Failing closed

An implementation reaching an epoch governed, **by the schedule it carries**, by a version it does
not implement MUST refuse, and MUST report that version. It MUST NOT fall back to its newest known
ruleset and MUST NOT extrapolate.

The qualifier is load-bearing and bounds what this refusal can cover. An implementation selects the
version from its own schedule, so it can only refuse a version its schedule names. An implementation
predating a new version carries neither the row nor the rules: it computes the activated epoch under
the last version it knows, returning a plausible number with no refusal available to it, because a
schedule cannot name a version invented after it shipped. **The two refusals therefore cover
different cases**: this one covers an implementation that has the schedule row and not the rules,
and the record refusal below covers a record arriving from a peer that is ahead. Neither covers an
implementation that has not been upgraded at all, which is out of scope here (section 11) and is
addressed only by the activation lead time required below.

Falling back is the dangerous branch precisely because it appears to work: the node computes a
plausible requirement, silently disagrees with the network, and, since a coin below the real
requirement is simply not counted (section 3), the operator stores stop earning while every surface
reports success. A refusal is visible; a wrong answer is not.

An implementation MUST likewise refuse to extend a record whose own version it does not implement:
continuing would substitute its own arithmetic for a seed it cannot reproduce.

An activation epoch is a deadline for every operator, so a new version SHOULD be scheduled with
enough lead time for the network to upgrade. Epochs are seven days, which makes that tractable; a
short lead time is not recoverable.

### The current schedule

| version | first epoch | status |
|---|---|---|
| 1 | 1 | current |

## 3. Census input

For each epoch `n` a census yields three non-negative integers:

- `stores(n)` — the count of distinct qualifying `(owner_puzzle_hash, store_id, root)` triples. It
  is an advertisement count, not a store count: one owner publishing two roots for one store id
  contributes two, each paid for in full.
- `owners(n)` — the count of distinct owner puzzle hashes across those triples. It is **not** a
  node count and **not** an operator count. Every surface displaying it MUST say "collateralised
  owners".
- `locked(n)` — the sum, in DIG base units, of the amounts of the coins selected per triple.

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
handicap(n) = floor( HANDICAP_MAX_DIG_BASE_UNITS
                   * (HANDICAP_ZERO_AT_OWNERS - min(owners(n), HANDICAP_ZERO_AT_OWNERS))
                   / HANDICAP_ZERO_AT_OWNERS )

base(n) = floor(EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS * multiplier_micros(n) / MULT_SCALE)

required_per_store(n) = max( MIN_REQUIRED_PER_STORE_DIG_BASE_UNITS,
                             saturating_sub(base(n), handicap(n)) )
```

The handicap curve is **linear** and MUST NOT be negative at any owner count. The `min()` on the
owner count is the normative form: it removes the branch in which a subsidy could become a
surcharge.

`HANDICAP_MAX_DIG_BASE_UNITS < EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS` is a required property of the
constant set. At a multiplier of `1.0` with zero verified owners the requirement is
`EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS - HANDICAP_MAX_DIG_BASE_UNITS`, which is `1_000` base units
(1.000 DIG). That is the **bootstrap price**, and it MUST be produced by the subsidy curve rather
than by the clamp: a subsidy at or above the equilibrium price would hand the bootstrap price to the
clamp and flatten the bottom of the curve, so that gaining an owner did not change the price.

The two guards in `required_per_store` are independent and MUST NOT be conflated. The clamp forbids
a requirement of zero; the saturating subtraction stops a subsidy larger than the base price from
wrapping into an enormous requirement. Both bind only in a contracting network, and no epoch of the
section 10 vectors reaches either.

At a multiplier of `1.0` the whole schedule collapses to `required = 1_000 + 4 * owners` base units
for owners below `1_000`.

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

This is the only ceiling division in the crate. A margin that rounds down can leave a node one DIG
base unit short of the requirement, which is exactly the failure it exists to prevent.

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
| `EQUILIBRIUM_PER_STORE_DIG_BASE_UNITS * multiplier_micros(n)` | ~5e15 |
| `PARTICIPATION_WEIGHT * participation_micros(n) + VOLUME_WEIGHT * volume_micros(n)` | ~4e8 within the recurrence, where both signals are already clamped to `SIGNAL_CAP_MICROS` |
| `required_per_store(n) * (BASIS_POINTS_SCALE + margin_bp)` | ~5e19 at the presets of section 7 — not consensus, but see below |

An overflow that panics on one node and wraps on another is a fork by another route. There MUST be
no panicking path.

The bounds above are the ones the recurrence produces. They are **not** the bounds an implementation
may assume, because each of these quantities is computed by a routine an implementation exposes, and
a caller reaching such a routine directly supplies arguments the recurrence never would. The
weighted signal sum is the sharp case: within the recurrence both signals are clamped and it cannot
exceed `4 * SIGNAL_CAP_MICROS`, yet the same expression over unclamped 64-bit arguments exceeds 64
bits above `u64::MAX / (PARTICIPATION_WEIGHT + VOLUME_WEIGHT)`. Widen and saturate at the boundary
of the routine, not at the boundary of the values the caller was expected to pass.

The safety margin of section 7 is not consensus, but it MUST saturate for a second reason: an
unchecked product wraps a large margin into a *smaller* posted amount, which is the one direction a
margin must never fail in.

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

Golden vectors are **per protocol version**. `tests/fixtures/golden_vectors_v1.json` is the
conformance contract for version 1: ten epochs — bootstrap, growth, a 55% participation shock and
recovery — with every intermediate for each. An implementation conforms when it reproduces every
field of every epoch from the census inputs alone.

A version set of vectors MUST be retained and MUST keep running after that version is superseded.
They are the regression test that a later change did not rewrite history, which matters because
verifying the present replays every past epoch under its own rules.

An epoch record MUST retain its inputs and intermediates, not merely its output: divergence between
two implementations has to be auditable, not merely detectable, and floor divisions bite in enough
places that "somewhere in the chain" is not a usable answer. It MUST also retain the protocol
version that computed it (section 2a).

## 11. Scope

This crate is pure arithmetic. It performs no chain reads, no I/O, and depends on no DIG or Chia
crate. The census (`dig-mirror-coin`), the per-epoch database and the gossip service (`dig-node`),
and the margin UI (`dign`, DIG App) are specified elsewhere.
