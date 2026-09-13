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
independent derived typed equality over its original or canonical selection.
No hashes, projected strings or row counts stand in for the identity assertions.

The CSV reports index construction work/time, model construction time, repeated
lookup work/time, selected-row visits, and the reference's scan visits/time.
Lookup work includes probes, visited typed descriptors/text bytes, sequence ends
and consumer row visits. Reference visits use a different unit and must not be
divided into lookup work to claim a speed ratio. Model construction uses the
existing constructor and its standard sort; its logical work is not measured by
this probe. Fixture/query/reference preparation and CSV publication are outside
the reported timings. Measured loops retain correctness assertions, use a fixed
route order and run once per case, so they are bounded scaling observations,
not an isolated kernel-speed or statistically replicated application result.

Storage columns are actual index retained/preparation-peak integer capacities,
source atom-vector cell capacity, model position capacity and the model's
conservative canonical payload admission measure. They exclude allocator and
Arc overhead, fixture query/reference storage and other process state. The atom
cell column excludes nested values; canonical payload bytes are a different
measure and must not be added to it as physical storage. The shared index setup
receipts are repeated on the two route rows and should be counted once per case.
They do not imply that `Model::lookup` constructs or retains an index. None of
these columns is peak RSS or a process memory limit.

Retain source, compiler, executable, command, exit and host identities alongside
any measured CSV. Ordinary objective solving requires its own matched full-model
and cost comparison, including setup, search, output and completion.
