# Reusing command workflows

The command line is one consumer of the libraries. A Rust application need not
construct argument strings, launch a zetesis process or parse its human output
to solve a program or measure a primitive. The command adapters map user options
to the same typed requests available to library callers.

| Task | Library operation | Result or observation |
| --- | --- | --- |
| Solve admitted input | `zetesis_solve::Session` | Verified answers and a separate semantic outcome |
| Publish answers | `zetesis_cli::publish_prepared` | `AnswerView` callbacks and independent publication evidence |
| Compare the corpus with clingo | `zetesis_validation::corpus_comparison::run_with_invocation` | A report with typed decisions for each source |
| Check a backend through the CLI | `zetesis_validation::backend_check::run` | Bounded process evidence and complete-family checks |
| Measure corpus workloads | `zetesis_validation::performance::command::run` | A matrix report with every planned position accounted for |
| Measure a primitive | `zetesis_experiments::primitives::measure` | Typed preparation, sample and completion events |
| Compare saved measurements | `zetesis_validation::performance::series::read_compare` | A typed comparison after bounded decoding and identity checks |
| Inspect GPU adapters | `zetesis_wgpu::discover_adapters` | Advertised capabilities; no claim of successful execution |

The executable comparison workflows deliberately launch bounded solver children.
That is part of their task: they test the process interface and include its
startup, input and output costs. They are not required for an embedded solve.
The new command adapters select the public `solve` interface for the installed
executable; legacy argument spelling remains available for comparing an older
binary. Selecting an interface does not alter the requested semantic task.

## Separate measurement from presentation

Requests name inputs, execution policy and resource bounds. Outcomes retain
failures, refusals and incomplete work explicitly. Human tables and JSON are
views of those outcomes. A renderer cannot promote partial enumeration into a
complete `WorldView`, infer device execution from a requested device, or replace
an unavailable duration with zero.

Answer publication uses the streaming
[`AnswerRenderer` boundary](outcomes.md). Observation computes `#show` before the
view receives an answer. The view retains access to the full interpretation,
shown atoms, evaluated terms and cost independently.

Statistics use solver-owned `PhaseTimings` and `SemanticOutcome` values. The
default human statistics view accepts these by reference through
`zetesis_cli::statistics_view::Statistics`. Other consumers can derive their own
views directly. Statistics output does not repeat solving or start a timer.
Its execution and compact work tables project retained receipts, never fill
missing actual execution from requested policy. Optional instrumentation being
disabled does not erase mandatory search accounting. Each work row states its
scope; completed-check closure totals omit stopped-check partial work, and
decoded GPU work excludes submitted batches whose results did not return.
The complete field catalog remains available in the typed values and JSON.

`zetesis-presentation` supplies explicit color roles and text tables. It performs
no terminal or environment discovery and knows no answer-set semantics. The
process adapter resolves each output stream's terminal capability. Generic
library writers remain plain under automatic styling; a caller can request
styling explicitly. Narrow tables retain their values in a vertical layout.

Primitive measurements use synchronous observers outside measured intervals.
The human adapter renders typed observations; the JSON adapter streams the
profile's event schema. A failed observer stops publication. A missing completion
event is not a completed benchmark. Corpus reports can produce a compact typed
summary without copying their raw answer streams.

## Scope of checks

An installed backend check is a small conformance suite over known programs. It
does not replace the maintained Rust tests, coverage checks, Lean build or full
physical qualification. Corpus comparison checks the selected corpus and its
reported answer contracts; it is not a proof for every admitted program.
The [validation reference](../reference/validation.md) describes those separate
obligations.

The primitive command currently exposes the relation, aggregate, tight and lazy
profiles with typed event streams. Historical static/formula experiments retain
their compatibility executable. CPU-only solver builds do not compile the
experiment crate and report that primitive command as unavailable; corpus
validation, corpus measurements and saved-report comparison remain available.
