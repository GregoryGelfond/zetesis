# From answer-set semantics to exact transforms

zetesis has two complementary vocabularies. The semantic layer speaks about
programs, interpretations, satisfaction, reducts and answer sets. The execution
layer speaks about relations, masks, reductions, fixed points and batches. The
connection between them is the architecture: each execution operation must
implement a stated logical operation, and composition must preserve its subject
and completion conditions.

![The semantic contract maps to execution representations and exact transforms, which are scheduled on the host or GPU.](alignment.svg)

The vertical arrows indicate representation obligations. They are not claims
that every Rust or WGSL correspondence has been formally proved. The horizontal
semantic path remains the same across execution strategies.

The pseudocode specifies finite, admitted inputs. Every bounded operation must
either finish or return an explicit stop; a stop propagates as unfinished work.
It must never be interpreted as an empty result, a false formula or exhaustion.

## What the transformer inspiration means

The hardware opportunity is to express repeated logical work through a small
algebra of composable operations on many interpretations and shared immutable
program data. Packing truth values, retaining reusable representations, fusing
equivalent operations and evaluating independent rows can make that work suitable
for modern CPUs and GPUs.

Here a **transform** is an exact operation, not a trained approximation. There
are no learned weights, attention scores or floating-point truth values. In the
Lean foundation, a `Transformer` is specifically a function from a set of atoms
to a set of atoms. Not every execution operation has that type: a join binds
tuples, a satisfaction test returns truth, and a frozen-reduct constructor retains
an interpretation-dependent view. Their individual contracts make composition
meaningful.

This does not require every operation to become dense matrix multiplication.
Sparse joins, bitwise operations and integer reductions often preserve the
problem's structure more directly. Efficient scheduling follows the representation
and its dependencies; it does not change the answer-set definition.

