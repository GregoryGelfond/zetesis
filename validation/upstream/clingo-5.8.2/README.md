# Exact upstream clingo assertions

For normal fixture consumption, start with the self-contained `curated/`
directory and the Rust `zetesis_validation::curated` API. After installation:

```sh
zetesis-corpus verify validation/upstream/clingo-5.8.2/curated
```

This verifies 24 exact ASP files, their provenance and license, and 73 recorded
full-model occurrences without reading upstream C++ files or running a solver.
The [validation library guide](../../../crates/zetesis-validation/README.md#compose-capture-contracts-and-publication)
describes the verification boundary. Fixture integrity alone is not model parity;
the comparison campaign below checks live solvers.

These objective-free assertions come from clingo v5.8.2, commit
`a99ffb2a58293c68b28fcc283a1d1c9ccad900fe`. The immutable
[`curated/manifest.json`](curated/manifest.json) retains public revision-qualified
links and SHA-256 identities for the two original test files and their helper,
verbatim copyright notices, assertion identities, original byte/line spans,
exact assertion excerpts, helper arguments and expected helper output.
The [MIT license](curated/LICENSE.md) accompanies the decoded ASP files.

`zetesis_validation::curated::open` verifies the manifest, license and source
hashes. Its private bounded excerpt decoder also checks that each preserved
assertion decodes to the exact ASP bytes and helper contract. Adjacent string
literals preserve spaces and newlines; unsupported spellings fail explicitly.
This is a checker for the retained assertion format, not a general C++ importer.
The complete model expectations are retained independently of the helper's
selected display and compared with separately captured reference envelopes.
Original coordinates and whole-file identities locate evidence that can be
checked against the linked upstream revision; normal verification does not
fetch or revalidate those whole files.

The upstream helper enumerates all models, then filters printed atoms by a list
of string prefixes and sorts models without deduplication. This collection
excludes assertions with optimization bounds and sources containing `#show`.
The complete clingo model multiset is compared first; the original helper's
filtered expectation is checked separately. Expected upstream diagnostics are
preserved, but their exact text is not a native zetesis diagnostic contract.
Three selected assertions are competition-derived encodings from `aspcomp13.cc`;
they do not represent the complete ASP competition suite.

Portable tests parse and raise all 24 sources with pinned themelios, then replay
the native cases through automatic and explicit countermodel CLI routes with
exhausted enumeration and complete full-model identities. An admission refusal
is neither UNSAT nor a parity pass. Native admission policy does not rewrite
the immutable source or expected-model contracts.

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
