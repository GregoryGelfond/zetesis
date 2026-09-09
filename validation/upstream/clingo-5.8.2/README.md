# Exact upstream clingo assertions

For normal fixture consumption, start with the self-contained `curated/`
directory and the Rust `zetesis_validation::curated` API. After installation:

```sh
zetesis-corpus verify validation/upstream/clingo-5.8.2/curated
```

This verifies 24 exact ASP files, the sealed provenance/license and 73 recorded
full-model occurrences without reading C++ or running a solver. The
[validation library guide](../../../crates/zetesis-validation/README.md#compose-capture-contracts-and-publication)
describes the import and verification boundary. Fixture integrity alone
is not model parity; the Rust comparison campaign below checks live solvers.

These 24 objective-free semantic assertions come from clingo v5.8.2, commit
`a99ffb2a58293c68b28fcc283a1d1c9ccad900fe`. The original C++ files and solver
helper are preserved under `originals/`, with their MIT copyright notices and
the upstream license. They are third-party material, not newly authored zetesis
tests.

`cases.jsonl` retains the assertion identity, original byte/line span, exact C++
assertion, decoded ASP bytes, source hashes, helper arguments, expected helper
output and a separately captured complete clingo model reference. Source literal
concatenation preserves every space and newline. The Rust
`zetesis_validation::curated::import_legacy` operation reconstructs that metadata
from the preserved originals; its bounded literal decoder explicitly refuses
unsupported C++ forms and escape sequences. Normal verification and comparison
read only the independently curated data.

The upstream helper enumerates all models, then filters printed atoms by a list
of string prefixes and sorts models without deduplication. This collection
excludes assertions with optimization bounds and sources containing `#show`.
The complete clingo model multiset is compared first; the original helper's
filtered expectation is checked separately. Expected upstream diagnostics are
preserved, but their exact text is not a native zetesis diagnostic contract.
Three selected assertions are competition-derived encodings from `aspcomp13.cc`;
they do not represent the complete ASP competition suite.

The `native` field records the current admitted or refused boundary. A refusal
is neither UNSAT nor a parity pass. Portable tests independently parse and raise
all 24 sources with pinned themelios, then replay every admitted source through
both automatic and explicit countermodel CLI routes, requiring exhausted
enumeration and complete full-model identities. Changing a refusal into an
admission requires updating this expectation after external verification.

Run the external semantic campaign from the repository:

```sh
zetesis-corpus compare validation/upstream/clingo-5.8.2/curated \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report target/upstream-parity.json
```

Each subprocess has a five-second deadline and a combined 1,114,112-byte capture
ceiling, with a separate one-second cleanup interval and a combined campaign
capture ceiling. Native CPU/eager/automatic-oracle requests are explicit in the
report; other solver budgets retain the sealed executable's defaults.
The report retains raw byte prefixes, typed failure classifications, requested
limits and source/binary seals checked before and after execution. Publication
never replaces an existing path. Passing requires all 24 cases to complete and
match every full-model occurrence and original helper contract; an admission
refusal cannot pass. Native JSON supplies full atom identities independently of
shown output. This is a regression campaign, not a benchmark or a claim of full
clingo language compatibility. It does not qualify physical GPU execution.
