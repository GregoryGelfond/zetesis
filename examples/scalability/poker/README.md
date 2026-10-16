# Draw poker

A user-contributed enumeration regression: select a five-card hand whose best
category is a nonroyal straight flush. The complete family has 36 answer sets,
one for each of nine low ranks in four suits. An ace can be low or high; the
four royal flushes have category 10 and do not satisfy the required category 9.
There is no objective.

[manifest.json](manifest.json) records all 36 displayed answers, each containing
five `hand/3` atoms and one `top_hand/5` atom, together with the source hashes and
include graph. The workload exercises choices, arithmetic, nested maximum
assignments and complete enumeration.

## Run and compare

From the repository root, with zetesis and clingo installed:

```sh
zetesis solve examples/scalability/poker/draw_poker_tests.lp --all
zetesis-bench run --workloads examples/scalability/poker/manifest.json \
  --compare-grounders --compare-threads 1,14 \
  --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report poker-timing.json
```

The benchmark uses the existing workload matrix to compare eager and lazy
grounding at both thread counts. It requests complete enumeration, qualifies
each displayed family against a fresh clingo run and the recorded contract,
and compares full native answer identities across profiles. Clingo uses one
thread. It is a qualification reference in this matrix; it is not included in
the native timing distributions. The report records its qualification time
separately.

The command retains ordinary solver resource defaults. Its process and campaign
deadlines bound the measurement; incomplete results remain in the report. Choose
a new report filename for each run. This included-source bundle requires clingo
and does not support `--without-clingo`. It is an explicit maintained workload,
outside the default twenty-case scalability population and the 94-case
correctness corpus.

`scripts/check.sh oracle` checks the complete clingo family. The ordinary
`authored_examples::` fixture tests check source integrity and the exact display
contract without launching a solver. Native comparisons use the benchmark command
above, so the portable gate does not acquire a long enumeration test.

## Provenance

These files were supplied by a user for inclusion in the maintained regression
workloads. No author or license statement accompanied them. Filenames were
normalized for portable commands: `draw_poker_1.lp` became `draw_poker.lp`, and
`draw_poker_tests 1.lp` became `draw_poker_tests.lp`. The sole content edit changes
the latter's `#include` to `draw_poker.lp`; all other source bytes are preserved.
Commented examples and timing notes are retained from the supplied files and are
not measurements of the current zetesis version.

| File | SHA-256 |
| --- | --- |
| Supplied `draw_poker_1.lp`; maintained `draw_poker.lp` | `fd7fda56bf940ece43621831c79a34a0db77648774d65e95692dc96ae2d52eef` |
| Supplied `draw_poker_tests 1.lp` | `5ddbdee02e9ce7c131f7c9280f51ae1ee14ebe8a751dea33984bc3a51acac3ac` |
| Maintained `draw_poker_tests.lp` | `3d0f76216569de5dd65f789edfb354ce59b5c08ddea6a098cea2467b2628fc4a` |
