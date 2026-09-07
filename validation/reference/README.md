# zetesis: executable semantic reference

This standalone Rust package is an exact **CPU semantic model**, independent of the apokrisis implementation. It has no dependencies or network access. It implements a finite ground kernel and a bounded lifted interpreter with streaming relational joins. It does not implement a GPU backend, neuromorphic device backend, themelios parser/adapter, Rayon execution, or learned candidate generation. It is a correctness baseline for those future experiments, not evidence of hardware performance or scalability.

The supported finite ground language has normal rules, constraints, `not`, `not not`, and singleton choice heads. Rules are supplied through the Rust API. Atom identifiers are dense zero-based indices; `u64` masks represent sets. Construction rejects atom counts of 64 or more, out-of-carrier heads/body atoms, and headless choices. The validation campaign uses at most five atoms, with a separate six-atom depth test. Exhaustive enumeration remains exponential even though masks can represent larger carriers.

## Run

Rust 1.97 or newer is required; the package uses edition 2024.

```sh
cargo test --offline
cargo run --release --offline -- --report test-results.json
```

Both commands execute the deterministic property campaign. `cargo test` additionally runs named fault and adversarial tests. The executable writes its own semantic-check report; it does not invoke Cargo tests or claim that they ran. `test-results.json` is the recorded executable campaign. All random generation and shuffled event schedules have fixed seeds. The report's `seconds` has `time_scope: ground_kernel_campaign`: it excludes compilation, Cargo tests, and the separately reported lifted fixtures, and is not total verification time.

## Lifted transformer composition and sparse candidates

`src/lifted.rs` makes the central experiment executable without a mandatory ground-rule interface. Its validated templates contain `Var`/`Const` atom patterns, ordinary positive bodies, true/false reduct gates, and exact Eq/Neq filters. Domain values distinguish integers, text, and symbols. Every variable must occur in an ordinary positive body atom. The domain and predicate arities describe finite carriers symbolically; no implicit new value is admitted.

`LiftedProgram::check` takes a frozen sparse true seed. Its complement over the symbolic gate-predicate carriers is false, including tuples never allocated or visited. The oracle starts with empty derived relations and composes streaming binding, filtering, bound-gate pushdown, projection, set union, and complete synchronous fixed-point passes. Constraints use the same filtered/gated matching pipeline and retain filter truth. The oracle can derive heads absent from the candidate proposal. It never enumerates the seed carrier and never retains instantiated ground rules; it does visit the exact positive-body matches needed by inference.

Round, work, and derived-atom limits return explicit errors with no accepted/rejected value. Template construction also refuses more than 64 ordinary positive body atoms, a reference-only capability limit that bounds the recursive join stack. This is a bounded scalar reference, not a production index planner, resumable cursor protocol, or complete lifted candidate search. The small ground module still demonstrates complete coverage search separately. A future generator can propose sparse seeds through this API; the package does not claim to learn those proposals.

The CLI separately runs two fixture families at domain sizes 32 and 128. The first tests symbolic-carrier laziness:

```prolog
dom(i).  % one fact for each declared domain value
{pick(X,X)} :- dom(X).
seen(X,X) :- dom(X), not ban(X,X).
```

The conservative symbolic seed carrier has `2*n*n` atoms (`pick/2` and `ban/2`). The complete empty seed produces exactly `2*n` atoms: every `dom(i)` and `seen(i,i)`. A competent conventional grounder can also instantiate the diagonal rules in O(n); this fixture does not establish an advantage over such a grounder.

The second family exercises candidate-dependent join reduction:

```prolog
{pick(X)} :- dom(X).
pair(X,Y) :- dom(X), dom(Y), not not pick(X), not not pick(Y).
```

A complete singleton seed `{pick(0)}` produces exactly `n+2` atoms: the domain facts, `pick(0)`, and `pair(0,0)`. After binding X, the frozen gate rejects every X except 0 before scanning Y. The interpreter performs O(n) tuple probes instead of visiting all n² source substitutions for the pair rule. Both families assert zero carrier-tuple enumeration, zero retained ground rows, exact expected models, and linear probe/binding counts. The report distinguishes the analytical source-binding space from actual probes across all closure passes. These demonstrate execution mechanisms, not a benchmark against another solver or a hardware speed claim.

