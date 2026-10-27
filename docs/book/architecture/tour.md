# A guided tour

Consider a choice between two alternatives:

```asp
{{#include ../examples/choices.lp}}
```

Its answer sets are `{a}` and `{b}`. The complete world view is therefore
`{{a}, {b}}`. From a checkout's root, run the included source fixture on the CPU:

```sh
cargo run --locked -p zetesis-cli --no-default-features --bin zetesis -- \
  solve docs/book/examples/choices.lp --backend cpu --all
```

An installed executable can use the same source path and options:

```sh
zetesis solve docs/book/examples/choices.lp --backend cpu --all
```

The two answers may appear in either order. `--all` requests exhaustive
enumeration; stopping after the first answer does not establish the complete
family. The [Rust session example](../rust/sessions.md) checks these exact two
full interpretations and `Completion::Exhausted`, then repeats the collection
through `WorldView`. Run its assertions with:

```sh
cargo run --locked -p zetesis-solve --no-default-features --example book-session
```

## From source to admitted solver input

The Rust example calls `zetesis_themelios::admit_formula`. themelios supplies
lossless parsing and the canonical logical program; zetesis prepares and grounds
that program while preserving original statements and source metadata.
Admission establishes well-formed input, not satisfiability.

Constructed logical programs enter the same compiler through
`prepare_program_formula`, without rendering or reparsing. Preparation exposes
both eager and hybrid grounding. See [program preparation](../rust/source.md)
for their limits and checked examples.

```text
source or canonical logical Program
      │ preparation and grounding
      ▼
original formula theory ── candidate generation ──► candidate
      │                                               │
      └──────── satisfaction and frozen reduct ───────┘
                              │
                 no proper-subset reduct model
                              ▼
                     verified answer set
                              │ complete enumeration
                              ▼
                    complete answer-set family
```

These are semantic stages, not necessarily separately allocated intermediate
objects. A fused operation still owes every connecting invariant.

## The candidate fixes the reduct

For `M = {a}`, `not b` is true and `not a` is false. The positive reduct is the
fact `a.`. Starting from the empty interpretation derives `a` and then reaches
a fixed point. That least model is exactly `M`, so `{a}` is accepted. The
argument for `{b}` is symmetric.

The other two interpretations expose different failure modes:

| Candidate | Original satisfaction | Positive reduct | Decision |
| --- | --- | --- | --- |
| `{}` | Fails both rules | `a. b.` | Reject: not a model |
| `{a}` | Holds | `a.` | Accept |
| `{b}` | Holds | `b.` | Accept |
| `{a,b}` | Holds | Empty program | Reject: the reduct's least model is `{}` |

The last row is why classical satisfaction cannot replace reduct checking.
Equally, copying the candidate into the initial positive closure would make the
test unsound: it would give assumed atoms the status of derived consequences.

The session example also constructs a theory-bound interpretation for `{a}`
and calls `zetesis_stable::check`. A candidate carries no membership claim.
The completed check accepts it only after original satisfaction and reduct
minimality. The [checked Lean consumer](../lean/normal-rules.md) applies the
normal-rule/Ferraris bridge to the corresponding mathematical program.

## A finite closure algorithm

For a fixed seed, the normal positive reduct has a least consequence set. This
bounded full-scan reference states the computation:

```text
derived := empty
repeat within the work and storage limits:
    snapshot := derived
    pending := heads of every enabled source instance in snapshot
    violated := whether any enabled constraint has no head
    if pending adds no atom:
        return accepted exactly when not violated and
               the gate projection of derived equals the seed
    derived := derived union pending
```

An instance is enabled when its positive body is true in `snapshot`, its frozen
gates pass against the fixed seed, and its ground filters hold. Each source join
must enumerate every matching finite substitution. Each round reads one frozen
snapshot; pending consequences become visible only at commit. Constraints are
checked against the completed no-growth snapshot before acceptance.

The reference invariant is that every derived atom follows from the same fixed
reduct; derivation grows monotonically and never starts with guessed atoms. If there are
`A` possible consequence atoms, every growing round adds at least one, so at most
`A` growing rounds precede a final no-growth scan. That complete final scan makes
the derived set closed; sound derivation and leastness then identify it with the
reduct's least consequence set. Exceeding a limit yields an
incomplete result, never an accepted partial closure. Finite source bindings and
complete scans are premises of this argument.

For `M = {a}`, the first scan adds `a`; the second adds nothing. More generally,
let `W` bound the charged indexing, join and condition work of one complete scan.
The schedule performs at most `(A + 1)W` charged scan work, with retained
atom/value payload and indexes bounded separately from join scratch space. This
bound excludes parsing, candidate generation and output. Covering `G` independent gate atoms can require
`2^G` candidate seeds even if one check is cheap. Indexes change actual join work;
these bounds establish neither a speedup nor a process-memory bound.

This closure computation is the definition that zetesis's laws state for normal
programs. Its executable form was the relational closure route, removed in 0.7.0.
zetesis now checks every program through its Ferraris translation, and
`NormalFerraris.answer_set_iff` proves that the two definitions agree on normal
programs. The [delta-round](../lean/theorems.md) and
[source-coverage laws](../lean/theorems.md) that stated the removed route's
incremental schedule remain as statements. The
[reachability example](grounding-comparison.md#follow-a-candidate-dependent-join)
shows why candidate-dependent joins can perform different work for different
seeds.

## Where the machine enters

CPU and GPU execution share the admitted formula representation. Independent
candidate checks may run concurrently; each retains its own interpretation and
frozen reduct. Hybrid execution additionally checks retained source constraints
before accepting an original answer. Scheduling does not change membership.

For a general formula reduct, such as the disjunction `a | b.`, there need not
be one least model. zetesis instead asks whether a proper subset of the
candidate satisfies its frozen reduct. The [next chapter](semantics.md) makes
that distinction precise.

An original theory with positive atomic-head producers is a separate useful
case. Their least consequences can be computed once, allowing positive cycles
and And/Or bodies, then tested against arbitrary original constraints. The
`PositivePlan` library primitive validates that complete formula shape; it is
not inferred from a dependency projection. A failed constraint leaves least
consequences but no answer set; larger classical models may exist. The
[constrained-positive guide](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/constrained-positive.md)
states the exact grammar and reduct argument. This primitive does not itself
enumerate or publish the ordinary session's `AnswerSet` values.

To follow the actual entry points, see the
[writer-free session example](../rust/sessions.md),
[`zetesis_themelios::admit_formula`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/lib.rs),
[the reduct's membership queries](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-stable/src/reduct.rs),
and the [finite formula API](../rust/reducts.md).
