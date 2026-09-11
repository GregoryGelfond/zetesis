# Costs and shown terms

Answer-set membership, objective costs and output observations answer different
questions. Membership uses the original theory and its frozen reduct. An
objective reads the complete interpretation to compute a score. An observation
reads that same interpretation to construct displayed terms. Neither operation
supplies support for an atom.

The example chooses one of two sites. Choosing east costs three units plus a
two-unit penalty for not choosing west. Output adds two to the site's input
value for display; this arithmetic does not alter the objective or introduce
new atoms. Complete collection retains both answers, including the more costly
one.

```rust
# extern crate zetesis_solve;
# extern crate zetesis_core;
# extern crate zetesis_cpu;
# extern crate zetesis_themelios;
{{#include ../examples/costs-and-output.rs:example}}
```

Use [optimal selection](outcomes.md#check-selection-and-complete-capture) when
only optimum ties are wanted. A score attached to one answer does not prove
optimality; that conclusion also requires completed search. Identical displayed
terms do not merge distinct answers.

## Library boundaries

`zetesis-objective::ObjectiveTemplate` describes an objective contribution.
Positive relational bindings and scalar filters can be combined with a closed
`Condition` over complete typed atoms. `ObjectiveProgram::new` checks the
condition's acyclic references and admission limits. `evaluate` reads the
supplied model; the caller establishes whether that model is an answer set.
It uses one contribution-key and priority-ordering contract for all templates.

Source priority presence is a separate preparation result. A retained priority
may have cost zero in every answer. In particular, a condition's false value
on the current answer is not permission to remove its priority slot. The
[language reference](../reference/language.md) states the admitted finite
source profiles and their remaining boundaries.

`ObservationProgram::evaluate` returns typed shown symbols;
`ObservationProgram::render` composes these with the selected atom channel.
Both preserve complete model identity. A term and an atom with the same printed
text remain separate output contributions. Consumers may retain typed values
instead of rendering them.

Observation limits separately bound work, completed bindings, constructed
symbols, local owned payload and output. The local byte budget counts semantic
nodes and text; it excludes borrowed model values and allocator overhead.
These are deterministic operation budgets, not process-memory limits. Reached
undefined arithmetic, cancellation or a resource refusal returns an error,
without a partial observation. An observation error does not invalidate an
already verified answer set or complete world view.

The [Lean correspondence](../lean/correspondence.md) distinguishes the laws for
original-model conditions and contribution transport from the remaining source,
Rust and device refinement obligations.
