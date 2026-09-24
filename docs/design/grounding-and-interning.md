# Grounding performance and the interning direction

## Why this document

The region-scheduler work in this change removes the parallel frontier as the
limit on solving. With that bottleneck gone, a profile of a representative
string-bearing program shows where the next cost lives: **grounding**, and within
grounding, **term comparison over string-bearing values** in the ground-atom
lookup. This document records the measured evidence, how to reproduce it, the
resulting direction — intern terms to integer ids and carry a single canonical term
form throughout the solver — and the one design question that must be settled before
that representation change is carried through. It is written so the grounding and
interning work can be planned and executed on this merged base.

## Summary

- The scheduler now walks per-worker work-stealing deques and returns the scalar
  answer-set family under 1, 2, 4 and 14 workers (see the
  [worker-scaling comparison](../book/reference/worker-scaling.md)). The shared-pool
  frontier is no longer the constraint.
- With that in place, a profile of Einstein's riddle
  (`examples/einstein-riddle.lp`) shows solving is dominated by grounding, and
  grounding is dominated by `GroundProgram::atom_id` — a binary search over a sorted
  `&[Atom]` whose every comparison walks string-bearing `Value`s.
- clingo interns every symbol to an integer at parse time, so its ground-atom keys
  are integer tuples compared and hashed in O(1). That is the gap.
- The direction is to intern distinct terms to stable integer ids at admission,
  giving O(1) identity, equality, hash and comparison wherever a `Value` or `Atom`
  is keyed, and a columnar, GPU-forward layout — a **single canonical term form**
  carried at ingest, grounding and output.
- The open question (below) is exactly that single-form commitment, and it is the
  crux, not a detail: it must be decided with the maintainer before the
  representation is changed.

## The evidence

### Phase breakdown — Einstein's riddle

Release binary, Apple M4 Pro, with the budget raised so the program completes:

| Phase | Time |
| --- | --- |
| Source preparation | 1.8 ms |
| **Grounding** | **83.3 ms** |
| Solving (reduct search) | 18.7 ms |
| Observation | 0.03 ms |
| Wall — zetesis | **0.11 s** |
| Wall — clingo | **0.04 s** |

The reduct search is trivial: one candidate. The ~2.75× wall gap is grounding. The
riddle's `solution/6` rule is a five-way join
(`lives_in × painted × drinks × owns × smokes`); grounding it calls the ground-atom
lookup once per generated atom.

### Hot frames — sampled wide join, eager grounder

A scaled five-way join long enough to sample (an `item/1` domain at n = 14, eager
grounder) has these hottest self-time leaves:

| Frame | Samples |
| --- | --- |
| `zetesis_core::ground::GroundProgram::atom_id` | 104 |
| `<zetesis_core::value::Value as Ord>::cmp` | 66 |
| `GroundProgram::instantiate_id` | 9 |

### Root cause

`GroundProgram::atom_id` (`crates/zetesis-core/src/ground.rs`) resolves a ground
atom to its dense id by binary-searching a sorted `&[Atom]`:

```rust
self.atoms.atoms().binary_search(atom).ok().and_then(|index| u32::try_from(index).ok())
```

Each lookup is O(log N) `Atom`/`Value` comparisons, and each comparison walks
string-bearing `Value`s — the riddle's atoms carry strings such as
`"The Englishman"`, `"Coffee"` and `"Parliaments"`. Grounding the wide join performs
on the order of N·log N such string comparisons. clingo interns every symbol to an
integer at parse time, so the same keys are integer tuples: O(1) hash and compare,
no byte walk. The "keyed join without symbol interning" hypothesis, confirmed.

## Reproduce the evidence

From the repository root, with the installed solver and a clingo 5.8.x on `PATH`.

1. **Phase breakdown** — riddle, budget raised so it completes:

   ```sh
   zetesis solve examples/einstein-riddle.lp --all --max-expansion-work 300000000 --stats
   ```

   Read the per-phase timings; grounding dominates. Compare wall time against the
   reference solver on the same source:

   ```sh
   clingo examples/einstein-riddle.lp 0
   ```

