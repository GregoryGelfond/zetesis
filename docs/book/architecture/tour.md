# A guided tour

Consider a choice between two alternatives:

```asp
{{#include ../examples/choices.lp}}
```

Its answer sets are `{a}` and `{b}`. The complete world view is therefore
`{{a}, {b}}`. From a checkout's root, run the included source fixture on the CPU:

```sh
cargo run --locked -p zetesis-cli --no-default-features --bin zetesis -- \
  docs/book/examples/choices.lp --backend cpu --models 0
```

An installed executable can use the same source path and options:

```sh
zetesis docs/book/examples/choices.lp --backend cpu --models 0
```

The two answers may appear in either order. `--models 0` requests exhaustive
enumeration; stopping after the first answer does not establish the complete
family. The [Rust session example](../rust/sessions.md) checks these exact two
full interpretations and `Completion::Exhausted`, then repeats the collection
through `WorldView`. Run its assertions with:

```sh
cargo run --locked -p zetesis-cli --no-default-features --example book-session
```

## From source to a logical program

themelios supplies lossless parsing and an owned logical `Program`. The
zetesis frontend checks its supported profile and creates an admitted
representation, preserving source metadata. For these normal rules it can retain
relational templates, rather than requiring a complete ground-rule graph.
Admission establishes a well-formed input for a particular execution profile.
It does not establish satisfiability.

```text
original source
      │ themelios parsing and logical Program
      ▼
admitted templates ── candidate generation ──► candidate
      │                                         │
      └──────── original program + frozen gates ┘
                              │
                              ▼
                  positive reduct consequences
                              │
                   closure and constraints
                              │
                   candidate agreement
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

The relational implementation can guess only the atoms needed to decide frozen
gates. It derives the remaining atoms and checks that the result's gate
projection agrees with the seed. Here both `a` and `b` are gate atoms, so seeds
and full candidates happen to have the same carrier.

The session example also constructs `Seed::new` for `{a}` and calls
`zetesis_cpu::check` directly. A seed fixes gate truth; it carries no membership
claim. The completed `Check` accepts it only after closure, constraints and gate
agreement. The [checked Lean consumer](../lean/normal-rules.md) applies the
normal-rule/Ferraris bridge to the corresponding mathematical program.

## A finite closure algorithm

For a fixed seed, the normal positive reduct has a least consequence set. This
bounded schedule computes it:

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

The invariant is that every derived atom follows from the same fixed reduct;
derivation grows monotonically and never starts with guessed atoms. If there are
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

The concrete contracts are
[`check` and `least_closure`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/oracle.rs)
and the [source-coverage laws](../lean/theorems.md). The
[reachability example](grounding-comparison.md#follow-a-candidate-dependent-join)
shows why candidate-dependent joins can perform different work for different
seeds.

## Where the machine enters

CPU execution can perform source joins directly, or use an explicitly compiled
ground graph. Independent candidate checks can run in an owned Rayon pool.
Lazy GPU rounds share source instances across candidates while keeping each
candidate's frozen gates and derived truth separate. CPU or GPU scheduling does
not change which interpretations qualify as answer sets.

For a general formula reduct, such as the disjunction `a | b.`, there need not
be one least model. zetesis instead asks whether a proper subset of the
candidate satisfies its frozen reduct. The [next chapter](semantics.md) makes
that distinction precise.

To follow the actual entry points, see the
[writer-free session example](../rust/sessions.md),
[`zetesis_themelios::admit`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-themelios/src/lib.rs),
[`zetesis_cpu::check`](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-cpu/src/oracle.rs),
and the [finite formula API](../rust/reducts.md).
