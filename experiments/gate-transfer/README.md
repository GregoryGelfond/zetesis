# Finite GPU gate-transfer experiment

This isolated, dependency-free Rust crate specifies an alternative to the
eight-row relation scan in `zetesis-wgpu/src/formula.wgsl::gate`. It is outside
the production workspace and has no solver, GPU, themelios or clingo dependency.
The proposed shader patch is **unapplied**. The six Rust property tests, strict
all-target pedantic Clippy, rustfmt check and isolated Lean law passed in the
recorded experiment. Exact commands/logs are retained by the parent campaign.
The finite vectors and source identities are retained here; none is a performance
result or a physical GPU qualification.

The contract uses two-bit domains: 0 is empty, 1 permits false, 2 permits true,
and 3 permits either value. A gate returns each position's supported values.
The caller intersects these supports with its current physical domain slots.
Left, right and output may alias; the five equality partitions are represented
explicitly. All **960 cases** are covered: three connectives × five partitions
× 64 domain triples. The enumeration includes different observed masks at
aliased positions, because concurrent atomic loads can observe intervening
monotone narrowing. It does not assume a coherent snapshot.

| Implementation | Mechanism | Independence and limits |
| --- | --- | --- |
| `reference` | Enumerates eight assignments, tests Boolean connective, input masks and physical alias consistency, then projects supports | Mathematical transcription of the current gate; independent of hexadecimal masks |
| `bitwise` | Intersects eight-bit row sets and projects selected false/true row masks | Candidate for a direct WGSL transfer; no lookup or per-row loop |
| `lookup` | Indexes a compile-time 960-byte Rust table generated from `reference` | Alternative to benchmark later; a portable WGSL `u32` storage table would use at least 3,840 payload bytes, not 960 |

Row index is `x + 2*y + 4*z`. The connective masks are And `0x87`, Or `0xe1`
and Implies `0xd2`. False/true position masks are `(0x55,0xaa)`, `(0x33,0xcc)`
and `(0x0f,0xf0)`. Equality masks for left=right, left=output and right=output
are `0x99`, `0xa5` and `0xc3`; all equal gives `0x81`. These masks encode
relation rows, not candidate bit planes or physical domain storage.

The tests compare every transfer, verify contraction and idempotence, and check
10,935 ordered subset-domain pairs for monotonicity. A separate nested Boolean
enumeration checks exact support for each value at each position. Physical-slot
tests cover every partition, invalid tags/masks and mixed aliased observations.
A stale-snapshot test intersects the returned supports into smaller current
physical domains and checks that every current satisfying completion survives.
The Lean file proves finite mathematical equality for the same 960 transfers;
its audited transitive axiom dependency is `propext`. It is isolated from the
production theorem inventory and checked with Lean 4.33.1, auto-implicit arguments
disabled and warnings as errors. Rust used the pinned 1.97.1 toolchain.

The repository's `./scripts/check.sh portable` explicitly includes this standalone
Rust package's formatting, all-target tests, doctests, pedantic Clippy and strict
Rustdoc. Its separate Cargo workspace retains the estate lint policy; the main
workspace's coverage percentage remains a distinct measurement. The isolated
Lean experiment still uses the separate commands documented here.

```sh
cargo test --manifest-path experiments/gate-transfer/Cargo.toml --offline
cargo clippy --manifest-path experiments/gate-transfer/Cargo.toml \
  --all-targets --offline -- -D warnings
cargo run --manifest-path experiments/gate-transfer/Cargo.toml \
  --example vectors --offline > /tmp/gate-transfer-vectors.tsv
python3 experiments/gate-transfer/tools/prepare_patch.py
```

The final command is a read-only replay once artifacts exist. The preparation
mode `--write` creates missing artifacts and refuses to overwrite retained
files. It checks the original shader hash and substitutes only the gate transfer
function, retaining the rest of the shader exactly. The original and proposed
full shaders, unified unapplied patch, and their hashes are in `generated/`.
The script does not compile, validate or execute WGSL. Rust tests likewise do
not establish shader compilation or physical execution.

The replacement preserves three atomic loads and three intersections in order,
M-false connective suppression, original truth evaluation, barriers, strict
proper-subset constraints, epochs, result decoding, conservative sweep work and
round limits. It does not turn quiescence into acceptance or change a residual
into a completed result. There is no added optimization/objective condition in
the frozen query. Atomic interleaving safety still requires monotonically
shrinking domains for the same immutable query; a new candidate needs a fresh
epoch and reset. The tests and finite theorem are not a WGSL memory-model or
compiler-correctness proof.

Before promotion, validate the proposed shader through the pinned wgpu/Naga
stack, run physical alias/frozen-mask/limit/fault tests, and repeat original-source
parity with actual GPU telemetry. Benchmark current versus alternative shader
with identical limits and preserved scalar/Rayon baselines, cold/setup and warm
resident measurements separated. The compiler may already unroll parts of the
old loop; table indexing or extra bit operations may lose. No speed claim is
made without device evidence, and no GPU device is required for this finite
reference/specification checkpoint.

The repository integration test `zetesis-wgpu/tests/gate_experiment.rs` also
validates the original and proposed WGSL with pinned Naga 30.0.1, requiring no
optional capabilities and preserving declared bindings and entry points. This
is portable shader validation; production device code remains unchanged and
physical execution/performance remain unqualified for the alternative.
