# Exact upstream clingo assertions

These 24 objective-free semantic assertions come from clingo v5.8.2, commit
`a99ffb2a58293c68b28fcc283a1d1c9ccad900fe`. The original C++ files and solver
helper are preserved under `originals/`, with their MIT copyright notices and
the upstream license. They were copied read-only from the local clingo checkout
under archeion. They are third-party material, not newly authored zetesis tests.

`cases.jsonl` retains the assertion identity, original byte/line span, exact C++
assertion, decoded ASP bytes, source hashes, helper arguments, expected helper
output and a separately captured complete clingo model reference. Source literal
concatenation preserves every space and newline. `scripts/upstream_assertions.py`
reconstructs that metadata from the preserved originals; its small decoder
explicitly refuses unsupported C++ forms and escape sequences.

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
python3 scripts/compare-upstream.py --zetesis zetesis --clingo clingo \
  --report target/upstream-parity.json
```

Each subprocess has a five-second deadline and a combined 1,114,112-byte capture
ceiling. Native resource limits stay at CLI defaults. The report retains partial
process output, explicit incomplete/failure classifications and source/binary
hashes. Passing means the recorded admission/refusal boundary was reproduced;
it does not mean all 24 cases were admitted or that full clingo compatibility
has been achieved. This campaign is a regression/discovery tool, not a benchmark.
