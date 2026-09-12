# Preserving positive bindings through table selection

[`TableBindings`](../Zetesis/TableBindings.lean) connects the value-support law in
[`FiniteTables`](../Zetesis/FiniteTables.lean) to the source matcher in
[`StructuralBindings`](../Zetesis/StructuralBindings.lean). The result is equality
of ordered row-and-binding families. It preserves more than a set of projected
values: every original row occurrence remains beside the same resulting binding.

## Two namespaces and one whole row

Consider the positive source pattern `p(X, 7, X, _)`. Its table scope is
`[0, 1, 0, 2]`. These labels identify table variables, not source binding slots.
Their origins are the source slot for `X`, the constant `7`, and the distinct
slot assigned to that anonymous occurrence. If the incoming binding gives `X`
the value `a`, the domains are `{a}`, `{7}` and unrestricted. Otherwise the
first domain is unrestricted too. A different anonymous occurrence has a
different source slot.

Rows `(a, 7, a, b)` and `(a, 7, a, c)` both survive. `(a, 7, c, b)` fails the
alias check even though its individual values occur in the relation. Two
equal-valued rows at different positions remain two witnesses. No combination
of separately supported column values manufactures a new tuple.

The mathematical `Argument` is either a constant or a source slot.
`Allowed before argument value` is typed constant equality, or consistency with
that slot's incoming binding. `constraints` lists all source-slot occurrences
in column order, retaining aliases. Constant checks remain separate from
binding extension.

## Why a successful match cannot be pruned

`flat_match_survives` starts with an original row, successful constant checks
and a successful call to the existing mathematical `matchConstraints`.

1. `StructuralBindings.matching_sound` gives agreement on every source-slot column
   and preservation of every incoming binding.
2. Columns sharing a table label have the same source argument. A constant has
   one value; a bound slot has one value. Hence the row satisfies every alias.
3. Constant columns satisfy their singleton domains. An unbound slot permits
   every value; a previously bound slot must retain that value.

This proves the `FiniteTables.Survives` premise. The existing
`indexed_survival_exact` law then says the same row occurrence belongs to the
intersection of permitted value supports. No assumption about candidate truth
appears in this argument. With no columns, the original-row premise still
prevents an empty relation from creating a nullary witness.

## Ordered match families and joins

`matchedRows` applies a pure positive matcher to an ordered row list. Success
retains `(row, resulting binding)`; relational mismatch contributes nothing.
`indexed_matches_preserved` compares this list with the one obtained by first
filtering through exact indexed-survival bits.

The proof proceeds through the original list. A successful match necessarily
has a true selection bit, so both sides retain the same pair. An unsuccessful
match contributes nothing on either side, whatever its bit. The tails agree by
induction. Thus order and multiplicity are preserved without deduplicating
bindings or row values. The statement permits an arbitrary unchanged positive
matcher; `flat_match_survives` supplies its necessity premise for flat patterns
under the stated source correspondence.

`join_family_preserved` composes this replacement over a finite positive-join
schedule. Each step retains the source-occurrence tag and selected row; a
complete result includes the entire witness trace and final binding. Equal
step families give equal continuations for every intermediate binding. The
empty schedule retains the incoming binding exactly once. Repeated uses of the
same predicate remain distinct source occurrences. The theorem fixes the
execution schedule; it does not justify changing that schedule or error order.

`ColumnRelations.row_mask_roundtrip` separately relates exact membership bits
to the same increasing sequence of original row positions. Its ordering and
membership premises still have to be established for the concrete mask.
The shortest-posting route and table route can therefore supply the same
successful family while visiting different rejected rows and charging different
work.

## What remains an implementation obligation

The consumer must borrow the same immutable relation from completed possible
support, preserve its signed predicate and source occurrence, construct the
canonical scope correctly, and derive fresh domains only from constants and
already-bound whole values. Packed words, zero tail bits, exact ordered decoding
and owner lifetimes are Rust representation obligations. The complete source
carrier is supplied by a separate [support argument](source-support.md).

The theorem's `none` is a positive relational mismatch. It does not mean an
undefined arithmetic expression, cancellation, exhausted work or allocation
failure. Those failures must remain observable. Authored comparisons, negative
conditions and aggregates still receive the same completed positive-binding
family; they cannot supply extra prefilter domains that hide a required body
error. Head-only evaluation retains its separate selection boundary.

The retained source row is still an atom in the emitted formula, never a truth
constant. `StructuralBindings.retained_atom_context` and
`retained_atom_stability` state the original/frozen consequence when the capture
map is unchanged. Establishing that correspondence for the grounder is separate
from these table laws. The laws do not certify Rust execution, finite budgets,
parallel scheduling, GPU code or complete answer-set enumeration.
