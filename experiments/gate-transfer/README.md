# Finite gate-transfer reference

This standalone, dependency-free Rust package specifies exact Boolean
gate-support transfer. It compares enumeration, bitwise projection and table
lookup without depending on a solver, GPU, themelios or clingo.
It is outside the production Cargo workspace but participates in the repository's
portable gates.

The production formula oracle exposes
[`GateProjection`](../../crates/zetesis-wgpu/src/formula/projection.rs):
`Enumerated` remains the default; `Bitwise` is an explicit optional
implementation. The
[device constructors](../../crates/zetesis-wgpu/src/formula/device.rs) accept that
selection. The bitwise path assembles the unchanged shader scaffold with
[bitwise gate transfer](../../crates/zetesis-wgpu/src/formula/bitwise.wgsl).
The finite reference and retained proposal artifacts below are separate from
this runtime selection. Their presence does not establish physical qualification
or a performance benefit.

## Finite transfer contract

A domain uses two bits: 0 is empty, 1 permits false, 2 permits true and 3 permits
either value. A gate returns the supported values at each position; the caller
intersects them with the current physical domain slots.

Left, right and output can alias. Five equality partitions cover the possible
physical identities. The finite space has 960 transfers: three connectives,
five partitions and 64 domain triples. It includes different observed masks at
aliased positions because separate atomic loads can observe intervening
monotone narrowing. A coherent snapshot is not assumed.

| Implementation | Mechanism | Boundary |
|---|---|---|
| [`reference`](src/reference.rs) | Enumerates eight Boolean assignments, checks the connective, masks and aliases, then projects support | Independent of the bitwise hexadecimal masks. |
| [`bitwise`](src/lib.rs) | Intersects eight-bit row sets and projects position supports | No table lookup or per-row loop. |
| [`lookup`](src/lib.rs) | Uses a compile-time table generated from the reference | The Rust table contains 960 bytes; a corresponding WGSL `u32` storage table would need at least 3,840 payload bytes. |

The row index is `x + 2*y + 4*z`. And, Or and Implies masks are `0x87`,
`0xe1` and `0xd2`. False/true position masks are `(0x55,0xaa)`,
`(0x33,0xcc)` and `(0x0f,0xf0)`. Equality masks for left=right,
left=output and right=output are `0x99`, `0xa5` and `0xc3`;
all equal uses `0x81`. These encode relation rows, not candidate bit planes
or the physical domain-storage representation.

The finite [contract tests](tests/contract.rs) compare all transfers, contraction,
idempotence and the 10,935 ordered subset-domain pairs for monotonicity.
A separate nested Boolean enumeration checks each position's exact support.
Physical-slot models cover aliases, invalid tags/masks and mixed observations.
A stale-snapshot control intersects supports into smaller current domains and
checks that all current satisfying completions survive.

## Run the reference

From the repository root:

```sh
cargo test --manifest-path experiments/gate-transfer/Cargo.toml --offline
cargo clippy --manifest-path experiments/gate-transfer/Cargo.toml \
  --all-targets --all-features --offline -- -D warnings
cargo run --manifest-path experiments/gate-transfer/Cargo.toml \
  --example vectors --offline > target/gate-transfer-vectors.tsv
```

Create the output parent before redirecting the vector example if `target`
does not exist. `scripts/check.sh portable` also checks this package's formatting,
all-target tests, doctests, pedantic Clippy and strict rustdoc under the estate
lint policy. Main-workspace coverage remains a separate measurement.

[GateTransfer.lean](proofs/GateTransfer.lean) proves finite mathematical equality
for the same transfer space. It is an isolated proof outside the production
theorem inventory; its [recorded axiom output](evidence/lean-axioms.txt) lists
`propext`. From the repository root, use the pinned Lean environment:

```sh
(cd proofs && lake env lean -DautoImplicit=false -DwarningAsError=true \
  ../experiments/gate-transfer/proofs/GateTransfer.lean)
```

These semantic checks do not prove shader compilation, memory-model behavior,
convergence or physical execution.

## Retained shader proposal and provenance

[generated/patch-record.json](generated/patch-record.json) identifies the
retained original/proposed full shaders and
[unapplied patch artifact](generated/gate-transfer-unapplied.patch).
The filename describes that stored patch; it does not mean the optional
production `Bitwise` path is absent.

The explicit [preparation adapter](tools/prepare_patch.py) verifies the retained
shader identity and replaces only the gate-transfer function. Normal invocation
is a read-only replay when the artifacts exist:

```sh
python3 experiments/gate-transfer/tools/prepare_patch.py
```

Its `--write` mode creates missing artifacts and refuses to overwrite retained
files. The adapter does not compile or execute WGSL.
[evidence/source-fingerprints.json](evidence/source-fingerprints.json) records
the original experiment snapshot; it is not a current working-tree checksum
manifest. The retained [vectors](evidence/vectors.tsv) and
[independent vector review](evidence/vector-review.json) preserve the finite
reference evidence.

[gate_experiment.rs](../../crates/zetesis-wgpu/tests/gate_experiment.rs)
parses and validates the retained proposal with the pinned Naga dependency,
checking the declared bindings and entry points. It is portable validation of
those shader inputs, not a physical-device result.

## Runtime preservation obligations

A gate substitution must preserve the three atomic loads and intersections in
order, M-false suppression, original truth evaluation, barriers, strict
proper-subset constraints, epochs, result decoding, charged sweep work and round
limits. It must not turn quiescence into acceptance or a residual into a completed
membership decision.

Interleaving safety requires monotonically shrinking domains for one immutable
query. A new candidate needs a fresh epoch and reset. Exact local transfer alone
does not establish the surrounding propagation or reduct-completion protocol.

Physical qualification must exercise the actual selected implementation,
including aliases, frozen masks, limits and faults. Performance comparison must
keep candidates and limits matched, preserve scalar/Rayon baselines and separate
setup from resident calls. Enumeration may already be unrolled by a compiler;
extra bit operations or table access need not be faster. The
[experiment guide](../../crates/zetesis-experiments/README.md) describes the
maintained paired formula-projection measurement.
