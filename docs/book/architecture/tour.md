# A guided tour

Consider a choice between two alternatives:

```asp
a :- not b.
b :- not a.
```

Its answer sets are `{a}` and `{b}`. The complete world view is therefore
`{{a}, {b}}`. Running `zetesis choices.lp --models 0` requests both; stopping
after the first answer does not establish that complete family.

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