| ASP operation | Execution interpretation | Where to read the implementation |
| --- | --- | --- |
| Instantiate a rule body | Join positive witnesses, agree on repeated variables, filter scalar conditions, project an instance | [`source::scan`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/oracle/source.rs); [source preparation](grounding.md) |
| Determine the normal reduct | Evaluate positive/negative gates against one immutable seed | [`check_static`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/static_oracle.rs) |
| Derive positive consequences | Test enabled bodies, union their heads, repeat until closed | [`check_static`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/static_oracle.rs); [lazy batches](../rust/parallel.md) |
| Test formula satisfaction | Evaluate an acyclic Boolean graph; require every asserted root | [`models`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/oracle.rs) |
| Construct and reuse a formula reduct | Freeze candidate truth at every graph node; mask candidate-false nodes during later queries | [`FrozenReduct`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-ferraris/src/reduct.rs) |
| Establish subset minimality | Search for a proper-subset reduct model; propagate Boolean domains and exactly complete unresolved queries | [`zetesis-sat`](../../../crates/zetesis-sat/README.md); [`GpuFormulaOracle`](../../../crates/zetesis-wgpu/README.md) |
| Evaluate an aggregate | Coalesce complete tuple identities, combine eligibility, then reduce count/sum/extrema | [Native aggregate operations](../../../crates/zetesis-wgpu/README.md#native-numeric-aggregates) |
| Check several proposals | Share program data while keeping each interpretation, reduct and verdict separate | [`BatchOracle`](../rust/parallel.md); [commit boundaries](execution.md#immutable-rounds-and-commit-boundaries) |

These are capability mappings. Ordinary relational solving supports lazy source
rounds, Rayon and GPU closure. Ordinary formula solving currently grounds eagerly
and combines host candidate search with optional GPU propagation and exact host
completion. Native aggregate and GPU tight-program operations are explicit
library capabilities; ordinary dispatch does not automatically use them.

## Positive inference is a composition with a fixed point

Consider `reachable(Y) :- reachable(X), open(X,Y), not blocked(Y).`
Binding joins `reachable` and `open`; a candidate-fixed gate checks `blocked`;
projection produces `reachable(Y)`. Union combines consequences from all rules.
For a fixed candidate `M`, write the resulting consequence operation as `T_M`.

Conceptually, the operation has the following structure, with local binding and
filter requirements supplied by the admitted rule:

```text
T_M = union over rules of (Project ∘ Gate_M ∘ Filter ∘ Bind)
next(X) = X ∪ T_M(X)
```

For the positive normal reduct, this operation is monotone in `X`. Starting from
the empty interpretation yields only justified consequences; reaching closure
establishes the least result. Facts contribute without positive antecedents.
Constraints are checked separately and derive no head.

The pseudocode below describes the logical algorithm. Each scan must finish;
limits, cancellation or an execution failure return `Unfinished` rather than
the Boolean answer suggested by an incomplete prefix.

```text
check_normal(P, M):
    R := freeze_normal_reduct(P, M)
    X := empty interpretation
    repeat:
        H := complete_enabled_heads(R, X)
        Y := X union H
        if Y = X:
            break
        X := Y
    if any constraint of R has a true body in X:
        return Rejected
    if X != M:
        return Rejected
    return Accepted(M)
```

**Invariant:** `X` is contained in the least closure of the frozen positive
rules. Every changing round adds an atom; a finite carrier bounds such rounds.
An unchanged *complete* scan proves closure. Together these facts establish
leastness; equality with `M` supplies the answer-set test.

The relational implementation can propose only a **gate seed**, compute its
closure `X`, and compare `X`'s gate projection with that seed. It thereby avoids
guessing every derived atom. The seed must cover every gate that can affect the
reduct; the resulting `X`, not the seed alone, is the answer interpretation.
Eager scans and lazy joins implement this same contract. Shared lazy scans may
offer instances from a union of worlds, but each world must recheck its own
antecedents before deriving a head.

This least-closure procedure applies to the admitted normal profile. A positive
disjunctive program may have several incomparable minimal models, so the general
oracle below retains subset minimality.

## Satisfaction and the reduct share one graph

Let the original finite theory `T` be a directed acyclic graph whose children
precede their parents. A node denotes an atom, falsum, conjunction, disjunction
or implication. Classical evaluation is a topological fold. Freezing stores that
fold's values for `M`; a later query evaluates the same nodes in `J` while masking
every candidate-false node to falsum.

```text
evaluate(T, I, frozen = absent):
    values := empty node array
    for node in T, children before parents:
        value := classical_operation(node, I, values_of_children)
        if frozen is present:
            value := value AND frozen[node]
        append value to values
    return values

satisfies_roots(T, values):
    return ALL(values[root] for root in T.asserted_roots)

check_general(T, M):
    frozen := evaluate(T, M)
    if not satisfies_roots(T, frozen):
        return Rejected
    result := search_proper_subsets(M, using the fixed predicate:
        J => satisfies_roots(T, evaluate(T, J, frozen)))
    match result:
        Found(J)        => RejectedWithCounterexample(J)
        Exhausted       => Accepted(M)
        Stopped(reason) => Unfinished(reason)
```

**Invariant:** after each graph step, the stored value is that node's classical
truth, or its reduct truth when a frozen mask is supplied. The mask belongs to
the original `M` for the entire query. The countermodel search ranges over
`J ⊊ M`, excluding `M` itself; its exhaustion must establish coverage of that
space. For an empty `M`, there are no proper subsets.

The small reference checker explicitly enumerates subsets. Ordinary solving
uses propagation and native search to discharge the same existential question.
On the GPU, Boolean domains can be narrowed by many local gate operations. A
propagation fixed point may leave choices unresolved, so it returns residual
work for exact host completion. A class certificate can justify a cheaper exact
membership test when its premises hold.

The opportunities for parallelism have different dependencies: candidates and
frozen-subset queries are independent, while nodes depend on their children and
closure rounds depend on earlier consequences. Parallel evaluation must respect
those dependencies. Reusing a frozen mask is useful even on one CPU thread.

## Batch the question, preserve every obligation

Candidate generation and membership checking have separate completeness
requirements. A source generator can reduce materialization, and a search
generator can avoid already excluded interpretations, provided neither loses an
answer of the original program.

```text
enumerate(P):
    generator := proposals_for_original_program(P)
    while generator has unaccounted work:
        batch := generator.next_bounded_batch()
        checks := check_each_proposal(P, batch)  // independent subjects
        for each completed check:
            commit its verdict to the corresponding proposal
            if accepted:
                emit its checked AnswerSet
        if any required work stopped:
            return Incomplete(with retained progress)
        generator.apply_only_sound_feedback(checks)
    return Exhausted
```

This is a scheduling contract, not a claim that every backend implements an
identical loop or commit granularity. Candidate rows cannot share truth by
accident. A rejected proposal, a pending query and a committed answer remain
different states. General subset blocking is not licensed merely by finding an
answer: for example, `{a}.` admits both the empty answer and `{a}`.

Objective selection and display follow checked answers. Complete unrestricted
enumeration plus complete retention can construct a `WorldView`; optimal ties
and projected displays have different contracts. Batched execution must account
for every proposal before claiming exhaustion.

## The correspondence in Lean

[`Transformers.lean`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Transformers.lean)
defines composition, parallel union, filters, relational image, least closure
and world-indexed batches. `finiteIter_sound` and `finiteIter_exact_if_closed`
capture the positive-inference argument. `least_batch` states that independent
worlds retain their individual least closures. `fusion_preserves_least` requires
pointwise equivalence of the substituted operations, not agreement on one run.

[`NormalFerraris.ferraris_answer_set_iff_closure`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/NormalFerraris.lean)
connects normal-rule closure to formula answer-set semantics.
[`Ferraris.lean`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/Ferraris.lean)
defines satisfaction, the formula reduct and subset minimality; the
[theorem map](../lean/theorems.md) links the execution and coverage laws.
These are mathematical bridges. Establishing that concrete source lowering,
packed layouts, Rust operations and shaders implement them remains part of the
[refinement boundary](../lean/correspondence.md).

## Relation to clingo

clingo combines the gringo grounder with clasp. clasp is an ASP solver whose
primary algorithm uses conflict-driven nogood learning; it also provides
multithreaded search and dedicated cardinality/weight propagation. Its use of
Boolean solving techniques does not reduce its semantics to ordinary SAT.
See Potassco's descriptions of [clingo](https://potassco.org/clingo/) and
[clasp](https://potassco.org/clasp/).

zetesis makes candidate coverage and exact reduct membership explicit composition
boundaries, then maps their operations to shared source scans, packed data,
Rayon tasks and wgpu batches. This separation is the basis for its hardware
experiments and proof obligations. Parallelism alone is not a distinction from
clasp, and this comparison does not classify every earlier ASP architecture.
The [grounding comparison](grounding-comparison.md) explains the materialization
boundary; measured benefits depend on the input and executed profile.
