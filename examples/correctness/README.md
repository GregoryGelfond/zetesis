# correctness examples

These are ordinary, self-contained ASP examples adapted from
[kr-domains](https://github.com/GregoryGelfond/kr-domains) at revision
`38f0660ded448ed268c5a68759ceb0e2840dd497`. They cover shortest paths, task
allocation, traveling-salesman variants, equality generalized TSP, six
N-Queens encodings and SEND + MORE = MONEY.

The collection contains 14 shared encodings, 87 scenarios and 7 standalone
programs: **94 runnable cases in 108 source files**. It excludes the original
clingcon encodings and their dependent scenarios. [LICENSE](LICENSE) preserves
the upstream MIT license and Gregory Gelfond copyright.

## Run an example

From the zetesis repository root, using the installed solver:

```sh
zetesis examples/correctness/scenarios/task-allocation/variant-01/01-basic.lp --models 0
zetesis examples/correctness/scenarios/shortest-path/variant-01/01-basic.lp --stats
zetesis examples/correctness/standalone/n-queens/variant-01.lp --models 0
```

The task-allocation example assigns `a1` to `t1` and `a2` to `t2`, with total
cost 5. The shortest path uses the two edges through `b`, with total cost 2.
The eight-queens example has 92 answer sets. `--models 0` requests complete
ordinary enumeration or every final optimum tie, as applicable.

Each scenario includes its shared encoding through an unchanged relative path.
Run the scenario file directly; no external checkout, generated instance,
elenctic installation or annotation interpreter is required. Keep the directory
layout when copying scenarios. The `encodings/` files describe input signatures
and are intended to be paired with instance facts; the `standalone/` programs
can run by themselves. All six N-Queens variants use `#const n = 8`;
the recorded contracts apply to the default instance. Change that declaration
to select a different board size.

## Sources and test contracts

The original leading elenctic annotation comment lines were removed. The local
N-Queens variant 01 also replaces its fixed board size with `#const n = 8`,
matching the other variants. Its default 92-answer contract is unchanged.
All remaining bytes and include spellings are preserved.
The curated files have **different byte identities and
physical line numbers** from annotated originals; they are not presented as
byte-exact originals.

[manifest.json](manifest.json) records both SHA-256 identities for every source,
the upstream revision and historical reference toolchain, include dependencies,
and each deleted comment's original line, byte span and text. Schema 2 also
records exact before/after bytes and coordinates for local edits, applied after
comment deletion. Historical
annotated originals remain in `validation/corpus/kr-domains` for provenance.
The historical toolchain records how the contracts were authored; it is not
an execution receipt for the current solver.

The manifest also contains typed expectations for satisfiability, selected model
count, final cost, displayed witnesses and symbols required in every selected
display. Optimizing examples select final optimum ties; ordinary examples select
the complete answer-set family. Printed symbol multiplicities and model counts
are preserved. These are contracts on **reported displays**, not a claim to
reconstruct hidden interpretations or implement general cautious reasoning.
Notes are retained as explanation and impose no solver rule.

The solver receives only ASP source. The independent Rust validation library,
`zetesis_validation::examples`, owns loading, integrity checks and display
contracts. Its normal loader uses this directory alone. The explicit
`verify_originals` audit additionally reads retained originals, checks exact
comment deletion and local edits, and confirms the typed translation of every
original contract. This byte-derivation audit does not prove semantic equivalence;
the external comparison below checks default-instance answer agreement.

## Reproduce the checks

The installed integrity command verifies the clean collection independently:

```sh
zetesis-corpus verify-examples examples/correctness
zetesis-corpus verify-examples examples/correctness --originals validation/corpus/kr-domains
```

The second command also verifies the recorded derivation from preserved originals.
Neither integrity command runs a solver. For the full native/clingo comparison:

```sh
zetesis-validate --repo . --report target/correctness-parity.json
```

The validator loads these examples by default. The Rust regression suite is:

```sh
cargo test --locked -p zetesis-validation --test integration example_corpus::
```

For an independent original-versus-clean clingo comparison, use clingo 5.8.2 on
`PATH`, or set `CLINGO` to its absolute path:

```sh
CLINGO=/absolute/path/to/clingo cargo test --locked -p zetesis-validation \
  --test integration example_parity:: -- --ignored --nocapture
```

The opt-in test runs all 94 cases from both trees with bounded capture,
`--models=0 --opt-mode=optN`, checks successful completion and compares selected
display multiplicities, costs and all typed contracts. It is a source-cleaning
qualification, not a benchmark or a complete native-language compatibility claim.
The maintained [parity test](../../crates/zetesis-validation/tests/integration/example_parity.rs)
implements this check.
