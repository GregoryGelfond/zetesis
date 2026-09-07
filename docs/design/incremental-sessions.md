# Incremental Rust sessions

Multi-shot execution is an optional architectural extension, separate from the
current single-shot compatibility and performance milestone. Theory atoms remain
out of scope for now. Python and Lua scripting are not required for a native Rust
session API. This document specifies obligations; it introduces no implemented
session API or compatibility claim. The public Rust API belongs to themelios;
zetesis would implement its engine contract. The
[integration note](themelios-solve-integration.md) maps existing mechanisms to
themelios's draft vocabulary and records future Rust `@` function support.

Lazy candidate construction and repeated solving address different costs. A lazy
solve can avoid materializing unused bindings for one problem. A session can
reuse valid work across related problems, such as successive planning horizons
or new observations. Its value should be measured against repeated independent
solves on the same sequence of original programs.

## Snapshot contract

A session owns a sequence of immutable program versions. A successful update
publishes one complete version atomically; a refused update leaves the previous
version available. A solve names exactly one version and returns version-bound
models, optimum information and coverage. Data arriving during that solve is
queued for a later version or causes an explicit interruption. It never changes
the candidate truth mask or reduct midway through a membership check.

A first correct implementation may recompile and solve each version from scratch.
That gives the reference behavior against which incremental reuse is checked.
Possible operations include adding source/data, selecting supported program parts,
changing external assignments and solving again. Their exact semantics require
separate admission and scope contracts; current refusal of `#program` and
`#external` is not removed by this proposal. Static one-shot meanings of those
directives remain distinct compatibility work. The existing
[clingo Control interface](https://potassco.org/clingo/python-api/5.8/clingo/control.html)
provides a behavioral comparison target, rather than a required implementation
language or runtime dependency.

## Reuse requires evidence

| Retained object | Reuse obligation after an update |
| --- | --- |
| Parsed files, constants and provenance | Content and global resolution context are unchanged |
| themelios analysis | The analyzed normalized program is unchanged, or affected dependency/class facts are recomputed soundly |
| Possible-support relations and binding indexes | Every current possible tuple remains covered, with current scalar semantics and index identities |
| Factored body and aggregate eligibility graphs | The complete input relations, scopes, tuple keys and original atom mapping still match |
| Candidate blocks | The excluded candidate has already been accounted for in this version's requested enumeration; previous-version delivery is insufficient |
| Incumbent and objective bounds | The model is reverified for the current reduct and reevaluated under current objective keys/priorities |
| Reduct masks and countermodel results | Original theory identity and outer candidate are exactly unchanged |
| GPU graph/transport buffers | Graph identity or validated remapping matches; mutable per-candidate state is reset before reuse |

Adding facts is not generally monotone for answer sets: default negation can
invalidate previous models. Deletions and changed external values can also
invalidate derived support. Dependency-local recomputation therefore requires a
semantic composition argument, not just a graph reachability heuristic. The
safe baseline discards semantic search evidence across version changes while
retaining only independently justified representation or allocation reuse.

Lean refinement should state that each version's completed outputs, optimum and
coverage agree with a fresh solve of that version. A separate reuse theorem must
establish every retained object's invariant under the admitted update relation.
Kernel acceptance continues to mean original modelhood and absence of a proper
subset satisfying the current candidate's Ferraris reduct. Session history never
supplies atom support.

Benchmark sequences should report update/analysis/materialization costs, reused
state and bytes, first-answer latency, full completion, optimum ties and memory
across the sequence. A single cold solve cannot establish an incremental benefit.
