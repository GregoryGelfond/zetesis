# Source programs and grounding

Source processing and solving are separate library capabilities. themelios owns
syntax, logical terms, program structure and source provenance.
`zetesis-themelios` checks the supported semantic profile and lowers it into
relational templates or a finite formula theory. It also retains objective and
observation data associated with that representation.

Successful parsing is only the first boundary. Profile admission, binding
safety, arithmetic evaluation and resource limits can still fail. A frontend
syntax diagnostic, an unsupported zetesis construct, and an exceeded resource
ceiling have distinct meanings; none denotes an inconsistent program.

## Eager and lazy execution

For admitted relational programs, **eager** grounding explicitly compiles a
complete bounded graph. This is useful when repeated candidate checks amortize
its construction. `GroundProgram::compile` is the operation that creates this
graph; constructing a `Program` or `Seed` does not silently invoke it.

**Lazy** relational execution retains source templates and streams relevant
instances during positive inference. It can avoid storing the complete ground
program. Candidate generation still owes coverage of all relevant gate choices,
and each completed closure owes coverage of all instances enabled in its final
snapshot. Laziness moves and limits materialization; it does not remove these
obligations or guarantee smaller memory use on every input.

The finite formula source path currently materializes a bounded theory. Explicit
lazy formula grounding is unsupported. This is a current implementation boundary,
not a theorem that general formulas require eager grounding. Automatic selection
chooses among implemented profiles; it does not relax admission limits to obtain
an answer.

## Source instances as a composition

The useful lower-level operations have logical contracts:

| Operation | Input and result | Obligation |
| --- | --- | --- |
| Bind | Join finite positive witnesses into substitutions | Preserve repeated-variable agreement and local scope |
| Filter | Evaluate scalar conditions on bound values | Never invent missing bindings or use undefined arithmetic as a value |
| Gate | Test frozen positive/negative candidate conditions | Use the candidate, not the growing consequence set |
| Project | Construct a head or constraint instance | Preserve the complete atom and its source instance |

An atom in a **possible support relation** is a witness available to source
enumeration. It is not thereby true in a candidate, and an aggregate's proposed
result is not thereby its evaluated result. The emitted formula must retain the
original activation and equality conditions. Structured positive witnesses can
bind local variables before dependent arithmetic is checked; this does not
permit arithmetic inversion or a global guessed value universe.

The distinctions matter to parallelism. A union of several worlds' relations
can offer shared source instances, but a join can combine atoms that coexist in
no individual world. Each world must therefore recheck positive truth. Optional
world-membership masks prune a prefix only if no current world satisfies it.
They do not prune future snapshots or the candidate carrier.

See [lazy checking](../rust/parallel.md) for the injected execution contract,
and [the theorem map](../lean/theorems.md) for the corresponding coverage laws.

