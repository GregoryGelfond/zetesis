# Working with finite reducts

`zetesis-ferraris` accepts an already finite formula DAG. Every edge refers to a
preceding node. Roots are the formulas asserted by the theory; their conjunction
is the semantic input. `Theory::new` checks this shape and transfers the vectors
without grounding or searching.

Here atom indices `0` and `1` denote `a` and `b`. The theory is `a | b.`.
The candidate `{a,b}` satisfies the theory but has a proper-subset reduct model:

```rust
{{#include ../examples/reduct.rs:example}}
```

The three calls answer distinct questions:

- `models(T, M, …)` checks classical satisfaction of the original theory.
- `models_reduct(T, M, J, …)` checks `J` against the reduct frozen at `M`.
- `check(T, M, …)` checks original satisfaction and searches proper subsets for
  a witness against minimality.

The public reduct query permits arbitrary `J`, including interpretations not
contained in `M`. The membership checker imposes the proper-subset condition.
Its exhaustive subset enumeration is useful for small exact checks and reference
comparisons; use the ordinary session for the composed native search.

## Preserve the subject

An `Interpretation` belongs to one immutable `Theory`. Cloning the theory shares
identity. Independently constructing equal node lists produces another identity,
and passing an interpretation from it returns `Stop::WrongProgram`.

`check` returns completed verdict data: accepted, not an original model, or
nonminimal with a witness. For an owned decision tied to the candidate actually
checked, use `check_interpretation`. Its private construction prevents attaching
an unrelated decision to a different candidate. A `StableInterpretation` is
runtime evidence of a completed native check, not a Lean proof object or a
certificate that enumeration finished.

## Cost and limits

Formula evaluation walks the topological nodes and asserted roots. The current
`models_reduct` convenience operation evaluates the candidate and tested
interpretation on each call. A caller must account for that repeated work.
`Limits::max_work` bounds charged operations; `max_subsets` bounds the reference
membership check's proper-subset queries. Cancellation and deadlines use
`zetesis_cpu::Control`.

Every operation returns a typed stop on exceeded limits or invalid identity.
An error is not the Boolean value `false`. In particular, exhausting the subset
budget cannot establish an answer set. The source compiler's finite coverage
and aggregate translations remain separate from this already-admitted DAG API.