Additional Cargo tests independently enumerate all substitutions for small templates (at most eight ground atoms), build a ground `Program`, and compare every sparse seed's accepted full-model set with the independent full-interpretation reference. Those tests include choices, constraints, Eq/Neq filters, repeated variables, multi-body joins, typed constant identity, empty bindings, unsafe variables, invalid seeds, and budget interruption. The independent test grounder deliberately materializes carriers and rows; its work is separate from the lazy interpreter's zero-materialization counters.

## Implementations compared

1. An independent full-interpretation reference forms each reduct using sets and checks exact least-model equality plus constraints.
2. The reduced seed checker guesses only gate atoms, computes positive closure from bottom, and accepts exactly when `closure & S == seed` and constraints hold.
3. Cube search computes must/may reduct closures, narrows seed bounds, and splits an unknown bit until every remaining leaf is accepted or refuted. It records a complete ledger of transforms and split edges. Replay verifies coverage and accepted models; aggregate counters alone do not establish completeness.
4. An event evaluator latches the seed for an epoch, counts distinct positive antecedents, emits each derived atom once, and fires each enabled rule once. A seeded scheduler shuffles reliable message delivery. Three schedules are checked per seed. Head activations from multiple supports are intentionally deduplicated; duplicate deliveries of the same antecedent to one rule are errors.

The event evaluator runs to quiescence and performs a final rule/delivery coverage scan. This scan detects dropped events in the model. A physical backend must supply its own reliable delivery and quiescence contract; the reference does not claim that an empty device queue can independently detect dropped packets. Each invocation creates fresh state. Stale epochs, duplicate antecedents, counter underflow, missing messages, and premature acceptance have explicit tests.

## Deterministic verification coverage

- Every program with one atom and at most four distinct rules from the complete supported one-atom rule universe: 12,951 programs.
- Every program with two atoms and at most two distinct rules from the complete supported two-atom rule universe: 51,361 programs.
- 1,000 seeded random programs, with one to five atoms and zero to twenty rules. The generator alternates sparse and unrestricted body masks and includes choices, constraints, double negation, and contradictory gates.
- For each program, compare all full candidates with all reduced seeds, every seed with three event orders, every cube with every contained seed, all valid seed survival obligations, and complete cube-search output with the independent reference.
- Separate tests cover empty carriers, empty constraints, unsupported positive loops, odd negation, mutual negation, choices, depth, competing support, stale state, upper-bound unsoundness, gate correlation, and corrupted coverage ledgers.

The report records actual counts for the executed campaign. These finite checks provide regression evidence, not a general mathematical proof and not validation of unimplemented source-language lowerings.

## API example

```rust
use zetesis_reference::{Program, Rule, exact_cube_search, verify_ledger};

// {a}. b :- a. u :- u. :- not b.
let program = Program::new(3, vec![
    Rule::singleton_choice(0, 0, 0),
    Rule::normal(1, 1 << 0, 0, 0),
    Rule::normal(2, 1 << 2, 0, 0),
    Rule::constraint(0, 1 << 1, 0),
]).unwrap();
let result = exact_cube_search(&program).unwrap();
verify_ledger(&program, &result).unwrap();
assert_eq!(result.models, vec![0b011]); // {a,b}, never u
```

`Rule::normal(head, positive, negative, double_negative)` separates ordinary positive dependencies from reduct gates. `singleton_choice` adds its head as a positive gate internally. `Program::seed_carrier()` includes gate references in constraints. Contradictory positive/negative gates are disabled. May-closure can still combine mutually incompatible rules, so it is a sound upper approximation rather than a feasible-world witness.

Search here always runs to completion. A completed empty result means UNSAT for this finite input; a nonempty result is its complete set of stable models. Any future budgeted, distributed, or capacity-limited version must add an explicit UNKNOWN/incomplete status and preserve unresolved ledger regions.
