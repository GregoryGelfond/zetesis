# Checked atom lookup probe

`atom_lookup.rs` is a bounded consumer of the public `AtomIndex`, `Model::lookup`
and checked typed lookup APIs. It does not read an ASP source or invoke a solver.
One immutable atom catalog owns the fixture payload; the prepared index borrows
it, and a sparse `Model` shares it. The index covers all original rows, while the
model exposes only its selected true rows in canonical order.

Run deterministic correctness and accounting controls:

```sh
cargo run --locked -p zetesis-core --example atom_lookup -- --check
```

For a separately scheduled release measurement, omit `--check`:

```sh
cargo run --locked --release -p zetesis-core --example atom_lookup
```

The fixed seven cases vary catalog rows (128, 1,024, 4,096), predicate count
(1, 16, 256) and structural depth (0, 8, 32). Each has 16 queries, including
typed misses and signed predicates; measured mode repeats that list eight times.
Every index construction and query batch has a 100-million operation ceiling.
The linear reference separately bounds row visits at that same finite number.
There are no configurable unbounded dimensions or automatic retries.

For every query both implementations check exact key presence, original dense
identity and the complete ordered predicate row sequence. The reference uses
an independent linear scan over the original or canonical selection, with the
same borrowed typed equality used by the catalog views.
No hashes, projected strings or row counts stand in for the identity assertions.

The CSV reports index construction work/time, model construction time, repeated
lookup work/time, selected-row visits, and the reference's scan visits/time.
Lookup work includes probes, canonical storage/navigation operations, visited
typed descriptors/text bytes, sequence ends and consumer row visits. Reference visits use a different unit and must not be
divided into lookup work to claim a speed ratio. Model construction uses the
existing constructor and its standard sort; its logical work is not measured by
this probe. Present-key queries are explicitly exported to independent owned input
fixtures before timing; missing keys are also owned fixtures. Fixture/query/
reference preparation and CSV publication are outside the reported timings. Measured loops retain correctness assertions, use a fixed
route order and run once per case, so they are bounded scaling observations,
not an isolated kernel-speed or statistically replicated application result.

Storage columns are actual index retained/preparation-peak integer capacities,
canonical catalog named storage, model position capacity and the model's
conservative canonical payload admission measure. Catalog storage includes its
fixed snapshot and occurrence mapping; allocator bookkeeping and Arc reference
counters remain excluded. Query/reference fixtures and other process state are
outside these measurements. The current `catalog_storage_bytes` column replaces
the historical `atom_cell_bytes` column, which measured only the old source
atom-vector cells. Those columns have different meanings and must not be compared
as the same storage measure. Portable canonical payload bytes are also distinct
from physical named storage and must not be added to it. The shared index setup
receipts are repeated on the two route rows and should be counted once per case.
They do not imply that `Model::lookup` constructs or retains an index. None of
these columns is peak RSS or a process memory limit.

Retain source, compiler, executable, command, exit and host identities alongside
any measured CSV. Ordinary objective solving requires its own matched full-model
and cost comparison, including setup, search, output and completion.

# Appendable atom interning probe

`atom_interning.rs` exercises the public `AtomInterner` independently of the
immutable lookup probe above. It constructs each catalog in four equal append
rounds, checks repeated membership through both owned-atom and borrowed-key
doors, obtains canonical committed snapshots and commits each pending suffix.
The fixed final populations are 128, 1,024 and 4,096 distinct atoms. No source
parser, grounder, solver, device or parallel worker participates.

Run the same semantic and accounting controls without timings:

```sh
cargo run --locked -p zetesis-core --example atom_interning -- --check
```

For a separately scheduled release measurement:

```sh
cargo run --locked --release -p zetesis-core --example atom_interning
```

The descending fixture includes paired positive/negative predicates, numbers,
numeric strings and negative structured functions containing a tuple. Dense IDs
must equal first-insertion positions. A separate reference sorts original IDs
with typed `Atom::Ord`; both snapshots must return that exact order and
payload identity. During append, the retained committed view must stay unchanged
while the appender can find new pending atoms. Before commit the ordered snapshot
excludes those atoms; afterward it includes them. Neither membership nor
commitment establishes logical truth.

Each round queries every admitted atom four times through each lookup door and
checks its original ID, including both committed and pending atoms. Occupied
insertion must return that ID without copy/write work. Sixteen signed, absent
symbol keys are each queried four times without insertion. Each commit must
preserve the original dense order and typed contents. Final catalog publication
checks those same rows after transferring the discovery map and sharing the
canonical prefix; it still admits publication work when no discoveries remain
pending.

All interner operations use `Limits::for_atoms(rows, CANONICAL_BYTES)` for the
stated final population. This fixture supplies 64 MiB for canonical storage and
snapshot directories; arbitrary text and terms cannot be bounded by atom count
alone. The helper adds discovery/index metadata from actual layouts, including
conservative buffer overlap. Actual allocation capacity is checked and may
refuse. Each phase has a fresh 100-million-unit work ceiling. Value construction uses `ValueLimits`.
Every library, work-limit and output error terminates the probe; there is no
retry or fallback. A partial CSV is evidence of a prefix, not a completed case.
The fixed loop counts and assertions are identical in both modes. Actual vector
capacities can vary with the allocator even in `--check` mode.

The CSV has seven phase rows per round: append, owned and borrowed duplicate
lookup, borrowed misses, pending snapshot, commit and committed snapshot. `items`
counts atoms/queries, or returned IDs for snapshots. `interner_calls` counts
entry plus insertion calls for append/duplicates, entry calls for misses, and
one call for each snapshot or commit. Accessors, `split` and `AtomPattern::key`
construction are excluded from that call count. `work` counts actual callback
admissions,
including comparison descriptors/text, index operations, conservative relocation
and publication work; it is not an instruction count or a count of cloned bytes.

Each interval includes its loop, borrowed-key construction where used, and the
checks within that phase. Fixture construction, reference sorting, receipt
capacity checks, CSV publication and final catalog publication are outside those
intervals. Cases and phases run once in a fixed order, so the output does not establish statistically
replicated latency or a comparison with an earlier representation.

`interner_retained_bytes` reports named canonical payload and index capacity,
snapshot directories, discovery maps, owner headers and retained scratch;
`interner_peak_bytes` is the cumulative greatest live or actual conservative
reservation-overlap envelope for that case, including temporary ordered IDs.
`ordered_ids_bytes` reports the returned vector header plus its actual capacity,
and is zero outside snapshot rows. That vector is dropped before the next
operation. Peak receipts are repeated observations, not additive phase costs.
Canonical text and term payload is included. The bounded local lookup direction
record, allocator bookkeeping, Arc counters, caller frames, fixture/reference
owners and other process state are excluded. These measures are neither RSS nor
a total memory ceiling. Preserve source, compiler, executable, command, exit and
host identities with measured output; ordinary grounding and solving still need
their own complete-result and resource comparisons.
