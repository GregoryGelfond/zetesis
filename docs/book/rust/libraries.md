# Choosing a library boundary

Use the narrowest capability that represents your input and the question you
need to answer. Parsing command-line arguments is not required to use the solver.

| Input or task | Library and entry points |
| --- | --- |
| Original ASP source | `zetesis_themelios::admit`, `admit_extended`, `admit_formula`, and their bundle APIs |
| Ordinary solve over an admitted owner | `zetesis_cli::{PreparedInput, Session, SolveConfig}` |
| Finite relational templates and atoms | `zetesis_core::{Program, Template, Atom, Seed}` |
| Explicit complete relational graph | `zetesis_core::GroundProgram::compile` |
| Normal reduct membership | `zetesis_cpu::{check, check_static, BatchOracle}` |
| Finite formula construction and reference membership | `zetesis_ferraris::{Theory, Node, Interpretation, check}` |
| Repeated queries against one candidate's reduct | `zetesis_ferraris::FrozenReduct` |
| Native formula candidate/countermodel search | `zetesis_sat` |
| Bounded device execution | `zetesis_wgpu` |
| Model-relative objective evaluation | `zetesis_objective` |
| Source-domain analysis | `zetesis_domain` |
| Reproducible comparisons and measurements | `zetesis_validation`, `zetesis_experiments` |

Despite its current crate name, `zetesis-cli` exposes a writer-free ordinary
solver session. It is the appropriate existing entry point when an application
wants the solver's composed behavior. The lower libraries remain usable
independently; a caller building a theory need not parse source, and a caller
preparing a program need not search it.

`zetesis_themelios::{base, syntax, logical, analysis}` re-export the corresponding
themelios tiers. Use their terms and source identities at the source boundary.
A formula atom index or dense relational ID is an execution representation,
not a replacement source symbol or a transferable identity across admissions.

## Owners preserve coherence

`PreparedInput::admitted` borrows an `Admitted` owner; `PreparedInput::formula`
borrows an `AdmittedFormula`. Bundle variants retain multi-file provenance.
The formula owner keeps the theory, atom indexing, objective program and
observations together. Do not build a session by independently pairing a theory
with an atom table from another admission.

`PreparedInput::ground` borrows an `Arc<GroundProgram>` that retains its original
program identity. It reuses the supplied graph; an explicit request for lazy
grounding or a formula oracle is incompatible with this input.

Current APIs do not promise a themelios-solve backend implementation, a general
ASPIF interchange boundary or a custom theory-propagator interface. Those
capabilities require additional contracts. The existing admitted-owner and
session boundaries are useful without pretending those interfaces are present.

Generate the [local Rust API reference](../../doc/zetesis_cli/index.html) as
described in [Building the documentation](../building.md). Public signatures and
their per-operation cost and error contracts are authoritative.
