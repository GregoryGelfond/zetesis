# Disjunction reference fixture

[`disjunction.json`](disjunction.json) contains 49 hand-authored source programs,
their independent formula theories and expected semantic records, and 62 external
clingo runs. Each `source` is the exact input text; `sha256` hashes its UTF-8 bytes.
The `path` field is a logical source identity, not an external file dependency.

The raw records are fresh executions of PATH-resolved `clingo` 5.8.2. Every source
was supplied unchanged on stdin using its recorded mode:

```text
clingo - --models=0 --outf=2 --warn=none --opt-mode=optN
clingo - --models=0 --outf=2 --warn=none --opt-mode=ignore
```

Each `raw` record retains the actual argument vector, exit code, standard output
and standard error. The executable SHA-256 is
`31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015`.
`reference_binary` retains the version command and its output;
`reference_capture` records capture times, limits and comparison scope. Each
execution had a five-second timeout, a four-MiB bound per output stream and an
eight-MiB combined output bound. These are qualification limits, not solver
language limits.

This capture replaces fixture SHA-256
`9886783789b7c128c169bb0ab55b150d2f94aa8cde3458b34a923d08cbdd32e5`.
All source bytes, source hashes, independent theories, expected records and
normalized results are unchanged. The comparison also checked full witness/cost
multisets, including multiplicity and absent cost vectors, exit codes and
completion metadata. Diagnostics agree after replacing the old input label with
the stdin label; the stored diagnostics themselves are unmodified new output.

There are 48 completed satisfiable runs, 12 completed optimization runs and two
unsafe-source failures with exit code 65 and `UNKNOWN`. Those failures are
reference refusals, not completed unsatisfiability results. Native admission
classifications in this fixture retain their original regression scope.
`disjunction.rs` excludes subsequently supported cases from its refusal assertions;
separate tests cover those features. The fixture is not a current language
compatibility inventory.

From the repository root, run the independent formula and native checks, then
optionally replay all 62 reference modes with an installed clingo:

```sh
cargo test --locked -p zetesis-themelios --test integration disjunction::
CLINGO=clingo cargo test --locked -p zetesis-themelios --test integration disjunction:: -- --ignored --nocapture
```

The replay compares complete full models, optimum costs, priority presence and
reference failures against the captured output. It does not overwrite the
fixture. Clingo remains an external test oracle and is not a production solver
dependency.
