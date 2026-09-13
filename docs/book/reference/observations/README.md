# Historical release observations

The [observation data](release-6bebb980-1e5b78ce.json),
[provenance](release-6bebb980-1e5b78ce-provenance.json) and
[table view](release-6bebb980-1e5b78ce-tables.md) reproduce the canonical release
comparison in the [performance reference](../performance.md#canonical-release-comparison).
They describe source `6bebb980f9c102dbb7f943076d7cde92374841ce` versus
`1e5b78ce913ab3aeece6ed496f69ca8176f0644d`, measured on 12 September 2026.
They do not qualify a later implementation.

This is a derived observation view, not a byte-identical archive of the original
reports. It retains all 468 ordered observation receipts across nine workloads
and four blocks, including qualification, warmup, timing, diagnostics and memory
positions. The provenance records original report hashes and byte lengths,
source and executable identities, corpus files, the build recipe, platform and
the actual observation limits. The original reports remain retained separately.
Machine-local paths, process IDs, output streams and diagnostic payloads are
omitted. These files do not publish the other Metal or LTO comparisons.

`decision: "pass"` records the original comparison's conclusion: complete
selected displays, including symbol and model multiplicities, final costs and
every optimum tie, matched the pinned corpus contract and clingo. The retained
selected counts and costs do not independently prove that conclusion. Native
human output and clingo JSON have different encoding costs; hidden full
interpretations were unavailable to this ordinary comparison. Rendering the
tables does not rerun the solvers or re-establish output parity.

Each producer/case/block has exactly three timed nanosecond observations and
one separate child-RSS observation in bytes. Other elapsed values remain in
their own populations. The table uses the middle sorted time, minimum and
maximum; it never pools blocks. RSS comes from a fresh resource helper, excludes
that helper, and can include usage propagated by waited descendants. It is not
simultaneous process-tree RSS or device memory. For memory positions the capture
exit belongs to the helper, while `memory.exit_code` belongs to the solver;
both are retained. The small populations provide neither confidence intervals
nor a general speedup claim.

Run the maintained Rust view from the repository root:

```sh
cargo run --locked -p zetesis-validation --example release_observations
cargo run --locked -p zetesis-validation --example release_observations -- --check
```

The default command prints the three published Markdown tables. `--check`
validates without writing them. Both use only the embedded data: they check the
linked provenance digest, exact historical source/binary bindings, all positions
against `zetesis_validation::performance::Schedule`, recorded completion and
cost consistency, and the separate RSS receipts. Missing, extra, duplicate or
reordered positions refuse table generation. Integer formatting rounds to
three decimal places with ties to even, without converting raw units through
floating point. The example also requires equality with the retained published
table view; its regression checks that view against the manual.

Direct integer rounding corrects eight final displayed digits from the earlier
presentation of these same observations. For example, 32,871,500 ns is exactly
32.8715 ms and rounds to 32.872; 6,094,848 bytes is exactly 5.8125 MiB and rounds
to 5.812. The raw observation and provenance JSON remain unchanged.
