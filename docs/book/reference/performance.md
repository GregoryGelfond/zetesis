# Execution performance

The latest prepared-grounding comparison demonstrates reused query capacity and
less repeated scalar grounding work. Ordinary CPU timings are mixed, with little
RSS change and some small library regressions. Representation and work reductions
do not establish a general speedup. The observations below distinguish complete
implementations, execution routes and source revisions; earlier Metal results
do not qualify the latest source.

## Prepared-grounding CPU comparison

Three compiled sources were measured on arm64 macOS on 14 September 2026.
Each measured executable reports version `0.1.0`; these measurements identify
the binaries below, not a later build carrying version `0.1.1`.

| Role | Source | Native executable SHA-256 |
| --- | --- | --- |
| Earlier atom catalog | [`ca10a5e7`](https://github.com/GregoryGelfond/zetesis/tree/ca10a5e7ec84e13fbcc4a23bd0de8b0232c53fe1) | `fd19a078e99c75c1bbaf30e437f5da02aa621fd595554b079bb3bc0b078dae0e` |
| Intermediate workspaces | [`f56a5a24`](https://github.com/GregoryGelfond/zetesis/tree/f56a5a2496f519d7b71b7c4c8fdc166c355874ff) | `fb4d784313f4b0c9a5078712728e70f17d44c545eae4a6ea67180ec7a49bbb29` |
| Latest prepared grounding | [`679ca856`](https://github.com/GregoryGelfond/zetesis/tree/679ca8568a6fd8577d9b944fbd99d7c54f666601) | `a1d8cd7c640bab9b2a57f2e9dd612ff391c39b77f6dc9be95dbea0f890c13bd2` |

All use Rust 1.97.1 and ordinary release optimization without a CPU/LTO override.
The earlier and latest builds explicitly target `aarch64-apple-darwin` and select
CLI, experiments and validation; the intermediate build selects CLI and
experiments on the native Apple target. Top-level GPU features agree, but the
retained evidence does not establish full transitive build equivalence. These
are integrated application comparisons, not isolated algorithm effects.
The fixed performance runner hashes to
`c83264ce4f459287d79042061a0320884117385ff876185373ae5ba168594d89`;
clingo 5.8.2 hashes to
`31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015`.

The acquisition order is earlier/intermediate/latest/latest/intermediate/earlier.
Each of six blocks contains 117 positions across six N=8 queens encodings, SEND,
task allocation variant 04/scenario 05 and shortest path variant 01/scenario 06.
Each producer/case has qualification, one warmup, three timed runs and one
separate RSS run; native diagnostics add one position per case. All **702 unique
positions** pass, without capture faults or unresolved children. Qualification
checks complete selected displays, multiplicities, final optimum ties and costs;
it does not independently compare hidden full interpretations.

Two overlapping four-block views retain every original position. The
[earlier-to-latest observations](observations/release-ca10a5e7-679ca856.json) and
[their provenance](observations/release-ca10a5e7-679ca856-provenance.json) compare
atom-catalog and prepared-grounding sources. The
[intermediate-to-latest observations](observations/release-f56a5a24-679ca856.json)
and [their provenance](observations/release-f56a5a24-679ca856-provenance.json)
compare reusable workspaces with the subsequent algorithms. Each view has 468
positions; the two latest blocks are shared, so these are **702 observations,
not 936 independent observations**. These derived receipts retain exact integer
samples, identities and original report hashes, with streams and machine-local
paths omitted. They neither qualify later code nor publish the separate library
or device reports. Reproduce both complete table views without running a solver:

```sh
cargo run --locked -p zetesis-validation --example release_observations -- --dataset release-ca10a5e7-679ca856
cargo run --locked -p zetesis-validation --example release_observations -- --dataset release-f56a5a24-679ca856
```

Add `--check` to validate without printing. The historical default selection is
unchanged. For new acquisitions, use the nine-case `ordinary` function in
[Reproduce the measurements](#reproduce-the-measurements), choosing the fixed
runner and the three executable paths above. All six calls use `indexed`:

```sh
catalog_solver=/absolute/path/to/ca10a5e7/zetesis
workspace_solver=/absolute/path/to/f56a5a24/zetesis
prepared_solver=/absolute/path/to/679ca856/zetesis

ordinary catalog-1 "$catalog_solver" indexed
ordinary workspaces-1 "$workspace_solver" indexed
ordinary prepared-1 "$prepared_solver" indexed
ordinary prepared-2 "$prepared_solver" indexed
ordinary workspaces-2 "$workspace_solver" indexed
ordinary catalog-2 "$catalog_solver" indexed
```

Replace those paths with the corresponding frozen executables. Run each call
separately and inspect its exit and report before continuing.

The protocol is CPU/eager/Indexed, automatic membership selection and one closure
and completion worker. Five-second child and thirty-second campaign limits,
4 MiB captures, 128 MiB cumulative capture and 512 MiB reports remain fixed.
Human native output and clingo JSON are inside fresh-process timers; comparison
is outside. Separate waited-child RSS excludes the helper, may include waited
descendants, and is neither simultaneous process-tree RSS nor device memory.
No cold-cache condition, confidence interval or whole-corpus result is claimed.

Latest task-allocation block medians are 89.481/92.402 ms, below both earlier
medians of 93.911/95.469 ms. Their midpoint decreases 3.96%, but each block has
only three timed samples. Other apparent gains are less consistent. The nominal
11.21% queens 04 decrease depends on the late earlier-source median rising from
6.230 to 7.773 ms; latest medians remain about 6.216 ms. Shortest path's nominal
9.50% decrease combines latest medians of 6.346 and 7.873 ms against earlier
medians near 7.856 ms. SEND's earlier blocks span 33.315–36.168 ms. Queens 01/02
instead increase 0.13%/0.76% between source midpoints. These percentages compare
the midpoint of two block medians, not a pooled six-sample median.

Ordinary RSS is broadly unchanged: task allocation moves from 16.000/15.984 MiB
to 15.844/15.844 MiB, while SEND stays near 24.18 MiB. Latest native RSS means
are slightly below the earlier atom-catalog means and above the intermediate
workspace means in all nine cases. In the latest blocks, task allocation and
queens 02 are faster than matched clingo; the other case midpoints favor clingo,
with queens 04 nearly equal. The separate times below retain their full ranges.

### Earlier atom catalog to latest prepared grounding

Native wall time, ms: median [minimum, maximum] of three timed samples per block.

| Case | ca10a5e7-1 | 679ca856-1 | 679ca856-2 | ca10a5e7-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 13.761 [13.747, 13.785] | 13.779 [13.736, 13.810] | 13.788 [13.768, 13.816] | 13.770 [13.742, 13.789] |
| Queens 2 | 87.685 [87.536, 87.775] | 89.100 [87.678, 90.669] | 90.666 [89.004, 90.697] | 90.721 [90.697, 92.081] |
| Queens 3 | 15.375 [13.720, 15.401] | 13.818 [13.758, 13.822] | 15.254 [13.764, 15.400] | 15.319 [15.277, 15.356] |
| Queens 4 | 6.230 [6.218, 7.811] | 6.216 [6.194, 6.225] | 6.218 [6.202, 6.247] | 7.773 [6.195, 7.789] |
| Queens 5 | 9.243 [9.202, 9.274] | 9.211 [7.683, 9.212] | 9.195 [9.184, 9.260] | 9.256 [9.226, 9.269] |
| Queens 6 | 9.218 [9.212, 9.261] | 9.212 [9.196, 9.230] | 9.218 [9.202, 9.262] | 9.225 [9.223, 9.244] |
| SEND | 33.315 [33.287, 34.961] | 33.281 [33.261, 34.881] | 34.838 [34.791, 34.908] | 36.168 [34.828, 37.819] |
| Task allocation | 93.911 [93.893, 94.068] | 89.481 [89.301, 92.296] | 92.402 [91.996, 92.456] | 95.469 [95.447, 95.494] |
| Shortest path | 7.851 [7.815, 7.855] | 6.346 [6.324, 7.911] | 7.873 [6.306, 7.874] | 7.860 [7.852, 7.862] |

Matched clingo wall time, ms, with the same three-sample notation.

| Case | ca10a5e7-1 | 679ca856-1 | 679ca856-2 | ca10a5e7-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 6.214 [6.174, 6.230] | 6.187 [6.159, 6.192] | 6.180 [6.030, 6.198] | 6.213 [6.178, 6.236] |
| Queens 2 | 117.585 [117.431, 117.605] | 117.593 [116.139, 117.730] | 119.146 [119.067, 120.669] | 122.398 [122.092, 123.633] |
| Queens 3 | 6.209 [6.190, 6.297] | 6.192 [6.176, 6.218] | 6.194 [6.172, 6.293] | 6.208 [6.202, 6.238] |
| Queens 4 | 6.174 [6.155, 6.200] | 6.169 [6.154, 6.200] | 6.175 [6.170, 6.175] | 6.180 [6.174, 6.200] |
| Queens 5 | 6.174 [6.148, 6.174] | 6.166 [6.145, 6.178] | 6.156 [6.147, 6.177] | 6.197 [6.173, 6.197] |
| Queens 6 | 6.161 [6.157, 6.200] | 6.151 [6.143, 6.172] | 6.162 [6.154, 6.175] | 6.174 [6.161, 6.188] |
| SEND | 12.284 [12.193, 12.292] | 12.274 [12.184, 12.291] | 12.263 [12.185, 12.333] | 12.305 [12.159, 12.327] |
| Task allocation | 185.523 [176.421, 186.998] | 185.536 [161.424, 190.019] | 185.561 [173.031, 188.505] | 186.978 [156.821, 189.726] |
| Shortest path | 6.231 [6.195, 6.272] | 6.231 [6.189, 6.286] | 6.218 [6.187, 6.314] | 6.277 [6.236, 6.277] |

Separate child RSS, MiB. Each cell is **native / clingo**, one fresh-helper observation of each per block.

| Case | ca10a5e7-1 | 679ca856-1 | 679ca856-2 | ca10a5e7-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 12.047 / 5.422 | 11.969 / 5.422 | 11.875 / 5.422 | 12.109 / 5.422 |
| Queens 2 | 12.609 / 8.156 | 12.516 / 7.969 | 12.516 / 7.969 | 12.625 / 8.844 |
| Queens 3 | 12.219 / 5.438 | 12.016 / 5.438 | 12.094 / 5.438 | 12.062 / 5.438 |
| Queens 4 | 12.109 / 5.469 | 11.969 / 5.469 | 12.078 / 5.469 | 12.219 / 5.469 |
| Queens 5 | 12.719 / 5.531 | 12.500 / 5.531 | 12.516 / 5.531 | 12.562 / 5.531 |
| Queens 6 | 12.797 / 5.562 | 12.656 / 5.562 | 12.719 / 5.562 | 12.859 / 5.562 |
| SEND | 24.188 / 8.281 | 24.203 / 8.281 | 24.156 / 8.281 | 24.188 / 8.281 |
| Task allocation | 16.000 / 21.438 | 15.844 / 19.391 | 15.844 / 21.438 | 15.984 / 21.438 |
| Shortest path | 13.234 / 5.641 | 13.156 / 5.641 | 13.156 / 5.641 | 13.266 / 5.641 |

### Intermediate workspaces to latest prepared grounding

The following view reuses the same two latest blocks. It is useful for locating
changes after workspace reuse, not another independent replication.

Native wall time, ms: median [minimum, maximum] of three timed samples per block.

| Case | f56a5a24-1 | 679ca856-1 | 679ca856-2 | f56a5a24-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 13.778 [13.726, 13.822] | 13.779 [13.736, 13.810] | 13.788 [13.768, 13.816] | 13.766 [13.761, 15.279] |
| Queens 2 | 89.165 [87.510, 89.239] | 89.100 [87.678, 90.669] | 90.666 [89.004, 90.697] | 90.655 [90.540, 90.682] |
| Queens 3 | 15.337 [13.747, 15.392] | 13.818 [13.758, 13.822] | 15.254 [13.764, 15.400] | 15.326 [15.264, 15.357] |
| Queens 4 | 6.247 [6.199, 7.777] | 6.216 [6.194, 6.225] | 6.218 [6.202, 6.247] | 7.731 [6.206, 7.751] |
| Queens 5 | 9.238 [9.213, 9.249] | 9.211 [7.683, 9.212] | 9.195 [9.184, 9.260] | 9.214 [9.193, 9.249] |
| Queens 6 | 9.215 [9.209, 9.280] | 9.212 [9.196, 9.230] | 9.218 [9.202, 9.262] | 9.223 [9.206, 9.265] |
| SEND | 34.905 [34.779, 36.366] | 33.281 [33.261, 34.881] | 34.838 [34.791, 34.908] | 34.820 [34.789, 34.901] |
| Task allocation | 92.361 [92.220, 92.395] | 89.481 [89.301, 92.296] | 92.402 [91.996, 92.456] | 95.314 [93.885, 97.401] |
| Shortest path | 7.861 [6.343, 7.864] | 6.346 [6.324, 7.911] | 7.873 [6.306, 7.874] | 7.878 [7.844, 7.905] |

Matched clingo wall time, ms, with the same three-sample notation.

| Case | f56a5a24-1 | 679ca856-1 | 679ca856-2 | f56a5a24-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 6.215 [6.175, 6.219] | 6.187 [6.159, 6.192] | 6.180 [6.030, 6.198] | 6.234 [6.181, 6.279] |
| Queens 2 | 119.186 [117.574, 120.662] | 117.593 [116.139, 117.730] | 119.146 [119.067, 120.669] | 122.091 [120.611, 122.218] |
| Queens 3 | 6.208 [6.189, 6.260] | 6.192 [6.176, 6.218] | 6.194 [6.172, 6.293] | 6.179 [6.179, 6.238] |
| Queens 4 | 6.174 [6.169, 6.179] | 6.169 [6.154, 6.200] | 6.175 [6.170, 6.175] | 6.167 [6.166, 6.172] |
| Queens 5 | 6.178 [6.173, 6.198] | 6.166 [6.145, 6.178] | 6.156 [6.147, 6.177] | 6.161 [6.148, 6.166] |
| Queens 6 | 6.156 [6.148, 6.185] | 6.151 [6.143, 6.172] | 6.162 [6.154, 6.175] | 6.158 [6.142, 6.185] |
| SEND | 12.284 [12.209, 12.307] | 12.274 [12.184, 12.291] | 12.263 [12.185, 12.333] | 12.261 [12.175, 12.309] |
| Task allocation | 186.939 [172.008, 188.107] | 185.536 [161.424, 190.019] | 185.561 [173.031, 188.505] | 176.524 [174.952, 186.997] |
| Shortest path | 6.193 [6.180, 6.279] | 6.231 [6.189, 6.286] | 6.218 [6.187, 6.314] | 6.261 [6.240, 6.295] |

Separate child RSS, MiB. Each cell is **native / clingo**, one fresh-helper observation of each per block.

| Case | f56a5a24-1 | 679ca856-1 | 679ca856-2 | f56a5a24-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 11.734 / 5.422 | 11.969 / 5.422 | 11.875 / 5.422 | 11.750 / 5.422 |
| Queens 2 | 12.359 / 8.812 | 12.516 / 7.969 | 12.516 / 7.969 | 12.391 / 8.234 |
| Queens 3 | 11.906 / 5.438 | 12.016 / 5.438 | 12.094 / 5.438 | 11.938 / 5.438 |
| Queens 4 | 11.734 / 5.469 | 11.969 / 5.469 | 12.078 / 5.469 | 11.828 / 5.469 |
| Queens 5 | 12.281 / 5.531 | 12.500 / 5.531 | 12.516 / 5.531 | 12.484 / 5.531 |
| Queens 6 | 12.625 / 5.562 | 12.656 / 5.562 | 12.719 / 5.562 | 12.594 / 5.562 |
| SEND | 24.062 / 8.281 | 24.203 / 8.281 | 24.156 / 8.281 | 24.078 / 8.281 |
| Task allocation | 15.859 / 19.406 | 15.844 / 19.391 | 15.844 / 21.438 | 15.656 / 21.438 |
| Shortest path | 13.031 / 5.641 | 13.156 / 5.641 | 13.156 / 5.641 | 12.859 / 5.641 |

### Scalar and shared lazy CPU observations

Separate `zetesis-bench lazy` observations use the same six-source order over
sparse/dense fixtures, widths 4/8 and candidate batches 1/32/128. Each block has
one initial, two warmup and twelve timed samples per case/route: **4,320 samples,
3,456 timed**, all completed. Scalar, four-worker Rayon, PortableUnion and
PortableWorlds compare full ordered closures, constraint verdicts and seed
mismatches to an independent scalar reference after timing. The JSONL records
retain receipts, not the full closures for an independent cross-version
reconstruction. Repeated seeds are candidate occurrences, not answer-set counts.
Device fields are absent; the shared portable routes evaluate consequences
serially, and the Rayon pool serves independent checks. No RSS was sampled.

Use each source's own frozen bench in the six-source order:

```sh
"$bench" lazy --backend cpu --widths 4,8 --batches 1,32,128 \
  --families sparse,dense --workers 4 --warmups 2 --repetitions 12 \
  --chunk-rules 256 --max-work 100000000
```

The bench SHA-256 values, in earlier/intermediate/latest source order, are
`ecb21a65cf618bf56f75dc62a165d3b83d8bf61f7856eeec4659805e00cbfad8`,
`2bc44c92f16999ef29950cfa26c727da0f9479e15c63a1414cba353a1843931d`, and
`59d568f0e15b99541d5e65bc9fc9da631e98e87c8424a42e1f0307cb6d066678`.
Scalar creates preparation and workspace per seed inside timing; warmed Rayon
retains exact-Program preparation and empty capacity across calls. It never
retains candidate truth. Shared source traversal keeps its full schedule.

Compared with intermediate workspaces, latest inclusive work and enabled
bindings decrease in every scalar and Rayon fixture. Dense width-8/128 Scalar
falls from 11,925,504 to 9,240,064 work units and 143,488 to 69,760 bindings, with
identical closure and round counts. Medians of the 24 timed samples per source
are 55.951 ms earlier, 65.376 ms intermediate and 55.932 ms latest: recovery to
roughly the earlier time, not a large gain over it. Latest warmed Rayon reports
one preparation build per exact program and reuse of every active slot; its
pool receipts are cumulative snapshots, not additional per-check work.

Small regressions remain. Sparse width-4/32 Scalar rises from 133.209 to
148.521 microseconds against the earlier source (+11.49%, or 15.312 microseconds);
both latest block medians exceed both earlier medians, though sample ranges
overlap. Dense width-4/128 Scalar adds 168.813 microseconds (+2.57%). The largest
Rayon increase against the earlier source is 5.01% (3.271 microseconds, dense
width-4/1). Sparse width-4/32 Rayon is 2.83% above the earlier source and 17.39%
above intermediate workspaces, with opposite directions in its repeated blocks.
Shared Union/Worlds timings move both ways while intermediate/latest logical
receipts agree. Dense width-8/128 shared times increase 0.65%/1.76% against the
earlier source. Fewer bindings do not imply a shared-source speedup.

Latest scalar named peak capacity is 320 bytes above the intermediate value
in each fixture; this is not RSS or an isolated layout attribution. Earlier
missing work/cache/probe fields remain unavailable, not zero. These fixtures
insert predicate rows in canonical waves, favoring append-at-end in the earlier
sorted representation; they do not measure the new index's avoided-shift
benefit. The [prepared-query contract](../rust/parallel.md) explains reuse,
reset and retained-owner admission independently of these timings.

### Optional domain admission observations

An Indexed off/on/on/off comparison of `grounding::profile` exercises the
[optional domain API](../rust/source.md#optional-domains-during-final-instantiation)
at `679ca856`. It uses `a(1..8)`, `b(1..8)` and a `c` square over 5..12, followed
by `r(X,Y) :- a(X), b(Y), c(X,Y)`, with facts explicitly enumerated to stay in
the flat admitted profile. All 36 sampled admissions and four independent
Indexed/domain-disabled references preserve the exact subject and one complete
96-atom model. The three observer modes have different overhead and are kept
separate. Each cell is a three-sample admission median in microseconds:

| Observer mode | off-1 | on-1 | on-2 | off-2 |
| --- | ---: | ---: | ---: | ---: |
| Unobserved | 1424.750 | 1182.083 | 1132.000 | 1156.084 |
| Boundary | 1584.542 | 1176.042 | 1131.583 | 1104.250 |
| Detailed | 1514.583 | 1178.666 | 1190.208 | 1134.291 |

Detailed final-rule probes fall 73→21 and offered rows 200→168, with 84 rejected
rows and the same sixteen bindings. Domain preparation adds 2,090 counted units;
peak named support capacity rises 18,072→18,656 bytes and excludes analyzer
standard collections. Possible-support completion is unchanged. Admission timing
includes normalization and requested analysis; source loading/parsing, reference
checking and publication are outside it. The late enabled/disabled comparison
ranges from −2.08% to +4.93% across modes, and the early disabled block is much
slower. This is work reduction with setup cost and drift, not a stable speedup.
Domains remain disabled in ordinary solving. A completed analysis-attempt phase
does not certify FixedPoint; these profiler records do not serialize the typed
analysis status. No RSS, clingo or GPU observation is implied.

### Larger queens screens

The same three natives were screened with all six N=10 queens encodings and
predeclared N=12 encodings 03/04. Each has qualification plus one timed sample,
no warmup, CPU/eager/Indexed, four closure/completion workers, clingo one worker,
batch size 64, ten-second children and ninety-second campaigns. JSON/statistics
output is included. The finite decoder admits 16,384 witnesses and 1,048,576
native full atoms; captures remain bounded. This is a completion screen, not a
precision timing experiment, and it has no RSS phase.

At N=10, five encodings qualify with 724 selected answers on each source.
Encoding 02 reaches clingo's ten-second timeout; native completes 724 but remains
reference-unavailable. Each report retains twenty passing positions, that timeout,
the native reference-unavailable position and two blocked timed positions, and
exits 1. No timeout is replaced with a first-answer observation. Both N=12
encodings qualify with 14,200 selected answers on all three sources. Latest native
internal elapsed times are 2.281/1.818 seconds versus earlier 2.273/1.816 seconds,
but there is only one timed sample per encoding/source. Full native records are
retained; selected-output agreement with clingo does not reconstruct its hidden
interpretations. These limits and outcomes do not establish a general scaling law.

## Earlier atom catalog CPU comparison

This comparison is
[`1e5b78ce`](https://github.com/GregoryGelfond/zetesis/tree/1e5b78ce913ab3aeece6ed496f69ca8176f0644d)
against
[`ca10a5e7`](https://github.com/GregoryGelfond/zetesis/tree/ca10a5e7ec84e13fbcc4a23bd0de8b0232c53fe1),
recorded on arm64 macOS 26.6.2 on 14 September 2026. The ordinary acquisition
ran from 03:42:07 to 03:43:27 UTC. Both versions use the canonical package build
recipe without an LTO override, and the same fixed performance runner and
clingo 5.8.2. Source and binary identities are retained together; rebuilding a
named source need not reproduce the same executable bytes.

| Artifact | SHA-256 |
| --- | --- |
| Prior canonical zetesis | `0758210934e1a80c350808fc936414c359974a2327119e0af3b3ab5e5f0f79f8` |
| Atom-catalog canonical zetesis | `fd19a078e99c75c1bbaf30e437f5da02aa621fd595554b079bb3bc0b078dae0e` |
| Fixed zetesis-perf | `7c05900d8f3e5c0d05224742c5713bfbe37b64198cff8ef986c1f141356ba9f5` |
| clingo | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |

The [observation data](observations/release-1e5b78ce-ca10a5e7.json) and
[provenance](observations/release-1e5b78ce-ca10a5e7-provenance.json) retain all
468 ordered positions, exact integer timing/RSS samples, source and executable
identities, and the original report hashes. This is a derived receipt view;
output streams and machine-local paths are omitted. It preserves the recorded
selected-display/cost comparisons, not an independent comparison of hidden
full interpretations. The [observation guide](observations/README.md) states
that boundary and reproduces the three tables below without running a solver:

```sh
cargo run --locked -p zetesis-validation --example release_observations -- --dataset atom-catalog
cargo run --locked -p zetesis-validation --example release_observations -- --dataset atom-catalog --check
```

The four blocks run prior/current/current/prior. Each contains all nine original
workloads: six N=8 queens encodings, SEND, task allocation variant 04/scenario 05,
and shortest path variant 01/scenario 06. Each producer/case/block has
qualification, one warmup, three timed runs and one separate child-RSS run;
native diagnostics are separate. All positions pass, with no capture faults or
unresolved children. Recorded selected families contain 92 models for queens,
one for SEND, 1,176 optimum ties at cost `[5]` for task allocation, and one at
`[4,4]` for shortest path.

Both natives use CPU/eager/Indexed with automatic oracle selection and one
requested closure and completion worker. All 36 diagnostics identify actual
`tight-support`, indexed probes and zero Table probes. Native timed output is
human-readable without statistics; clingo emits JSON. Timers include process
startup, capture and reaping, with output comparison outside the interval.
The limits are five seconds per child, 30 seconds per campaign, 4 MiB per
capture, 128 MiB total capture and 512 MiB per report. No cold-cache condition or
confidence interval is claimed. Timed samples do not measure Table joins,
shared lazy grounding, parallel residual completion or GPU execution.

Task allocation uses 16.016 MiB in both current RSS samples versus
19.641/19.531 MiB previously: an 18.23% decrease between the two sample means.
Its mean block-median wall time decreases 1.60%, with overlapping ranges;
matched clingo decreases 0.60%. SEND instead increases 7.91%, and every current
timed value exceeds every prior value in this acquisition. Matched clingo is
nearly unchanged. These percentages compare the mean of the two current block
medians with the mean of the two prior block medians, not a pooled population.

The apparent 7.25% shortest-path decrease depends on the final prior block's
9.193 ms median; the other three are about 7.92–7.94 ms. Several queens and
reference blocks also drift. Separate diagnostics locate higher SEND and task
grounding times and lower task solving times, but do not isolate a cause.
Neither these stage observations nor unchanged source-event counts enumerate
every comparison, allocation or copy. All block ranges remain visible below.

RSS measures a separate solver child's peak, excluding the resource helper;
waited descendants can contribute. It is neither simultaneous process-tree
RSS nor a measurement of one index's capacity. The two samples per version
support the stated task observation, not a general memory-scaling claim.
MiB means 1,048,576 bytes.

Native wall time, ms: median [minimum, maximum] of three timed samples per block.

| Case | prior-1 | current-1 | current-2 | prior-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 14.102 [14.054, 15.545] | 15.459 [15.346, 15.464] | 15.367 [15.356, 16.644] | 15.236 [15.144, 15.323] |
| Queens 2 | 89.576 [89.556, 89.769] | 90.657 [90.404, 92.048] | 91.815 [91.779, 92.996] | 91.885 [91.517, 91.980] |
| Queens 3 | 15.430 [15.387, 15.480] | 16.722 [16.604, 16.810] | 16.687 [16.616, 16.756] | 16.667 [15.470, 16.763] |
| Queens 4 | 7.812 [7.799, 9.183] | 7.899 [7.834, 9.096] | 9.148 [7.836, 9.165] | 9.114 [9.080, 9.214] |
| Queens 5 | 10.350 [8.980, 10.387] | 10.428 [10.053, 11.668] | 10.301 [10.290, 10.365] | 10.402 [10.298, 11.711] |
| Queens 6 | 10.322 [10.276, 11.698] | 11.432 [10.404, 11.671] | 11.658 [11.630, 11.670] | 11.622 [10.325, 11.627] |
| SEND | 33.847 [32.902, 34.132] | 36.700 [36.449, 36.800] | 36.821 [36.765, 36.851] | 34.285 [34.201, 35.516] |
| Task allocation | 94.253 [93.360, 94.554] | 93.412 [93.307, 93.548] | 93.406 [93.212, 93.666] | 95.599 [93.591, 95.827] |
| Shortest path | 7.921 [7.835, 7.951] | 7.935 [7.866, 9.202] | 7.938 [7.938, 9.165] | 9.193 [7.881, 9.229] |

Matched clingo wall time, ms, with the same three-sample notation.

| Case | prior-1 | current-1 | current-2 | prior-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 6.526 [6.493, 6.532] | 6.513 [5.244, 6.519] | 6.562 [6.511, 6.577] | 5.221 [5.180, 6.677] |
| Queens 2 | 121.563 [120.676, 121.624] | 121.586 [121.522, 123.060] | 123.265 [122.855, 123.400] | 123.990 [123.318, 124.397] |
| Queens 3 | 6.590 [6.586, 6.596] | 6.590 [6.584, 6.604] | 6.549 [6.507, 6.579] | 6.517 [6.459, 6.579] |
| Queens 4 | 6.479 [5.180, 6.532] | 6.520 [6.456, 6.520] | 6.510 [6.477, 6.513] | 6.442 [5.179, 6.461] |
| Queens 5 | 6.521 [6.445, 6.529] | 6.506 [6.495, 6.518] | 6.501 [6.428, 6.506] | 6.506 [5.159, 6.560] |
| Queens 6 | 5.247 [5.185, 6.442] | 6.494 [6.444, 6.499] | 6.480 [6.282, 6.521] | 5.213 [5.187, 6.633] |
| SEND | 11.589 [11.403, 11.605] | 11.600 [11.448, 11.613] | 11.563 [11.448, 12.680] | 11.556 [11.494, 12.875] |
| Task allocation | 186.945 [182.191, 188.547] | 187.135 [175.736, 188.596] | 183.200 [181.927, 186.255] | 185.619 [181.670, 190.859] |
| Shortest path | 5.298 [5.209, 6.508] | 5.198 [5.195, 5.253] | 6.508 [5.183, 6.566] | 6.358 [5.262, 6.569] |

Separate child RSS, MiB. Each cell is **native / clingo**, one fresh-helper observation of each per block.

| Case | prior-1 | current-1 | current-2 | prior-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 12.109 / 5.656 | 12.109 / 5.656 | 12.156 / 5.656 | 12.125 / 5.656 |
| Queens 2 | 12.672 / 11.000 | 12.781 / 10.000 | 12.812 / 10.016 | 12.703 / 9.688 |
| Queens 3 | 12.234 / 5.656 | 12.219 / 5.656 | 12.172 / 5.672 | 12.172 / 5.656 |
| Queens 4 | 12.219 / 5.703 | 12.250 / 5.703 | 12.234 / 5.922 | 12.281 / 5.703 |
| Queens 5 | 12.625 / 5.766 | 12.719 / 5.766 | 12.594 / 5.766 | 12.688 / 5.766 |
| Queens 6 | 12.703 / 5.812 | 12.844 / 5.812 | 12.922 / 5.812 | 12.719 / 5.812 |
| SEND | 24.188 / 8.500 | 24.250 / 8.500 | 24.297 / 8.500 | 24.172 / 8.500 |
| Task allocation | 19.641 / 22.750 | 16.016 / 22.375 | 16.016 / 22.516 | 19.531 / 21.688 |
| Shortest path | 13.156 / 6.062 | 13.234 / 5.891 | 13.344 / 5.891 | 13.156 / 5.891 |

### Shared lazy CPU checks

A separate four-block comparison of the same two sources directly exercises
shared lazy checking through the maintained `zetesis-bench lazy` command. The
prior bench hash is `e720983506475d7059198ebf0856e8617eff2e8888a838fd6a33954e42786a9a`;
the ca10 bench hash is `ecb21a65cf618bf56f75dc62a165d3b83d8bf61f7856eeec4659805e00cbfad8`.
The acquisition ran from 03:46:21 to 03:46:31 UTC, separately from ordinary
process timings. It covers sparse/dense fixtures, widths 4/8 and batches
1/32/128, with Scalar, Rayon, PortableUnion and PortableWorlds routes. Every
block completes all 720 positions: one initial, two warmup and twelve timed
checks per case/route, rotating route order. All 2,880 samples pass the
producer's comparison of complete ordered closures, constraint verdicts and
seed mismatches against an independent scalar reference after the timer.
The records retain counts and progress, not full closure payloads for another
cross-version reconstruction. Repeated seeds are occurrence checks, not unique
answer sets or an outer enumeration/clingo comparison.

PortableUnion and PortableWorlds actually call the shared source/catalog
consumer with serial portable consequence evaluation. The four-worker Rayon
pool runs the independent route; its presence does not mean shared grounding
runs in parallel. Device fields are null. Batch construction and finalization
are inside the shared timer; fixture/reference construction, pool setup,
comparison and JSON publication are outside it. No RSS was sampled here.

Most shared cases slow down: 11 of 12 Union cases and 10 of 12 Worlds cases.
Changes below compare the two current block medians with the two prior block
medians. Each block median uses twelve timed observations; this is descriptive,
not a confidence interval or an average across unrelated fixtures.

| Family / width / worlds | Union change | Worlds change |
| --- | ---: | ---: |
| sparse / 4 / 1 | +14.45% | +12.91% |
| sparse / 4 / 32 | +12.72% | +5.78% |
| sparse / 4 / 128 | +9.82% | +4.78% |
| sparse / 8 / 1 | +4.85% | +11.67% |
| sparse / 8 / 32 | +11.62% | +4.59% |
| sparse / 8 / 128 | +12.91% | +2.32% |
| dense / 4 / 1 | +4.68% | +6.02% |
| dense / 4 / 32 | +2.08% | -2.04% |
| dense / 4 / 128 | -2.34% | -1.62% |
| dense / 8 / 1 | +3.85% | +12.97% |
| dense / 8 / 32 | +3.04% | +2.08% |
| dense / 8 / 128 | +2.49% | +0.24% |

For example, sparse width-8/128 Union medians rise from 1.0335/1.0891 ms to
1.2000/1.1967 ms. Dense width-8/1 Worlds rises from 0.7848/0.7751 ms to
0.8836/0.8786 ms. The small improvements occur in some dense width-4 cases;
there is no general shared-source speedup. The prior/current change includes
more than the interner, so these timings do not assign a cause to one operation.
In particular, formula grounding already owned each emitted atom once before
its index changed; the removal of duplicated payload applies to the batched
lazy owner. [Ownership and cost contracts](../architecture/ownership.md) describe
these different populations and the remaining per-call preparation.

Use each source's matching bench in prior/current/current/prior order, retaining
all four outputs, with this same command suffix:

```sh
zetesis-bench lazy --backend cpu --widths 4,8 --batches 1,32,128 \
  --families sparse,dense --workers 4 --warmups 2 --repetitions 12 \
  --chunk-rules 256 --max-work 100000000
```

The recorded shared-source limits also include 1,024 candidates, 4,096 atoms,
4,097 rounds, 16,384 chunk words, 1 MiB per copied instance and 128 MiB host
storage. The named host envelope is not RSS. Work counters count documented
operations, not machine instructions. Fixed interner probes provide separate
preparation, lookup, append and capacity observations; they cannot substitute
for the whole-solve or shared-consumer results above.

### Explicit CPU profile matrix

A separate N=8 queens matrix requests eager and lazy profiles, four closure and
completion workers, and batch size 64. Across four prior/current/current/prior
blocks, all 360 positions are accounted: 240 pass, 24 return typed
`unsupported_oracle`, and 96 successors are blocked by those refusals. Every
report therefore exits 1. Both revisions refuse the same CPU/lazy requests
because the selected countermodel route requires eager or automatic grounding.
There are no capture faults or unresolved children. A refusal is an incomplete
outcome, not UNSAT or a successful zero-time sample.

All 120 successful native observations actually use CPU/eager/Indexed and
`tight_support`, exhaust their search and check, verify and publish 92 models.
The paired references have 92 selected displays. Completion records zero
residuals and zero effective completion workers, despite four requested workers;
there is no actual GPU activity. Counts and recorded passing comparisons are
not a new independent hidden-family reconstruction. These JSON/statistics
process timings form a different population from ordinary human-output times;
no RSS or successful lazy timing is available from this matrix. It neither
qualifies shared lazy performance nor establishes a parallel or GPU gain.

### Reproducing the ordinary comparison

The [ordinary comparison function below](#reproduce-the-measurements) uses the
same nine cases and limits. For this atom-catalog comparison, select the named
1e5b78ce and ca10a5e7 binaries and the runner hash above, and pass `indexed` for
both versions. After defining that function, run the following calls separately,
retaining and inspecting every report before continuing:

```sh
ordinary catalog-prior-1 "$previous_solver" indexed
ordinary catalog-current-1 "$current_solver" indexed
ordinary catalog-current-2 "$current_solver" indexed
ordinary catalog-prior-2 "$previous_solver" indexed
```

The new public dataset contains only the ordinary 468-position comparison.
Shared-lazy and matrix summaries retain their distinct methods and completion
limits. These CPU acquisitions do not qualify Metal or imply a performance
claim for later sources.

## Earlier Table-grounding comparisons

The sections below retain their original 6bebb980 → 1e5b78ce release, Table,
LTO and Metal evidence. Their previous/current labels refer to those sources,
not to the atom-catalog comparison above. The historical ordinary artifacts and
their three published tables are unchanged.

The ordinary grounder now offers `--formula-joins table` for eligible positive
joins over completed eager support. It reuses borrowed relation indices and
consumes original row positions through the existing matcher. Indexed joins
remain the default. These measurements establish actual table use and some
reductions in visited rows; they do not establish a broad application speedup.

The release profile also remains unchanged after a separate normal/thin/fat
LTO comparison. Smaller binaries and lower sampled RSS did not produce better
times on all important inputs. Matched eager CPU/Metal measurements also show
no broad Table speedup; CPU is faster on these inputs. That comparison spans
two acquisition windows, kept explicit below. Earlier Metal and standalone table
results retain their original source scope.

## Versions and measurement boundaries

Measurements ran on an Apple M4 Pro with macOS 26.6.2 on 12 September 2026,
using Rust 1.97.1 and clingo 5.8.2. The previous implementation is
[`6bebb980`](https://github.com/GregoryGelfond/zetesis/tree/6bebb980f9c102dbb7f943076d7cde92374841ce);
the new implementation is
[`1e5b78ce`](https://github.com/GregoryGelfond/zetesis/tree/1e5b78ce913ab3aeece6ed496f69ca8176f0644d).
The [preceding complete comparison](https://github.com/GregoryGelfond/zetesis/blob/993a7bbb625ae62ea4ff0ef4510c3d1a8be514ac/docs/book/reference/performance.md)
retains the earlier CPU, Metal and primitive tables and their reproduction commands.

The canonical release comparison uses the installer's package selection.
The LTO experiment uses a separate fixed single-binary build recipe; its normal
binary is not the canonical installer binary. The same preserved `zetesis-perf`
acquires all ordinary comparisons. Native output is human-readable without
statistics during timing; clingo emits JSON. Separate diagnostic invocations
retain actual execution and grounding counters.
These ordinary comparisons use the CPU backend, eager grounding and automatic
oracle selection, with one requested closure worker and one completion worker.

| Artifact | SHA-256 |
| --- | --- |
| Previous canonical zetesis | `35b96c837dd5027853c735044e092f9054d63fc516940ec83d10627ecc2cf5d8` |
| Current canonical zetesis | `0758210934e1a80c350808fc936414c359974a2327119e0af3b3ab5e5f0f79f8` |
| Fixed zetesis-perf | `5eb1c1d1de5e9076c2d37150dea8d7a869d34468cc88a8fc3627cf1002595e53` |
| Current grounding-profile zetesis-bench | `33cbe86ea2913d67685ed1bafd0bb97629f86d9f432aff14c269f62c84b92cd1` |
| clingo 5.8.2 | `31e738a632a8053eef1604c150f4d6418ff1dd8a9a3d5a8c1d594d6d30b67015` |
| Corpus manifest | `b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958` |

All six queens encodings use N=8 unless stated otherwise. SEND uses the
[standalone encoding](../../../examples/kr-domains/standalone/send-money/send-money.lp),
task allocation uses [variant04/scenario05](../../../examples/kr-domains/scenarios/task-allocation/variant-04/05-larger-mix.lp),
and shortest path uses [variant01/scenario06](../../../examples/kr-domains/scenarios/shortest-path/variant-01/06-layered-dag.lp).
Their [manifest](../../../examples/kr-domains/manifest.json) records companion
sources and their hashes. Unoptimized programs enumerate all answers; optimized
programs publish every optimum tie. This is neither time to the first answer
nor a comparison of unpublished nonoptimal interpretations.

Each ordinary A/B/B/A comparison has four blocks. A block contains qualification,
one warmup pair, three timed pairs, one separate memory pair and one native
statistics run for each of nine cases. All 16 blocks account for 1,872 passing
positions with no capture faults or unresolved children. Startup and output are
included; no cold-cache condition or confidence interval is claimed. Block
medians remain visible because some populations drift substantially.

Memory samples report solver-child peak RSS from a fresh helper; the helper is
excluded and waited descendants may contribute to child usage. Two samples per
side of a four-block comparison are a limited observation, not a process-tree
memory census or device-memory measure. MiB means 1,048,576 bytes.

## Canonical release comparison

The [curated observation data](observations/README.md) publishes all 468 ordered
receipts for this historical nine-workload, four-block comparison, with exact
timing/RSS samples and source, executable and original-report hashes. The
maintained `release_observations` Rust example reproduces the three tables below.
This derived view retains the original selected-display qualification scope;
it is not a raw-output archive, a new parity check or qualification of the
current implementation. The other Metal and LTO populations are outside it.

These tables now round directly from integer nanoseconds and bytes to three
decimal places, with ties to even. This corrects a few final digits in the
earlier presentation; the raw observations and comparison scope are unchanged.

The prior binary uses its default joins; the current binary explicitly requests
Indexed. Current blocks largely resemble the closing prior block, while many
first prior timings are lower. Matched clingo observations also drift (for
example task allocation 170.862 to 196.935 ms between prior blocks, with current
blocks 200.485 and 197.766 ms). Do not attribute that whole shift to the new code.
SEND still has higher current medians than either prior block, which remains a
visible result rather than evidence of universal neutrality. These measurements
show no broad speed or RSS improvement for this release boundary.

Native wall time, ms: median [minimum, maximum] of three timed samples per block.

| Case | prior-1 | current-1 | current-2 | prior-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 15.302 [14.036, 15.498] | 16.688 [16.451, 16.693] | 16.674 [16.430, 16.698] | 16.694 [16.528, 16.832] |
| Queens 2 | 90.830 [88.143, 92.014] | 96.748 [96.522, 97.113] | 95.871 [95.738, 97.597] | 96.982 [95.416, 97.047] |
| Queens 3 | 15.397 [15.395, 15.404] | 16.744 [16.721, 16.752] | 16.715 [16.641, 16.741] | 16.752 [16.680, 16.754] |
| Queens 4 | 7.819 [7.779, 9.064] | 9.147 [9.138, 9.230] | 9.147 [9.135, 9.161] | 9.036 [8.976, 9.117] |
| Queens 5 | 10.311 [10.296, 10.414] | 11.487 [11.454, 11.682] | 11.649 [10.337, 11.760] | 11.675 [11.597, 11.713] |
| Queens 6 | 10.321 [10.307, 10.350] | 11.660 [11.594, 11.731] | 11.638 [11.525, 11.640] | 11.688 [11.659, 11.730] |
| SEND | 32.872 [32.855, 32.957] | 36.864 [35.559, 36.877] | 36.846 [36.777, 36.862] | 35.530 [35.528, 35.543] |
| Task allocation | 94.881 [94.600, 96.831] | 99.692 [99.676, 99.701] | 99.762 [99.169, 99.803] | 101.743 [101.006, 102.320] |
| Shortest path | 7.872 [7.810, 7.899] | 9.214 [9.118, 9.220] | 9.144 [9.103, 9.210] | 9.228 [8.958, 9.236] |

Matched clingo wall time, ms, with the same three-sample notation.

| Case | prior-1 | current-1 | current-2 | prior-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 5.228 [5.217, 6.458] | 6.517 [6.514, 6.576] | 6.546 [6.519, 6.585] | 6.527 [6.506, 6.680] |
| Queens 2 | 120.825 [120.531, 122.078] | 130.939 [130.576, 130.940] | 130.835 [130.720, 130.959] | 130.706 [130.554, 130.716] |
| Queens 3 | 6.555 [6.479, 6.580] | 6.550 [6.509, 6.571] | 6.556 [6.551, 6.565] | 6.566 [6.529, 6.577] |
| Queens 4 | 6.460 [5.154, 6.472] | 6.533 [6.468, 6.659] | 6.503 [6.450, 6.511] | 6.514 [6.497, 6.532] |
| Queens 5 | 6.450 [5.172, 6.458] | 6.492 [6.449, 7.828] | 6.510 [6.485, 6.511] | 6.497 [6.462, 6.556] |
| Queens 6 | 5.176 [5.156, 6.518] | 6.522 [6.507, 6.559] | 6.507 [6.430, 6.507] | 6.511 [6.465, 6.530] |
| SEND | 11.604 [11.502, 11.772] | 12.817 [12.785, 12.851] | 12.765 [12.597, 12.825] | 12.822 [12.724, 12.886] |
| Task allocation | 170.862 [162.112, 178.523] | 200.485 [200.336, 201.329] | 197.766 [175.564, 198.352] | 196.935 [192.598, 200.959] |
| Shortest path | 5.209 [5.209, 6.569] | 6.497 [6.488, 6.570] | 6.557 [6.506, 6.594] | 6.511 [6.494, 6.541] |

Separate child RSS, MiB. Each cell is **native / clingo**, one fresh-helper observation of each per block.

| Case | prior-1 | current-1 | current-2 | prior-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 12.109 / 5.656 | 12.156 / 5.656 | 12.203 / 5.656 | 12.109 / 5.656 |
| Queens 2 | 12.656 / 9.547 | 12.625 / 10.953 | 12.656 / 10.531 | 12.672 / 10.344 |
| Queens 3 | 12.078 / 5.656 | 12.250 / 5.953 | 12.234 / 5.656 | 12.094 / 5.656 |
| Queens 4 | 12.078 / 5.703 | 12.250 / 5.922 | 12.172 / 5.703 | 12.156 / 5.703 |
| Queens 5 | 12.656 / 5.766 | 12.656 / 5.766 | 12.625 / 5.938 | 12.703 / 5.766 |
| Queens 6 | 12.750 / 5.812 | 12.797 / 5.812 | 12.750 / 5.812 | 12.812 / 5.812 |
| SEND | 24.094 / 9.234 | 24.250 / 9.438 | 24.266 / 9.047 | 24.125 / 8.938 |
| Task allocation | 19.453 / 21.156 | 19.719 / 21.422 | 19.688 / 22.500 | 19.484 / 23.250 |
| Shortest path | 13.109 / 5.891 | 13.219 / 6.672 | 13.109 / 6.141 | 12.969 / 5.891 |

## Indexed versus Table in ordinary solving

All four blocks use exactly the same canonical binary. Every native invocation
has its requested strategy flag; no clingo invocation has that flag. Table work
is observed on every one of the nine inputs in both Table diagnostic blocks.
Ordinary wall times show small mixed changes and later-block drift, rather than
a broad application speedup. There is no demonstrated RSS reduction from Table
on this population. Keeping Indexed as the default is consistent with these
results; the Table option supplies an applicable, measured alternative.

Native wall time, ms: median [minimum, maximum] of three timed samples per block.

| Case | indexed-1 | table-1 | table-2 | indexed-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 13.743 [13.743, 15.275] | 13.725 [13.721, 13.738] | 13.730 [13.713, 13.756] | 15.153 [14.061, 15.415] |
| Queens 2 | 87.671 [87.548, 89.096] | 88.934 [87.531, 90.587] | 89.086 [87.526, 89.151] | 90.405 [90.326, 90.724] |
| Queens 3 | 15.340 [13.754, 15.433] | 15.272 [13.713, 15.311] | 15.306 [15.245, 15.409] | 15.397 [15.364, 16.765] |
| Queens 4 | 7.733 [7.729, 7.738] | 7.715 [7.701, 7.718] | 7.742 [7.723, 7.752] | 7.802 [7.765, 9.229] |
| Queens 5 | 9.213 [9.210, 9.217] | 9.196 [9.196, 9.202] | 9.234 [9.205, 10.710] | 10.275 [10.275, 10.317] |
| Queens 6 | 9.199 [9.179, 9.201] | 9.191 [9.178, 9.200] | 10.717 [9.213, 10.767] | 10.293 [10.285, 10.300] |
| SEND | 31.785 [30.259, 31.847] | 31.803 [31.759, 31.825] | 31.778 [31.767, 33.417] | 32.885 [32.871, 32.901] |
| Task allocation | 93.930 [92.529, 94.056] | 93.952 [92.450, 93.956] | 93.969 [93.967, 95.584] | 94.851 [93.350, 96.125] |
| Shortest path | 7.842 [7.779, 7.862] | 7.789 [7.787, 7.867] | 7.862 [7.818, 7.885] | 7.886 [7.856, 9.175] |

Matched clingo wall time, ms, with the same three-sample notation.

| Case | indexed-1 | table-1 | table-2 | indexed-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 6.221 [4.654, 6.228] | 6.187 [6.178, 6.216] | 6.219 [6.193, 6.241] | 5.233 [5.197, 6.280] |
| Queens 2 | 117.564 [116.112, 117.592] | 119.033 [119.003, 119.116] | 119.109 [119.052, 119.173] | 120.612 [120.369, 121.364] |
| Queens 3 | 6.222 [6.186, 6.224] | 6.166 [6.161, 6.224] | 6.209 [6.194, 6.210] | 6.524 [5.201, 6.534] |
| Queens 4 | 6.177 [6.158, 6.186] | 6.156 [6.149, 6.171] | 6.176 [6.151, 6.208] | 5.176 [5.154, 6.501] |
| Queens 5 | 6.162 [6.160, 6.188] | 6.153 [6.144, 6.169] | 6.152 [6.147, 6.168] | 6.302 [5.165, 6.503] |
| Queens 6 | 6.164 [6.148, 6.176] | 6.151 [6.149, 6.152] | 6.170 [6.166, 6.205] | 5.186 [5.175, 6.495] |
| SEND | 10.725 [10.661, 10.769] | 10.745 [10.640, 10.807] | 10.781 [10.681, 12.233] | 11.518 [11.445, 11.579] |
| Task allocation | 177.943 [159.819, 182.149] | 171.897 [162.911, 180.489] | 184.059 [170.342, 185.564] | 179.709 [160.830, 180.513] |
| Shortest path | 6.240 [4.727, 6.273] | 6.240 [4.736, 6.241] | 6.258 [4.745, 6.259] | 5.270 [5.223, 6.563] |

Separate child RSS, MiB. Each cell is **native / clingo**, one fresh-helper observation of each per block.

| Case | indexed-1 | table-1 | table-2 | indexed-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 12.203 / 5.656 | 12.234 / 5.656 | 12.125 / 5.656 | 12.156 / 5.656 |
| Queens 2 | 12.750 / 8.406 | 12.781 / 8.406 | 12.797 / 9.141 | 12.578 / 10.094 |
| Queens 3 | 12.203 / 5.656 | 12.250 / 5.656 | 12.297 / 5.656 | 12.188 / 5.656 |
| Queens 4 | 12.234 / 5.703 | 12.266 / 5.703 | 12.250 / 5.703 | 12.188 / 5.703 |
| Queens 5 | 12.656 / 5.766 | 12.688 / 5.766 | 12.703 / 5.766 | 12.641 / 5.766 |
| Queens 6 | 12.859 / 5.813 | 12.766 / 5.813 | 12.906 / 5.813 | 12.844 / 5.813 |
| SEND | 24.203 / 8.500 | 24.281 / 8.500 | 24.266 / 8.500 | 24.234 / 8.500 |
| Task allocation | 19.641 / 19.766 | 19.703 / 19.719 | 19.719 / 19.750 | 19.688 / 22.703 |
| Shortest path | 13.141 / 5.891 | 13.125 / 5.891 | 13.203 / 5.891 | 13.172 / 5.891 |


## What the table consumer does

The [library API](../rust/finite-tables.md) separates immutable relation ownership,
prepared indices and per-query result masks. A selection can outlive its table
index while borrowing the original relation. Domain restrictions include typed
constants, already-bound source values and repeated-variable aliases. Each
probe derives fresh domains; it does not reuse a previous binding's mask.

The completed eager consumer caches tables by signed predicate and canonical
column scope. Increasing original row positions feed the same whole-row matcher,
binding extension and authored-body checks used by indexed joins. Possible
support remains distinct from model truth. Support-growth rounds and structural
patterns keep indexed matching; this is an explicit scope boundary. Work,
allocation and capacity failures remain grounding failures. The strategies can
reach those limits at different points because their charged costs differ.

In both separate Table diagnostic blocks, actual preparation/reuse/probe/row
counts agree for all nine ordinary inputs. The comparison below uses the same
current executable for Indexed and Table. All join rows include support-growth
work, which remains indexed.

| Case | Preparations / reuses | Probes / selected rows | All join rows Indexed → Table | Index bytes | Support peak Indexed → Table |
|---|---:|---:|---:|---:|---:|
| Queens 1 | 3 / 145 | 148 / 8464 | 8752 → 8752 | 560 | 10920 → 14376 |
| Queens 2 | 3 / 267 | 270 / 9608 | 9752 → 9752 | 560 | 11016 → 14376 |
| Queens 3 | 3 / 154 | 157 / 8536 | 8680 → 8680 | 560 | 11016 → 14452 |
| Queens 4 | 3 / 54 | 57 / 2136 | 2280 → 2280 | 560 | 11016 → 14452 |
| Queens 5 | 5 / 198 | 203 / 616 | 1800 → 904 | 1744 | 29064 → 34096 |
| Queens 6 | 5 / 189 | 194 / 544 | 1872 → 976 | 1744 | 29064 → 34096 |
| SEND | 6 / 1658 | 1664 / 16404 | 16618 → 16604 | 1320 | 21912 → 26620 |
| Task allocation | 16 / 483 | 499 / 4767 | 7411 → 7339 | 2504 | 39600 → 45316 |
| Shortest path | 7 / 293 | 300 / 449 | 1153 → 979 | 2496 | 26600 → 31320 |

Index bytes count retained prepared-index capacity, excluding cache slots.
Support peak includes admitted support/query capacity, live masks and the
conservative operation envelope; aggregation takes the maximum. Neither is
RSS. Table removes many row visits for Queens 5/6 and some for SEND, task
allocation and shortest path. Queens 1–4 visit the same number. Preparation and
query work remain, so these counts do not imply shorter grounding time.

The reduction offers a possible path to larger instances: repeated joins can
amortize index construction, and selective bindings can limit the growth of
whole-row matching work as relations expand. Total cost still includes mask
operations, retained index storage and the final ground program. Measurements
under fixed time and memory budgets are needed to establish whether more or
larger instances become tractable; the current counts do not establish an
asymptotic or measured scalability gain.

The six additional N=8 grounding profiles each compare an indexed reference
with one unobserved, one boundary-observed and one detailed Table admission.
Every sample and reference exhausts the same 92 unique full native answer sets;
atom/formula catalogs, source evidence and available subject fingerprints agree.
The detailed records independently confirm table use and reuse. These single
rotated profile rounds are semantic and work-schedule evidence, not a speedup
estimate.

The mathematical [binding-family laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/guide/table-bindings.md)
preserve ordered source-row occurrences and binding results under explicit
matching and completeness premises. They do not verify Rust bitsets,
allocation/error behavior or the complete grounding translation.

This integration is scalar. Immutable table queries can be independent library
work, but the current consumer does not parallelize mutable bindings, formula
emission or source-error order. It launches no GPU table kernel. Useful next
measurements would separate preparation amortization from query pruning and
examine larger repeated flat joins. Any parallel consumer or GPU route would
also need bounded ownership, failure and source-order contracts; independent
primitive throughput is insufficient to select it automatically.

## Larger queens and complete native families

A separate bounded screen changes only the parsed `n = 8` literal in each
source closure. It includes all six encodings at N=10 and encodings 3/4 at N=12.
The N=12 subset was chosen for full-output size before measurement; it is not a
selection of favorable timings. Previous/default, current/Indexed and
current/Table use separate CPU/eager JSON/statistics reports, four requested
closure/completion workers, batch 64, qualification and one timed pair, no
warmup. Requested worker counts alone establish no parallel work.

| Population | Complete native family | External comparison |
| --- | ---: | --- |
| N=10, Queens 1 and 3–6 | 724 answers for each encoding | Qualification and timed pairs pass in all three configurations |
| N=10, Queens 2 | 724 answers in each native qualification capture | All three clingo qualification calls reach the 10-second limit; no qualified timed pair |
| N=12, Queens 3 | No complete family | All three native configurations refuse the Work limit |
| N=12, Queens 4 | 14,200 answers | Qualification and timed pairs pass in all three configurations |

Across 39 complete native captures, comparison resolves shown atom indices
against full typed models and preserves signs, recursive values, shown terms
and costs. Families agree across the relevant versions, strategies and repeat
phases, with no duplicate full models. This independently establishes more
native identity than the ordinary human-output protocol. It cannot supply
missing clingo evidence for N=10 Queens 2 or a family after the N=12 refusal.

The N=12 Queens 3 refusal reports Work capacity 1,048,576 with the next required
amount 1,048,577. Table's added preparation/query charges reach that bound at a
different partial grounding state; fewer emitted records before failure are
not a successful pruning result. All 96 scheduled positions remain accounted:
78 pass, 3 time out, 3 lack the reference, 3 are refused and 9 are blocked.
The 87 acquired captures have no capture faults or unresolved children.

The one timed native sample for each passing cell is shown below, in ms.
These are full JSON/statistics process times, not medians or isolated grounding
times. There is no RSS sample in this screen.

| Population | Previous | Current Indexed | Current Table |
| --- | ---: | ---: | ---: |
| N=10, Queens 1 | 101.33 | 102.51 | 101.04 |
| N=10, Queens 3 | 91.54 | 101.82 | 100.84 |
| N=10, Queens 4 | 64.24 | 70.29 | 69.26 |
| N=10, Queens 5 | 542.37 | 597.19 | 631.96 |
| N=10, Queens 6 | 551.98 | 604.04 | 642.62 |
| N=12, Queens 4 | 1822.70 | 1837.12 | 1824.89 |

The N=10 Queens 5/6 Table observations are slower than current Indexed in this
single fixed sequence. Their fewer row visits do not establish an application
speedup.

Same-build counters at N=10 reduce Queens 5 row visits from 2,750 to 950 and
Queens 6 from 2,640 to 840, while probe counts remain 293 and 282.
Each uses five Table preparations. Its reported index capacity is 4,520 bytes;
support peak rises from 53,576 to 62,024 bytes. N=12 Queens 4 keeps 7,092 visited
rows and raises support peak from 30,696 to 37,216 bytes. Less row scanning
does not imply less storage, and one timed pair supplies no reliable trend.
These reports include full JSON/statistics output and cannot be pooled with
the ordinary timing tables.

Separate corpus qualification passes all 94 cases with the canonical CPU
countermodel route. A Table-requested eager CPU matrix also passes all 376
positions over those 94 cases, using a 16-MiB per-child capture/decoder allowance.
Actual table probes occur on the twenty task-allocation inputs, including
aggregate/objective and unsatisfiable cases. These are qualification results,
not timings acquired in a quiet window. Matching clingo displays and optimum
ties does not reveal its hidden interpretations.


## Link-time optimization

The release profile stays normal. Fat LTO reduced the experimental executable
by 21.2% and reduced each of the nine observed native RSS pairs, but Queens 2,
SEND and task allocation were slower in both Fat blocks. Thin LTO showed
substantial drift and no clear overall benefit. No release setting was changed.

The three single-binary builds use the same source, Rust/Cargo 1.97.1,
aarch64-apple-darwin target and macOS 26.5 SDK. They build the CLI with its GPU
feature, separately from the installer package set. Each is one fresh-target
build in normal/thin/fat order; filesystem caching is uncontrolled. Build times
therefore do not establish a general compilation-speed ratio. Build peak RSS
is unavailable.

| Variant | Build wall / user / system seconds | Executable bytes | SHA-256 |
|---|---:|---:|---|
| Normal (lto=false) | 145.97 / 137.46 / 4.88 | 13,722,232 | `12b5edf96a25ca591ca88a919c6b040474c8597a85da03e6f8cb233214329a8d` |
| Thin | 154.68 / 145.05 / 6.14 | 13,915,944 | `a0a0f34b3f8d770c196f3090ac72ea681f610c85fe3964db53810d25273e9e13` |
| Fat | 128.78 / 119.30 / 5.36 | 10,806,616 | `9aa90f55a7fe9c852cc2aece5b3fe32612b74d049e4fe0cfb2e8118629ff7855` |

“Normal” means `lto=false`, which still permits local ThinLTO across codegen
units; it is distinct from Cargo's `"off"` setting. See the
[Cargo profile contract](https://doc.rust-lang.org/cargo/reference/profiles.html#lto).
These settings optimize Rust host code, not the WGSL program.

The following Thin comparison is Normal/Thin/Thin/Normal. Each cell is the
three-sample wall median [minimum, maximum], in ms.

| Case | normal-1 | thin-1 | thin-2 | normal-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 16.884 [15.389, 16.932] | 16.396 [15.442, 16.615] | 13.753 [13.743, 13.813] | 13.736 [13.732, 13.766] |
| Queens 2 | 95.221 [93.867, 95.417] | 96.283 [96.207, 97.678] | 87.580 [87.524, 87.586] | 88.763 [87.493, 89.097] |
| Queens 3 | 16.919 [16.879, 16.979] | 16.979 [16.910, 17.017] | 13.781 [13.718, 15.357] | 15.271 [13.729, 15.306] |
| Queens 4 | 9.412 [8.995, 9.419] | 9.348 [9.309, 9.480] | 7.725 [7.715, 7.741] | 7.726 [7.716, 7.813] |
| Queens 5 | 10.859 [10.775, 10.922] | 10.868 [10.841, 10.875] | 9.236 [9.211, 9.241] | 9.239 [9.217, 9.247] |
| Queens 6 | 10.908 [10.870, 12.382] | 10.912 [10.888, 11.562] | 9.222 [9.211, 9.230] | 9.209 [9.193, 10.745] |
| SEND | 36.520 [34.973, 36.627] | 35.671 [34.580, 36.402] | 30.304 [30.297, 31.893] | 31.846 [31.815, 31.854] |
| Task allocation | 101.440 [100.440, 102.423] | 100.974 [100.435, 101.089] | 94.092 [93.958, 95.524] | 94.030 [93.975, 96.915] |
| Shortest path | 9.468 [9.234, 9.476] | 9.448 [7.884, 9.474] | 7.814 [7.794, 7.880] | 7.840 [7.814, 7.894] |

The closing Normal block is much faster than its opening block on several
cases. Matched clingo Queens 2 medians also move from 128.362 to 119.085 ms,
and task allocation from 194.058 to 180.967 ms. This prevents a clean broad
Thin-LTO speed claim. Its separate native RSS observations are slightly higher
on this population.

The Fat comparison is Normal/Fat/Fat/Normal, with the same notation.

| Case | normal-1 | fat-1 | fat-2 | normal-2 |
|---|---:|---:|---:|---:|
| Queens 1 | 13.726 [13.722, 13.779] | 13.720 [13.718, 15.238] | 13.712 [13.711, 13.730] | 13.768 [13.731, 15.307] |
| Queens 2 | 89.126 [87.539, 89.144] | 101.157 [101.096, 102.533] | 101.086 [101.080, 102.641] | 90.642 [89.048, 90.678] |
| Queens 3 | 15.347 [13.723, 15.474] | 15.243 [15.237, 15.301] | 15.311 [15.210, 15.324] | 15.301 [15.229, 15.332] |
| Queens 4 | 7.742 [7.732, 7.764] | 7.713 [7.701, 7.732] | 7.711 [7.217, 7.727] | 7.726 [7.722, 7.799] |
| Queens 5 | 9.261 [9.214, 9.280] | 9.209 [9.209, 9.215] | 9.203 [9.191, 9.213] | 9.210 [9.209, 9.224] |
| Queens 6 | 9.220 [9.220, 9.222] | 9.227 [9.176, 10.711] | 9.192 [9.176, 10.715] | 10.376 [9.191, 10.737] |
| SEND | 32.969 [31.795, 33.368] | 36.323 [34.778, 36.366] | 36.328 [34.755, 36.334] | 33.304 [33.111, 33.394] |
| Task allocation | 95.516 [94.042, 95.548] | 101.473 [99.989, 101.504] | 99.972 [99.929, 101.493] | 96.977 [95.446, 98.442] |
| Shortest path | 7.814 [7.798, 7.880] | 7.827 [7.782, 7.844] | 7.804 [7.802, 7.849] | 7.806 [7.782, 7.892] |

Queens 2 changes from control medians 89.126/90.642 to 101.157/101.086 ms;
SEND from 32.969/33.304 to 36.323/36.328 ms; task allocation from 95.516/96.977
to 101.473/99.972 ms. Matched clingo SEND medians stay near 10.7 ms, while
task allocation's reference varies from 164.376 to 182.206 ms across the
control blocks. These are bounded observations, not universal ratios.

The lower Fat RSS is a separate observed benefit. For example, Queens 2's
Normal samples are 12.547/12.516 MiB and Fat's 11.938/11.953; task allocation's
are 19.547/19.484 versus 18.891/18.844 MiB. Binary size alone would not establish
this result. With only two RSS samples per side and mixed runtime costs, these
measurements do not justify replacing the normal installation profile. Neither
LTO comparison measures Table joins, GPU execution or active CPU parallelism.


## Matched eager CPU and Metal

This comparison uses the canonical executables and corpus identified above,
with eager grounding and automatic oracle selection. Each case/profile has a
qualification sample, one warmup and three timed samples. Both native profiles
request four closure/completion workers and batch 64; clingo uses one worker.
The matrix includes full native JSON/statistics and clingo JSON. It is a separate
population from the ordinary human-output and RSS measurements; it has no RSS
samples. Every sample starts a fresh child; each Metal invocation initializes
its own GPU context.

All twelve reports pass: 810 positions, with 540 native and 270 clingo captures.
There are no refusals, blocked positions, capture faults or unresolved children.
Complete native families agree across the relevant versions, strategies and
CPU/Metal routes, including typed signed atoms, resolved shown values and costs.
SEND has one answer, each queens encoding has 92, and task allocation publishes
1,176 optimum ties. Clingo corroborates the selected displays and objective
results; its hidden interpretations remain unavailable.

The measurements span **two separate windows** on 12 September 2026. The first
ran 20:48:12–20:54:30 CDT and stopped on its scheduling allowance after seven
reports. The five unstarted reports ran at 21:25:04–21:28:36, after a
**30-minute 34-second gap**. Prior-1, Indexed-1, Table-1 and Table-2 baseline are
in the first window; Table-2 queens, Indexed-2 and prior-2 are in the second.
The Table-2 column therefore crosses the window boundary. No report was repeated
or replaced. The original order was retained, but it does not supply an
uninterrupted mirrored experiment.

The two suites keep Queens 2 in separate contexts: `baseline` contains SEND,
task allocation and Queens 2; `queens` contains all six encodings. The tables
preserve those contexts and every block. Cells are **median [minimum, maximum]
in milliseconds**, with three timed samples each. No confidence interval or
pooled cross-window estimate is claimed.

### CPU eager

| Case | prior-1 | Indexed-1 | Table-1 | Table-2 | Indexed-2 | prior-2 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| SEND | 34.189 [34.161, 35.584] | 34.209 [34.197, 35.629] | 35.508 [35.468, 35.632] | 34.984 [34.974, 35.136] | 35.169 [34.216, 36.954] | 34.204 [34.166, 35.625] |
| Task allocation | 326.493 [325.583, 326.584] | 325.434 [322.771, 325.741] | 324.049 [322.956, 325.237] | 367.010 [365.734, 368.144] | 323.431 [323.094, 325.910] | 326.138 [325.402, 327.925] |
| Queens 2 (baseline) | 95.890 [94.654, 95.937] | 94.566 [93.323, 94.580] | 94.588 [94.554, 95.967] | 95.401 [95.356, 98.426] | 95.905 [94.381, 97.307] | 94.650 [94.457, 97.052] |
| Queens 1 | 17.885 [17.875, 19.294] | 17.960 [17.929, 19.410] | 18.500 [17.948, 19.442] | 19.208 [17.969, 19.299] | 18.018 [17.951, 19.434] | 17.928 [17.912, 19.431] |
| Queens 2 | 98.283 [97.948, 98.350] | 94.665 [94.562, 95.588] | 94.582 [94.566, 95.317] | 92.242 [92.205, 93.970] | 95.876 [94.675, 95.977] | 95.889 [94.702, 97.117] |
| Queens 3 | 19.151 [19.139, 19.155] | 19.222 [17.856, 19.246] | 19.223 [19.181, 19.239] | 19.155 [18.964, 19.260] | 19.267 [19.262, 19.301] | 19.201 [18.935, 19.236] |
| Queens 4 | 11.593 [11.458, 11.697] | 11.707 [11.671, 11.739] | 11.722 [10.338, 11.777] | 11.751 [11.696, 12.989] | 11.587 [10.338, 11.697] | 11.777 [10.333, 11.842] |
| Queens 5 | 47.098 [46.952, 48.574] | 49.471 [47.030, 49.769] | 49.466 [46.934, 49.653] | 48.717 [48.039, 49.006] | 49.454 [47.017, 49.598] | 49.228 [49.143, 49.810] |
| Queens 6 | 48.409 [48.364, 48.427] | 49.494 [49.437, 49.570] | 49.507 [49.424, 53.568] | 49.328 [48.864, 49.367] | 49.186 [49.168, 49.530] | 49.486 [49.466, 49.574] |

### Metal eager

| Case | prior-1 | Indexed-1 | Table-1 | Table-2 | Indexed-2 | prior-2 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| SEND | 66.990 [65.708, 67.082] | 67.028 [65.707, 67.082] | 66.878 [65.778, 67.153] | 66.745 [66.665, 66.774] | 67.056 [66.840, 68.441] | 66.980 [66.898, 67.045] |
| Task allocation | 572.618 [569.779, 586.927] | 569.372 [564.941, 571.553] | 571.374 [559.004, 575.246] | 613.848 [610.517, 648.716] | 572.346 [563.973, 577.466] | 573.431 [571.637, 603.987] |
| Queens 2 (baseline) | 111.075 [109.479, 112.129] | 109.698 [109.691, 109.781] | 109.710 [108.359, 109.753] | 111.897 [110.275, 112.073] | 110.896 [109.745, 113.477] | 112.216 [111.984, 113.227] |
| Queens 1 | 34.255 [34.105, 34.301] | 32.968 [32.892, 34.175] | 33.050 [33.043, 35.075] | 35.185 [34.299, 35.561] | 33.041 [32.986, 34.357] | 32.944 [32.816, 35.504] |
| Queens 2 | 114.756 [114.293, 115.889] | 110.912 [109.693, 111.026] | 110.118 [109.594, 111.047] | 110.340 [109.753, 111.024] | 110.839 [110.719, 111.117] | 111.072 [109.604, 112.263] |
| Queens 3 | 34.312 [34.254, 35.534] | 33.047 [32.972, 34.216] | 34.225 [33.000, 34.392] | 35.572 [35.541, 39.504] | 34.246 [34.202, 34.318] | 33.023 [32.966, 34.374] |
| Queens 4 | 26.678 [26.638, 26.701] | 25.515 [25.486, 25.536] | 25.507 [25.486, 25.507] | 28.005 [27.509, 29.297] | 25.502 [25.292, 25.508] | 25.404 [25.250, 25.438] |
| Queens 5 | 63.634 [63.618, 65.951] | 65.961 [64.798, 65.977] | 64.542 [63.639, 65.998] | 67.397 [66.555, 68.377] | 65.875 [65.785, 65.944] | 65.740 [64.539, 65.788] |
| Queens 6 | 64.664 [63.454, 64.944] | 65.930 [65.780, 67.284] | 65.963 [64.533, 74.453] | 67.188 [66.939, 69.429] | 67.042 [65.739, 67.119] | 65.554 [64.505, 67.010] |

### Matched clingo

| Case | prior-1 | Indexed-1 | Table-1 | Table-2 | Indexed-2 | prior-2 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| SEND | 11.534 [11.519, 11.621] | 11.542 [11.498, 11.594] | 11.540 [11.540, 11.649] | 12.296 [12.292, 12.404] | 11.568 [11.548, 11.646] | 11.551 [11.526, 11.623] |
| Task allocation | 179.906 [154.734, 182.434] | 182.444 [173.605, 184.828] | 183.691 [182.366, 184.914] | 175.069 [149.345, 187.131] | 168.603 [154.642, 177.509] | 177.486 [174.749, 179.732] |
| Queens 2 (baseline) | 124.401 [123.226, 124.449] | 123.324 [123.270, 124.563] | 123.268 [123.231, 123.385] | 123.721 [123.325, 125.090] | 123.196 [123.096, 123.309] | 122.967 [121.812, 123.319] |
| Queens 1 | 6.534 [6.527, 6.576] | 6.521 [6.501, 6.576] | 6.249 [5.237, 6.606] | 6.536 [6.153, 6.606] | 6.537 [6.522, 6.557] | 6.538 [6.518, 6.675] |
| Queens 2 | 129.398 [128.186, 129.572] | 123.261 [123.207, 123.293] | 123.274 [122.143, 123.296] | 120.793 [120.733, 123.033] | 124.458 [124.033, 124.488] | 123.196 [121.958, 124.526] |
| Queens 3 | 6.541 [6.518, 6.541] | 6.527 [5.203, 6.538] | 6.365 [5.225, 6.534] | 6.529 [6.528, 6.632] | 6.542 [6.541, 6.563] | 6.526 [6.515, 6.529] |
| Queens 4 | 6.512 [6.487, 6.533] | 6.537 [6.449, 6.540] | 6.537 [6.448, 6.539] | 6.528 [6.462, 6.540] | 6.528 [6.460, 6.546] | 6.543 [6.452, 6.580] |
| Queens 5 | 6.587 [6.493, 6.589] | 6.588 [6.469, 6.598] | 6.599 [6.496, 6.615] | 6.604 [6.521, 6.620] | 6.599 [6.464, 6.604] | 6.614 [6.465, 6.624] |
| Queens 6 | 6.598 [6.593, 6.609] | 6.585 [6.559, 6.615] | 6.553 [6.380, 6.586] | 6.597 [6.577, 6.617] | 6.593 [6.592, 6.600] | 6.589 [6.577, 6.602] |

Metal is slower than its matched CPU route in all nine case/suite contexts.
The routes use different certified membership procedures, so this is an
end-to-end execution comparison, not an isolated hardware or Rayon speed ratio.
There is no broad speedup from Table or from the new release on this population.
Indexed remains the default.

SEND's Metal block medians remain between 66.745 and 67.056 ms. Task allocation
usually remains near 569–573 ms, while Table-2 baseline reaches 613.848 ms;
its matched CPU median also rises to 367.010 ms from the other blocks' 323–326 ms.
That observation belongs to the first window. The Table-2 solving-stage medians
are still 95.473 ms CPU and 335.605 ms Metal; its grounding medians are
2.754 and 2.710 ms. Marginal phase medians do not add to process time, and nested
phases must not be counted again. These observations do not identify a Table
regression or remove the visible end-to-end variation.

All 270 Metal captures report actual, settled GPU work with complete candidate
accounting. SEND's one candidate and each queens run's 92 candidates are decided
on the GPU with no CPU residuals. Task allocation submits 1,208 candidates:
13–14 are GPU-decided and the remaining 1,194–1,195 require exact CPU completion
with four admitted workers. Requested workers alone would not establish that
activity. Neither route turns a partial answer prefix into a complete family.

All 180 Table-requested native captures report actual table preparation and
probes. For example, task allocation records 14 preparations, 483 reuses,
497 probes and 4,765 selected rows in this matrix. These are host grounding
operations composed with the selected oracle; they do not execute a GPU table
kernel. Fewer row visits, library reuse and successful GPU execution establish
neither a general latency improvement nor lower process/device memory.

## Earlier Metal measurements

The [complete previous comparison](https://github.com/GregoryGelfond/zetesis/blob/993a7bbb625ae62ea4ff0ef4510c3d1a8be514ac/docs/book/reference/performance.md#instrumented-cpu-and-metal-comparison)
compared sources `15e0f77b` and `6bebb980` on the same Apple M4 Pro.
Its eager Metal SEND median decreased from 79.853 to 65.836 ms, and task
allocation from 628.813 to 576.636 ms. Those measurements included host candidate
search, uploads, dispatch/wait, exact CPU residual completion and JSON/statistics
output. The matched CPU route was faster on every measured case and used a
different certified membership procedure.

Those values belong to `6bebb980`, not to the new table consumer or an LTO
variant. Physical regression coverage and release performance are separate
claims; neither historical throughput nor a requested Metal backend proves
current device execution.

## Optional finite-table experiment

This retained **historical** population measured source `6bebb980`, before
ordinary eager grounding had a Table consumer. It compared complete row/domain
projection with a prepared scan and independent scalar/Rayon queries. All
2,880 query observations across 90 batches agreed with independent whole-row
reconstruction, including aliases, duplicate occurrences and restored domains.

Each cell is microseconds for 32 queries: median [minimum, maximum] of three
timed batches. Preparation and independent validation are excluded; projection
and common-output conversion are included.

| Fixture | Rows | Prepared scan | Scalar table | Rayon table, 4 workers |
| --- | ---: | ---: | ---: | ---: |
| Correlated | 128 | 82.042 [81.417, 108.208] | 43.250 [43.167, 44.667] | 40.417 [38.417, 47.000] |
| Correlated | 1,024 | 669.500 [644.167, 680.417] | 115.667 [95.250, 117.625] | 67.417 [59.375, 77.125] |
| Independent | 128 | 181.917 [181.792, 182.292] | 45.417 [44.209, 53.292] | 34.333 [33.709, 51.000] |
| Independent | 1,024 | 1,868.500 [1,836.292, 1,906.875] | 128.875 [109.667, 142.250] | 90.708 [75.833, 138.167] |
| Aliased | 128 | 181.875 [180.750, 183.625] | 38.458 [37.417, 45.167] | 30.584 [29.667, 30.917] |
| Aliased | 1,024 | 1,589.667 [1,570.167, 1,629.708] | 122.417 [121.291, 124.458] | 65.000 [61.041, 115.000] |

The fixed scan/table/Rayon order and three batches do not establish a stable
parallel crossover. For the independent 1,024-row fixture, the single measured
preparations were 546.042 µs for the relation and 274.125 µs for the Table index;
the index retained 9,080 bytes. Named capacities are not RSS.

These earlier projection results motivated the reusable consumer now described
above. Ordinary grounding uses borrowed row selection, not the experiment's
complete domain-projection output. Its preparation, live masks and remaining
grounding work must be measured together. There is still no GPU Table kernel.
The [historical method and limits](https://github.com/GregoryGelfond/zetesis/blob/993a7bbb625ae62ea4ff0ef4510c3d1a8be514ac/docs/book/reference/performance.md#optional-finite-table-experiment)
remain available with the original standalone commands.


## Reproduce the measurements

Use the [installer](https://github.com/GregoryGelfond/zetesis/blob/main/scripts/install.sh)
to build canonical commands from separate checkouts of the named sources.
Preserve each binary, actual build arguments, compiler/SDK identity and hash.
A rebuild may have different bytes even at the same source commit. Use one fixed
`zetesis-perf` for every compared solver.

From the checkout root, set absolute executable paths and a fresh output
directory. The ordinary comparison includes the same nine paths in each call:

```sh
perf_command=/absolute/path/to/zetesis-perf
previous_solver=/absolute/path/to/previous/zetesis
current_solver=/absolute/path/to/current/zetesis
clingo_command=/absolute/path/to/clingo
bench_command=/absolute/path/to/current/zetesis-bench
results_dir=$(mktemp -d "${TMPDIR:-/tmp}/zetesis-perf.XXXXXX")

ordinary() {
    comparison_label=$1
    comparison_solver=$2
    comparison_joins=$3
    set -- "$perf_command" examples/kr-domains \
        --zetesis "$comparison_solver" --clingo "$clingo_command" \
        --report "$results_dir/$comparison_label.json"
    if [ "$comparison_joins" != default ]; then
        set -- "$@" --formula-joins "$comparison_joins"
    fi
    set -- "$@" \
        --case standalone/n-queens/variant-01.lp \
        --case standalone/n-queens/variant-02.lp \
        --case standalone/n-queens/variant-03.lp \
        --case standalone/n-queens/variant-04.lp \
        --case standalone/n-queens/variant-05.lp \
        --case standalone/n-queens/variant-06.lp \
        --case standalone/send-money/send-money.lp \
        --case scenarios/task-allocation/variant-04/05-larger-mix.lp \
        --case scenarios/shortest-path/variant-01/06-layered-dag.lp \
        --warmups 1 --repetitions 3 --memory-runs 1 \
        --timeout-seconds 5 --campaign-seconds 30 \
        --sample-bytes 4194304 --capture-bytes 134217728 \
        --report-bytes 536870912
    if /usr/bin/env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
        LC_ALL=C TMPDIR=/private/tmp "$@"; then
        comparison_exit=0
    else
        comparison_exit=$?
    fi
    printf '%s\n' "$comparison_exit" > "$results_dir/$comparison_label.exit" || return 2
    return "$comparison_exit"
}
```

Run each call separately in a quiet window and inspect its report before
continuing, especially after a nonzero result. The prior solver does not accept
the new strategy flag, so `default` deliberately omits it.

```sh
ordinary release-prior-1 "$previous_solver" default
ordinary release-current-1 "$current_solver" indexed
ordinary release-current-2 "$current_solver" indexed
ordinary release-prior-2 "$previous_solver" default

ordinary joins-indexed-1 "$current_solver" indexed
ordinary joins-table-1 "$current_solver" table
ordinary joins-table-2 "$current_solver" table
ordinary joins-indexed-2 "$current_solver" indexed
```

The measured acquisition used a clean environment with a fixed locale, absolute
executables and no inherited solver flags. Exit 0 means all requested positions
pass; exit 1 retains nonpassing evidence, including refusals or operational
failures; setup/publication failure exits 2. Preserve actual exits, raw reports
and all blocked positions. A refused or uncaptured solve has no imputed time.
Child and campaign deadlines are polling boundaries; cleanup and publication
may extend process wall time.

For the matched eager CPU/Metal matrix, use the same four executables and corpus.
The `baseline` suite selects SEND, task allocation and Queens 2; `queens` selects
all six N=8 encodings. Keep Queens 2's two contexts distinct.

```sh
metal_matrix() {
    comparison_label=$1
    comparison_suite=$2
    comparison_solver=$3
    comparison_joins=$4
    set -- "$perf_command" examples/kr-domains \
        --suite "$comparison_suite" \
        --zetesis "$comparison_solver" --clingo "$clingo_command" \
        --report "$results_dir/$comparison_label.json" \
        --profile cpu-eager --profile metal-eager \
        --workers 4 --completion-workers 4 --clingo-workers 1 --batch-size 64 \
        --warmups 1 --repetitions 3 --timeout-seconds 10 --campaign-seconds 30 \
        --sample-bytes 33554432 --native-report-bytes 33554432 \
        --capture-bytes 134217728 --report-bytes 268435456
    if [ "$comparison_joins" != default ]; then
        set -- "$@" --formula-joins "$comparison_joins"
    fi
    if /usr/bin/env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
        LC_ALL=C TMPDIR=/private/tmp "$@"; then
        comparison_exit=0
    else
        comparison_exit=$?
    fi
    printf '%s\n' "$comparison_exit" > "$results_dir/$comparison_label.exit" || return 2
    return "$comparison_exit"
}

metal_matrix metal-prior-1-baseline baseline "$previous_solver" default
metal_matrix metal-prior-1-queens queens "$previous_solver" default
metal_matrix metal-indexed-1-baseline baseline "$current_solver" indexed
metal_matrix metal-indexed-1-queens queens "$current_solver" indexed
metal_matrix metal-table-1-baseline baseline "$current_solver" table
metal_matrix metal-table-1-queens queens "$current_solver" table
metal_matrix metal-table-2-baseline baseline "$current_solver" table

# The recorded second acquisition window starts here.
metal_matrix metal-table-2-queens queens "$current_solver" table
metal_matrix metal-indexed-2-baseline baseline "$current_solver" indexed
metal_matrix metal-indexed-2-queens queens "$current_solver" indexed
metal_matrix metal-prior-2-baseline baseline "$previous_solver" default
metal_matrix metal-prior-2-queens queens "$previous_solver" default
```

As above, run one call at a time and inspect the retained report before starting
the next. Preserve any scheduling stop and the window boundary when continuing;
do not replace an unstarted report with a guessed measurement. This matrix
captures full native JSON/statistics and retains native model records. It allows
32 MiB per child/native decoder input, 128 MiB total capture and 256 MiB report
output, retaining the decoder's other structural limits. There are no RSS
samples. Each Metal invocation initializes its own GPU context.

For the detailed N=8 profile, run each original source separately:

```sh
"$bench_command" grounding \
  examples/kr-domains/standalone/n-queens/variant-01.lp \
  --joins table --repetitions 1 > "$results_dir/grounding-q01.json"
```

Repeat for variants 02–06 with distinct output names. Each command performs the
unmeasured indexed reference and three observed/unobserved admissions; it is not
a bare table-kernel timer. The [grounding experiment contract](https://github.com/GregoryGelfond/zetesis/tree/main/crates/zetesis-experiments#original-source-grounding)
defines its default work, capture and complete-model bounds.

The larger screen composes the public
[`Workload::amended` and `run_workloads` APIs](https://github.com/GregoryGelfond/zetesis/tree/main/crates/zetesis-validation#derive-explicit-parameter-workloads).
The [runnable workload example](validation.md#compare-a-parameterized-workload)
shows the acquisition boundary. To reproduce the screen, derive `n: 8→10`
for all six encodings and `8→12` for variants 03/04, using a separate request for
prior/default, current/Indexed and current/Table. Keep original files unchanged.
Use the CPU backend, eager grounding and automatic oracle selection, with four
requested closure/completion workers, batch size 64, one clingo worker, no
warmups and one timed pair after qualification.

The screen's child/campaign intervals are 10/90 seconds. It allows 128 MiB
per captured child, 512 MiB total capture and 1 GiB final report; decoding allows
16,384 model witnesses, 262,144 shown-symbol occurrences, 1,048,576 native atom
occurrences and 4,194,304 native value nodes, with 32 MiB shown spelling.
Those settings are larger than the N=4 example's defaults. One amended workload
retains at most 16 KiB each of original/derived source, 64 KiB combined source
closure and 16 KiB metadata. Bounds and workload hashes remain part of each
report. This is a library client, not an undocumented solver CLI option.

For the separate LTO study, use fresh build targets for each setting. The
following is the normal single-binary recipe; replace only the final
`profile.release.lto=false` with `profile.release.lto="thin"` or
`profile.release.lto="fat"` for the other variants:

```sh
cargo +1.97.1 build --locked --offline --release \
  --target aarch64-apple-darwin --target-dir /absolute/path/to/fresh-target \
  -p zetesis-cli --bin zetesis --features gpu \
  --config 'profile.release.opt-level=3' \
  --config 'profile.release.codegen-units=16' \
  --config 'profile.release.debug=false' \
  --config 'profile.release.debug-assertions=false' \
  --config 'profile.release.overflow-checks=false' \
  --config 'profile.release.panic="unwind"' \
  --config 'profile.release.strip="none"' \
  --config 'profile.release.incremental=false' \
  --config 'profile.release.rpath=false' \
  --config 'profile.release.lto=false' \
  --timings --message-format=json -vv
```

The measurements used one Cargo job, incremental compilation disabled, no Cargo
configuration overrides beyond those shown, Xcode 26.6 and its macOS 26.5 SDK.
Preserve build wall/user/system times separately from runtime RSS. Repeat the
ordinary A/B/B/A method with normal/thin and normal/fat, always requesting
Indexed. Do not substitute these artifacts into the canonical release table.

The repository supplies corpus sources, contracts and measurement APIs.
Original raw captures are retained privately, not offered as a public download.
The commands and API settings above generate new self-contained reports with
captured outputs, identities and bounded outcomes.
