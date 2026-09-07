# Closed structural values

This landing adds finite **data values** to the existing logical carrier. It does
not add constructor pattern extraction or a mechanism that generates arbitrarily
many values. `p(f(1,g(2))). q(X):-p(X).` copies one complete value. In contrast,
`q(X):-p(f(X))` and `p(f(X)):-q(X)` remain outside admission, including when a
particular source would happen to ground finitely in clingo.

## Core representation and ordering

`Value::Structured(StructuralValue)` is additive; the existing public `Value`,
`Term`, `AtomPattern`, `Atom` and `Predicate` vocabulary stays in the dependency-free
core. Adding a variant requires migration of exhaustive matches. `Term` still
contains only a whole constant or a whole variable, and resolves by reference.
The carrier domain contains complete supplied values, not their subterms.

A structural value contains a private validated preorder node sequence, cached
canonical spelling, and root-inclusive depth. The two buffers have immutable
shared ownership. Scalar values do not acquire a heap indirection. Positive
nullary function nodes normalize to the existing Symbol variant even inside a
tree; negative nullary functions and empty/singleton tuples remain distinct.
Constructor sign is independent of the outer predicate sign and numeric minus.
The private constructor verifies one complete tree, all child counts and every
node/depth/storage ceiling before returning a value.

Identity includes every node's tag, signed constructor name/arity, ordered
children and leaf payload. Depth and spelling are deterministic derivatives.
Eq/Ord/Hash never use allocation identity or capacity. Clone and drop are flat;
comparison, hashing, debugging and validation are iterative. Display streams the
cached spelling with no internal allocation and can fail only through its sink.
Cached spelling is prepared through checked reservations during construction.

Storage Ord preserves the relative order of all old scalar variants. ASP
`compare_terms` remains a separate operation: extrema surround numbers, signed
nullary values, strings and nonnullary function-like values. Function-like heads
compare sign, arity and name, then lexicographic arguments. Empty tuples and
positive constants occupy the nullary class; tuples have an anonymous name.
Names remain the source adapter's lexical responsibility, as for existing public
core symbols and predicate names. Observation refuses unrepresentable supplied
public model names/NUL strings before emitting a record.

## Resource contract

Default `ValueLimits` are 262144 nodes, depth128 and 16777216 accounted bytes for
one construction. Bytes include node Vec capacity, UTF-8 text lengths, cached
spelling capacity and both simultaneous validation/render frame reservations.
Checks widen arithmetic before comparing to usize ceilings. Spelling size is
computed without per-number allocations. Malformed arities are rejected before
spelling arithmetic is used. These are logical accounting bounds: String spare
capacity, allocator/reference-count headers and process RSS are not measured.
The standard allocator can still abort on process-wide exhaustion, as with
existing scalar String/Arc ownership; fallible Vec/String reservations return a
typed error and no partial value.

`payload_bytes` counts referenced retained payload, without construction scratch;
clones share this payload. `canonical_bytes` counts identity tags, lengths,
constructor signs/arities and leaves, without allocation capacity. Objective
key/retained-model ceilings use the canonical measure. Source copying uses the
conservative retained-payload measure. CPU relational comparison, formula
expression/join work, objective evaluation and candidate-bound comparisons charge
structural payload traversal. Existing limits and solver algorithms are unchanged.
The bridge also inherits the source syntax/node/depth and cumulative expansion
ceilings; it does not make them unlimited. Public core Program still has its
existing count-based admission contract, not a new whole-program RSS guarantee.

## Source and consumer boundary

- `structural_value.rs` is the sole Symbol/core bridge. Input construction and
  reverse reconstruction are iterative. Nested constant dependency discovery
  includes all symbolic leaves, so forward references and cycles have the same
  meaning inside constructors as at the root.
- `compile.rs`, `extended.rs` and `profile.rs` admit only closed constructors.
  Strict S0 retains its arithmetic restrictions. The extended/formula normalizer
  uses checked i32 operations, and distinguishes signed functions from numeric
  negation. Pools and intervals below a constructor remain refused.
- Closed tuples compare as whole values. A variable-containing flat tuple
  equality still uses the existing finite binding planner and its complete guard;
  this change supplies no nested unification or new recursive generator.
- Formula catalogs, join/factorization keys, aggregate tuple coalescing and
  objective keys retain complete values. Numeric weights, priorities and numeric
  min/max keep their prior type restrictions. Same weights alone never merge keys.
- Core lazy/static closure matching copies whole values. Ferraris/SAT and GPU
  transport still receive original atom IDs; their truth, reduct, candidate
  blocking and membership algorithms are not changed.
- Observation whole-variable substitution reconstructs bounded upstream symbols;
  its output traversal is iterative even for deeper supplied public values.
  Atom and term output channels preserve duplicates across channels. Hidden full
  models and optimal ties remain separate. CLI retention includes every canonical
  structural byte before publication; output failures remain failures.

The new independent tests cover exact complete models, manually constructed
Ferraris truth/reduct formulas in every frozen world, all-model objective costs
and candidate bounds, signed coherence, whole-variable copies on both closure
routes, hidden ties, exact inclusive bytes/work/depth ceilings, cancellation and
recovery, malformed trees and depth10000 flat operations. Original clingo sources
and raw responses are retained with the validation record. These tests do not
prove the source compiler, consumer implementations or machine arithmetic from
Lean. The existing generic Ferraris laws still apply to the unchanged finite
atom/formula representation; no new end-to-end refinement is claimed.
