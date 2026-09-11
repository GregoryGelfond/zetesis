# zetesis-objective

This crate computes minimization costs from a supplied, complete `zetesis_core::Model`.
The caller establishes stable-model acceptance with its reduct oracle. Objective
evaluation reads atoms and returns scores; it does not derive atoms, establish
support, search for models, or prove optimality.

## API

`ObjectiveTemplate::new(weight, priority, tuple, positive, filters)` assembles one
lifted element with `WeightPolarity::AsWritten`. The additive
`with_weight_polarity(WeightPolarity::Negated)` builder represents maximization
without eagerly grounding a bound weight variable. `weight()` retains the original
term; `weight_polarity().normalize(i32)` is the single checked numeric operation
used by evaluation and the frontend's optional candidate-bound compiler. Weights and explicit tuple components use the core scalar `Term`
type. The priority is a fixed `i32`; a frontend supplies zero for an omitted
priority. Lifted binding conditions are ordinary positive `AtomPattern`s and
exact `Eq`/`Neq` filters. `with_condition(Condition)` additionally accepts a
closed query over complete typed atoms. `ConditionNode` represents Boolean
constants, atom membership, negation, conjunction and disjunction; each operand
index must precede its operation and the final node is the result. An empty
query is true. Arithmetic, aggregate and conditional source syntax still needs
an explicit frontend translation with its own completed eligibility evidence.

A closed condition reads the original supplied model. It owns no semantic atom
universe or theory roots, creates no support, and performs no reduct check.
This differs from a Ferraris theory even when a frontend derives the query
from the same source condition. Source-level eligibility and priority presence
remain separate from the query's truth in an individual model.

`ObjectiveProgram::new(templates, AdmissionLimits)` validates shape limits and
variable safety. Variable IDs are local to each template, dense from zero, and
must occur in a positive condition. Comparisons do not bind variables. Templates
retain their input order, so admission and evaluation failures can report a
`template_index()` into the caller's separate source-origin table. The admitted
program is immutable and cheaply shared through `Arc`.

`evaluate(&program, &model, Limits, &Control)` returns an `Evaluation` with:

- `score()`: signed costs at the program's distinct descending priority slots;
- `contributions()`: canonical active keys sorted by `(priority, weight, tuple)`;
- `statistics()`: charged work, complete bindings, active bindings, duplicate
  keys, retained keys and their canonical payload bytes.

The closed query is evaluated once before the template's positive joins. A
false query skips those joins while preserving the admitted priority slot.
A template with no positive conditions has one empty binding when its closed
query is true, subject to its scalar filters.
After a complete binding passes every filter, a numeric weight contributes its
normalized tuple key. Checked negation of `i32::MIN` returns a typed
`WeightNormalizationOverflow` with template/statistics evidence; no partial score
is successful. Nonnumeric weights are ignored, including real extrema, symbols
and strings. The program retains its fixed priority slots; the source frontend
separately establishes whether each priority has a possible numeric binding. Evaluation is not bound to a particular core program instance: checking
that the supplied model belongs to the intended source program is the caller's
responsibility.

## Tuple and score semantics

Equal keys `(priority, normalized weight, tuple)` contribute exactly once across every
binding, template and directive. Repeated eligibility therefore acts as an OR.
Normalization happens before deduplication: maximize `2@p,k` and minimize
`-2@p,k` coalesce, while maximize `2@p,k` and minimize `2@p,k` are different signed
keys whose costs can cancel. Direction is not part of key identity. Priority is
unchanged; printed/comparison costs retain the normalized minimization sign.
Core scalar identity distinguishes numbers, strings and symbols. Zero weights
remain legitimate keys.

Every admitted priority remains a score slot, including a slot with no active
key in the current model. Totals accumulate with checked `i128` arithmetic and
are converted to `i64` only after evaluation. This avoids template-order-dependent
intermediate `i64` overflow; an unrepresentable final total is a typed failure.

`Score::compare_costs` compares descending priorities with missing priorities
treated as zero. `Ordering::Less` means a better minimization score. Use this
method for incumbent comparison and ties. Structural `Eq` additionally preserves
the explicit slots and objective-presence flag; it is intentionally a different
relation.

`ObjectiveProgram::none()` represents an absent objective.
`ObjectiveProgram::new(...)` represents a present objective even when its
template list is empty. The evaluator never infers source-level absence from an
inactive condition, a zero weight, a zero total, or an empty model. A frontend is
responsible for matching the source grounder's rules for objective presence and
priority retention. Evaluating a possible-positive relation can discover its
active keys, but cannot by itself prove stable-model support or decide which
source priorities a grounder would retain.

## Resource contract

Admission bounds templates, tuple width, local variables, positive conditions,
predicate arity, filters and closed condition nodes. Evaluation separately bounds charged work, complete
positive bindings before filters, unique keys and retained canonical key bytes.
Every ceiling is inclusive; zero is a real ceiling. Duplicate keys consume join
and lookup work but no additional retained-key or key-byte budget.

The canonical key payload is 16 bytes for the priority, weight and tuple length,
plus each scalar's tag and payload: 5 bytes for a number, or 9 bytes plus UTF-8
length for a string or symbol. This portable count excludes vector capacity,
allocator overhead and temporary borrowed tuple storage. It is not a process
memory limit.

Joins use iterative heap frames and an undo trail; they do not recurse on body
depth or enumerate a global domain. Model atoms are scanned directly, without a
separately allocated relation index. Scalar comparisons, string copying, binding
work, key searches and key insertion shifts are charged, with shared cancellation
and deadline polling at charged operations. Counts and storage reservations are
checked. Input construction and standard allocator internals are outside this
logical work budget.

Closed queries use linear temporary Boolean storage and visit operations in
their admitted order. Atom tests use charged scans of the supplied model;
worst-case query work is linear in query nodes times model payload. There is no
recursive evaluation, implicit solver, or condition-specific score path.

Retained keys use a sorted vector with binary-search lookup and charged linear
insertion shifts. This simple implementation can take quadratic insertion work
in the number of distinct keys; the work ceiling makes that cost explicit. It
is an exact bounded baseline, not an optimized objective index.

Failures preserve partial statistics and an originating template index where
one exists. No partial score is returned as successful, and stopping evaluation
never proves optimality, satisfiability or exhaustion. A caller that enumerates
stable models must obtain complete search coverage before reporting a global
optimum.

## Validation

The tests cover partial-match undo, shared and repeated variables, global tuple
coalescing, negative and zero weights, fixed priority ordering, absent objectives,
ignored nonnumeric weights, exact resource ceilings, cancellation, deadlines and
1,024-condition joins on a small stack. A property test compares joins, key sets
and costs with independent valuation enumeration on tiny generated relations,
including reversal of template order. Final signed-cost conversion is checked at
both `i64` boundaries. `tests/polarity.rs` separately checks both normalization
polarities, dynamic weights, normalized cross-direction key identity, ignored
nonnumeric rows, wide costs, inclusive work ceilings and typed negation overflow.
`tests/conditions.rs` checks closed-query truth tables, full typed atom identity,
zero priority slots, coalescing with lifted rows, malformed references and exact
work ceilings. `zetesis-themelios/tests/objective_condition_bounds.rs` compares
every candidate against every retained score to establish that optional bounds
preserve closed-query costs and ties without changing the original atom catalog.

Run `cargo test -p zetesis-objective` and
`cargo clippy -p zetesis-objective --all-targets -- -D warnings` from the workspace.
This executable evidence does not claim a Lean refinement of the evaluator.

Licensed under the repository's MIT license; copyright Gregory Gelfond.
