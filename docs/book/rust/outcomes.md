# Completion, resources, and output

Keep three questions separate: what has been proved about the program, how much
search has completed, and what an external consumer received.

| Evidence | What it establishes | What it does not establish |
| --- | --- | --- |
| Verified model | Completed membership for the original subject | Exhaustive enumeration or optimality |
| Evaluated score | Cost of that verified model | A globally best cost |
| Exhausted search | Relevant candidate coverage completed | Successful external publication |
| Proved optimum | Complete relevant search establishes the incumbent | Every optimal tie was delivered |
| `WorldView` | All full answers of the original subject were captured after unrestricted exhaustion | Successful external publication |
| Published record | The sink accepted one whole record | Flush, durability or complete search |

`SemanticOutcome::unsatisfiable()` requires exhausted coverage and zero verified
models. A zero display count, an empty consumer vector, or `completion() == None`
cannot establish inconsistency. A requested model limit and an interrupted
search are distinct completion states.

`SemanticOutcome::selection()` identifies the family requested by the session.
`All` ranges over the original program; `Optimal` permits sound exclusion of
worse candidates. `Completion::Exhausted` therefore does not by itself identify
the unrestricted family, and an optimum proof does not prove that a consumer
retained every tie. `WorldView` owns both unrestricted enumeration and complete
capture; callers cannot attach an arbitrary vector to this evidence.

Objective presence is also semantic data. An absent objective is not an active
objective whose cost happens to be zero. Preserve the priority structure and
optional score in typed results and comparisons; flattening both cases to an
empty or zero vector can conceal a disagreement.

Unrestricted enumeration evaluates scores without retaining incumbents. Its
`scored_models()` can be positive while `incumbent()` is absent and
`retained_models()` is zero. Those fields describe objective-search retention,
not the number of answers in a `WorldView` or a consumer's own collection.

## Resource contracts

Each operation names the resources it bounds: source bytes, syntax depth, atoms,
substitutions, nodes, work, candidate occurrences, retained results or transport
storage. Zero is a real ceiling where those limits apply; it does not mean
unlimited. Work and storage limits are independent.

`Control` is cloneable shared cancellation with an optional absolute deadline.
Cancellation is observed at cooperative polling boundaries, not by forcibly
terminating arbitrary work. In particular, an already-started bounded static
compilation is not preemptible; session setup polls before it and subsequent
work polls again. A deadline is therefore not a hard process-kill guarantee.

Admission limits also do not retroactively bound vectors constructed by the
caller. A documented logical payload budget excludes what its contract names,
such as allocator bookkeeping or worker threads. It must not be reported as
process RSS. Read the operation's own limit type rather than assuming one global
memory bound covers the entire solver.

## Views and failures

Use `Session` when the consumer wants semantic values and owns its presentation.
Use the `run_finalized` family when it wants the ordinary source driver and an
injected output sink. `SolveReport` separates semantic evidence from
`Publication`. A writer failure can coexist with already established exhaustion;
it cannot retract that proof, and it cannot claim a partially written record as
fully published. `SolveFailure` preserves the original cause and available
semantic, publication and timing evidence.

The CLI's `--json` output is a versioned view. Full semantic atoms, shown atom
indices, shown terms and costs remain separate. Human output and JSON do not
define different solving modes. Applications should consume typed library
values or the JSON contract rather than parse styled answer lines.

`--stats` reports requested policy separately from observed execution and
distinguishes unavailable measurements from zero. Eager grounding and solving
can be measured as separate stages; lazy source work occurs within membership.
Host intervals around device calls include transport, waits and readback.
Enabled session elapsed time can include the consumer's delay between pulls;
active solving spans do not include that delay. These scopes matter when using
the same library in a server or comparing it with a command-line run.
