# Lending completed grounding rows

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

Lending a completed join binding to its immediate consumer reduced dense-rule
instantiation time in this four-program CPU comparison. On the larger dense
case, the improvement also reached whole-process time. The smaller case and
controls do not support a general speedup or a memory-saving claim.

The comparison is baseline
[`71020b34`](https://github.com/GregoryGelfond/zetesis/commit/71020b34f93935e34fde95e85e4f7d77f750f027)
against
[`3d7454d8`](https://github.com/GregoryGelfond/zetesis/commit/3d7454d810acec99121ee5b5615505b6ddc6eea7).
Both contain the same borrowed candidate-bound implementation. This comparison
therefore measures the subsequent grounding change, not their combined benefit.

## Results

Two blocks each ran fresh executables in A1, B1, B2, A2 order, where A is the
baseline and B the candidate. Each leg's time is the median of seven samples.
The table shows the range of those four medians per version, in milliseconds.
Changes are the four paired percentages, B1/A1 and B2/A2 in each block;
negative means less time. These ranges are descriptive, not confidence intervals.

| Generated program | CPU threads | Baseline ms | Candidate ms | Whole-process change |
| --- | ---: | ---: | ---: | ---: |
| TransitiveDense20 | 1 | 9.134–9.254 | 9.105–9.182 | −1.6% to +0.4% |
| TransitiveDense20 | 4 | 9.130–9.237 | 9.060–9.200 | −1.5% to +0.8% |
| TransitiveDense40 | 1 | 25.369–25.720 | 24.323–24.351 | −5.3% to −4.1% |
| TransitiveDense40 | 4 | 25.576–26.740 | 24.389–25.500 | −4.6% to −0.7% |
| Chain1000 | 1 | 16.537–16.837 | 15.511–16.628 | −7.2% to −1.2% |
| Chain1000 | 4 | 15.563–16.488 | 15.537–16.740 | −5.8% to +6.6% |
| ChainArithmetic1000 | 1 | 10.403–10.460 | 10.399–10.466 | −0.1% to +0.3% |
| ChainArithmetic1000 | 4 | 10.442–10.512 | 10.395–10.526 | −0.9% to +0.1% |

Dense40 rule instantiation took **6.4–10.0% less time**, and its complete grounding
stage took **4.3–7.7% less**. Dense20 rule instantiation took **5.7–11.6% less**,
but its whole-process changes ranged from −1.6% to +0.8%.

The lazy Chain control does not use the changed eager path. Its four-thread
whole-process changes cross zero in both blocks, so its movement cannot be
credited to row lending or described as established neutrality. Arithmetic
timings are also mixed. Nested phase medians overlap and must not be added to
reconstruct total time. The requested thread count does not make eager source
preparation parallel.

## What changed

An eligible completed row now borrows the join's existing `Binding`; its final
undo waits until the next advance. Generators and consumers that retain a row
still request ownership. The matcher, source arithmetic policy, emitted
formulas and reduct checks are unchanged. The
[ownership contract](../architecture/ownership.md) describes the lifetime and
resource boundaries; the [proof correspondence](../lean/correspondence.md#ownership-and-execution-correspondence)
states the remaining Rust refinement obligations.

Every observed native phase retained the following snapshot populations at both
thread counts and in both blocks:

| Program | Rule snapshots, baseline → candidate | Support snapshots, unchanged |
| --- | ---: | ---: |
| TransitiveDense20 | 1,540 → 0 | 400 |
| TransitiveDense40 | 11,480 → 0 | 1,600 |
| ChainArithmetic1000 | 1,001 → 1,000 | 1,001 |

The arithmetic case retains its 1,000 owned generated continuations. Snapshot
counts include empty frames and are not allocator-event or copied-byte counts.
Lazy Chain has no separate eager-grounding measurement; absence is not zero.
Separate RSS observations do not establish a repeatable memory reduction.

## Protocol and evidence

Measurements used aarch64 macOS 26.6.2, build 25G83, with Darwin machine identifier
T6041. Each case ran with CPU threads 1 and 4, auto grounding and oracle selection,
indexed formula joins, region search, batch size 64, one completion worker,
statistics and JSON output, and an observed 2 GiB native memory allowance.
Clingo 5.8.2 used one thread. Dense and arithmetic inputs selected eager CPU
positive consequences; Chain selected lazy interleaved CPU closure.

Each case and producer in each leg had one qualification run, **two warmups,
seven timed runs and two separate RSS runs**. All **1,152 scheduled positions**
passed. The **768 native family checks** retained full atoms, multiplicity and
costs and agreed across versions and profiles; all four programs have one model
and no objective. Clingo comparison checks its complete selected family, without
claiming access to hidden clingo atoms.

The [first block](observations/rows-3d7454d8-block-1.json) and
[second block](observations/rows-3d7454d8-block-2.json) retain each leg's medians
and paired changes. The [evidence projection](observations/rows-3d7454d8-evidence.json)
contains executable and report hashes, generated-source identities, full-family
fingerprints, individual timed observations, work counters and separate RSS
receipts with helper and solver process identities. Both blocks are retained.

Inputs come from the maintained
[`Workload::generated`](https://github.com/GregoryGelfond/zetesis/blob/3d7454d810acec99121ee5b5615505b6ddc6eea7/crates/zetesis-validation/src/performance/matrix/workload.rs)
API: `TransitiveDense` at sizes 20 and 40, `Chain` at 1,000 and `ChainArithmetic`
at 1,000. The
[`matrix` API](https://github.com/GregoryGelfond/zetesis/blob/3d7454d810acec99121ee5b5615505b6ddc6eea7/crates/zetesis-validation/src/performance/matrix.rs)
provides the instrumented workload protocol; source and binary identities must
be matched when repeating it. Wall time includes input, preparation, solving,
statistics and output. This experiment does not measure statistics-free solving,
Metal, the full corpus or general parity with clingo.