2. **Default-budget behavior** — no override:

   ```sh
   zetesis solve examples/einstein-riddle.lp --all
   ```

   At the default budget the eager grounder exceeds the formula-work ceiling on the
   wide join. This change makes that refusal legible — an honest, actionable
   resource-limit message rather than an opaque failure. The standing goal is for the
   riddle to run at the **default budget, with no encoding change, par-or-faster than
   clingo**.

3. **Hot-frame profile** — a run long enough to sample. Construct a scaled wide join:
   an `item(1..14).` domain and a five-way rule, e.g.

   ```
   item(1..14).
   p(A,B) :- item(A), item(B).   q(A,B) :- item(A), item(B).
   r(A,B) :- item(A), item(B).   s(A,B) :- item(A), item(B).
   wide(A,B,C,D,E) :- p(A,B), q(B,C), r(C,D), s(D,E), item(E).
   ```

   run it under the eager grounder and sample the process (macOS `sample`; use the
   equivalent sampler on other platforms):

   ```sh
   zetesis solve widejoin.lp --grounder eager --stats &
   sample $!
   ```

   The hottest leaves are `atom_id` and `Value::cmp`, as tabulated above.

Broader corpora for regression and scaling live beside the riddle:
`examples/correctness` (semantic breadth on small instances, clingo-verified) and
`examples/scalability` (parametric throughput cases). They are the verification base
for any change to grounding or the term representation.

## Direction — intern terms to integer ids

The durable fix is to intern distinct terms to stable integer ids at admission:

- **Integer identity.** Identity, equality, hash and comparison become integer
  operations, O(1), wherever a `Value` or `Atom` is keyed — not only in `atom_id`
  but on every `Value::cmp` hot path.
- **Children-first structural interning.** A leaf keys by `(kind, payload)`; a
  compound term by `(name-id, sign, [child id])`, so compound keys are small
  id-tuples with no string comparison past leaves.
- **Columnar, GPU-forward layout.** Dense typed id columns autovectorize and map to
  device buffers.
- **One canonical term form** carried throughout — the same interned representation
  at ingest, grounding and output — rather than string-bearing values compared by
  walking their bytes.

Interning only assigns equal terms equal ids, so answer-set families are unchanged;
correctness is checked by the `examples/correctness` corpus against clingo and by
scalar-versus-workers agreement.

A smaller first lever, if a staged approach is preferred, is a hash index for the
ground-atom lookup (a `HashMap<Atom, AtomId>` or the existing `atom_lookup` index)
built alongside the sorted catalog, turning the O(log N) `Value::cmp` in `atom_id`
into O(1) hashing without yet changing the term representation. It is a legitimate,
measurable stopping point; full interning is the end state and additionally removes
the string comparisons on every other hot path.

## Open design question — one canonical form, held once

Carrying a *single* canonical term form is the goal, and it is also the crux. A bare
integer-id representation for `Value` cannot also be its own canonical order: a
`std::cmp::Ord` takes no arguments, but the canonical (storage) order of an interned
term must be resolved *through the interner* — it is not the raw id order, because
ids are assigned in discovery order. So once `Value` and `Atom` are id-backed:

- `Atom`'s derived `Ord` necessarily becomes **id order**, sound only for internal,
  unobservable membership (for example a `BTreeSet<Atom>` borrow bridge), and
- every **observable** atom or term order — models, catalogs, the gate carrier, any
  comparison that reaches output — must be resolved **canonically through the
  interner**.

The failure mode to avoid is a solver that ends up holding **two** orders at once:
core structures canonical, some consumer's internal structures id-order, quietly
converting between them. That is the opposite of the single-canonical-form goal, and
a mismatch — a search in one order over data built in the other — is a correctness
defect that compiles cleanly and surfaces only as a wrong answer-set family.

**The decision to settle first**, with the maintainer, before the representation is
changed: commit the entire solver — including internal membership and search
structures — to **one** order resolved through the interner, accepting
interner-threaded comparisons on the affected hot paths; versus any scheme that keeps
more than one order. Every downstream choice follows from that commitment: which
structures thread the interner, where a std `Ord` is still admissible, and how the
columnar tables are keyed. The reproduction fixtures here and the correctness and
scalability corpora are the verification base for whichever form is chosen.
