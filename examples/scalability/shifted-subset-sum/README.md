# Subset sum with shifted weights

Select distinct items whose weights range from `n+1` to `2*n` and sum to
`target`. This authored workload uses an ordinary choice rule and `#sum`, with
item identity included in each aggregate tuple. There is no objective.

The default `n=8, target=17` is unsatisfiable: a singleton weighs at most 16,
while the smallest pair weighs 19. The nearby target 19 has exactly one answer,
`selected(1) selected(2)`. More generally, for `n>=2`, target `2*n+1` is
unattainable and target `2*n+3` selects exactly items 1 and 2. Every selection
of three or more items exceeds either target.

These cases distinguish an unattainable sum from its surrounding numeric
interval. Adjacent weights have greatest common divisor one, so divisibility
alone cannot rule out the impossible target. The original
[subset-sum example](../subset-sum.lp) remains a complementary case with
weights `1..n`.

[manifest.json](manifest.json) seals the unchanged source and its complete
zero-answer default contract. From the repository root:

```sh
zetesis solve examples/scalability/shifted-subset-sum/subset_sum.lp --all
zetesis-bench run --workloads examples/scalability/shifted-subset-sum/manifest.json \
  --compare-grounders --compare-threads 1,14 \
  --repetitions 3 --memory-runs 0 \
  --timeout-seconds 120 --campaign-seconds 1800 --report shifted-subset-sum.json
```

The existing matrix requests complete enumeration and qualifies every native
answer family against a fresh clingo run and the recorded contract. Clingo uses
one thread and serves as the qualification reference, outside the native timing
distributions. Incomplete results remain explicit in the report.

This workload is selected separately; it does not change the default scalability
or correctness populations. The existing `authored_examples::` tests check source
integrity, the complete default contract and checked parameter edits. The
maintained oracle gate checks both targets at `n=8` and `n=30`. Native comparisons
use the matrix above. Larger parameter studies can use the existing
`Workload::authored` and `ConstantAmendment` APIs, which record the expected old
values and require fresh reference qualification after an edit.
