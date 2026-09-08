# Fedora 44 / Radeon 780M qualification plan

Prepared 2026-09-08 from source
`344c73799e5698b5e519e0f4452772141469bf6d`; lazy-transport qualification is incomplete.
This is a preparation plan, not a laptop execution or qualification record.
The campaign must identify its frozen source revision and binaries.

## Feasibility and minimum prerequisites

The reported Ryzen 7 7840U contains Radeon 780M graphics according to
[AMD's product specification](https://www.amd.com/en/products/processors/laptop/ryzen/7000-series/amd-ryzen-7-7840u.html).
This is a useful second CPU/GPU architecture for zetesis. The reported 96 GB is
system RAM, not 96 GB of dedicated GPU memory. Actual Vulkan memory heaps,
budgets, buffer limits and allocation failures must be observed on the laptop.

The expected Linux path is Rust/wgpu → Vulkan loader → Mesa RADV → the kernel's
amdgpu driver. [Mesa documents RADV](https://docs.mesa3d.org/drivers/radv.html) as
its Vulkan driver for AMD GCN/RDNA GPUs and describes this userspace/kernel split.
This is a feasibility inference; successful initialization and exact device tests
remain necessary. No ROCm, HIP, CUDA, proprietary AMD driver or GPU SDK is required
by zetesis's wgpu Vulkan implementation.

Fedora 44 has official packages for the [Mesa Vulkan drivers](https://packages.fedoraproject.org/pkgs/mesa/mesa-vulkan-drivers/),
[Vulkan loader](https://packages.fedoraproject.org/pkgs/vulkan-loader/vulkan-loader/)
and [Vulkan tools, including vulkaninfo](https://packages.fedoraproject.org/pkgs/vulkan-tools/vulkan-tools/fedora-44.html).
Use the distribution packages and record the installed versions rather than
pinning this plan to a web page's latest update:

```sh
sudo dnf install mesa-vulkan-drivers vulkan-loader vulkan-tools
vulkaninfo --summary
```

A native source build additionally needs a linker (Fedora's `gcc` package), Git,
the pinned Rust 1.97.1 toolchain, and initial read access to zetesis and the pinned
private themelios dependency. `./scripts/install.sh` builds the five installed
commands for the actual Rust host target; on this laptop that should be an x86_64
Linux target, to be recorded rather than assumed. macOS binaries cannot be copied
across as Linux executables. An offline first build needs a separately prepared
complete dependency cache; a zetesis Git bundle alone is insufficient. No sibling
estate checkout or dependency-pin change is needed.

At runtime, installed commands require neither Cargo nor Rust. An accessible
amdgpu render device and functioning Vulkan loader/driver are required for GPU
work. Run as the normal logged-in user; missing device access is a setup failure,
not a reason to run solver qualification as root or to admit a software adapter.
Clingo is a separate native executable used only for reference comparison. Record
its exact version and SHA-256; match the reference version across hosts for a
cross-host comparison, or label the difference.

## What can run with the existing interfaces

`zetesis` already accepts `--backend vulkan`. Explicit selection requires a
physical GPU and never changes to a CPU or different graphics API on failure.
The four library oracles expose `new_selected(..., GpuSelection)` with Vulkan;
no shader rewrite is needed merely to request that API. This does not establish
that every existing kernel passes on RADV.

`zetesis devices` reports compiled native APIs and each adapter's actual name,
API, vendor/device IDs, category, driver strings and static-profile capability
assessment. Its successful exit does **not** establish GPU availability: an empty
inventory or CPU-only build is reportable without a failing exit. Inspect the
content, then require a real explicit solve. Its capability summary is also not
an exhaustive numeric report for all four oracle profiles; retain full
`vulkaninfo` output alongside it.

Existing smoke commands, run from the frozen checkout with the new installed
Linux binary:

```sh
zetesis devices
zetesis examples/network-repair.lp --backend vulkan --grounder lazy \
  --oracle closure --models 0 --json --stats --color never
zetesis examples/network-repair.lp --backend vulkan --grounder eager \
  --oracle closure --models 0 --json --stats --color never
```

The network example has exactly two complete models. Require exhausted coverage,
those exact full models, explicit Vulkan metadata and positive actual device
activity where that route exposes counters. Keep stdout, stderr and exit code.
Do not treat the requested backend string alone as evidence of execution.

The existing full-corpus validator already accepts Vulkan, verifies the authored
actual API, and reconciles submitted candidates, GPU work and exact CPU residual
completion on its countermodel route:

```sh
zetesis-validate --repo . --zetesis "$HOME/.local/bin/zetesis" \
  --clingo /absolute/path/to/clingo --native-backend vulkan \
  --native-oracle countermodel --native-batch-size 64 \
  --native-completion-workers 4 --native-stats \
  --timeout-ms 30000 --max-output-bytes 33554432 \
  --report /absolute/new/report-directory/corpus-vulkan.json
```

Run the identical command with `--native-backend cpu` and a distinct report path
for the CPU reference series. These are one-pass qualification/observation
campaigns, not repeated benchmarks. The command has **no grounder option**;
forcing countermodel uses current eager formula admission. It covers the complete
94-case clean non-clingcon corpus, including all six original queens encodings.
A pass compares final optimal displays, symbol/model multiplicities, complete
counts, costs and supported manifest contracts. It does not reconstruct hidden
clingo atoms and therefore is not an unrestricted full-hidden-model parity claim.
Some outer-UNSAT cases legitimately need no membership dispatch; the report keeps
that distinction and requires actual GPU work somewhere in the physical campaign.

The platform-independent static hardware test is also already usable:

```sh
cargo test --locked -p zetesis-wgpu --test hardware \
  exact_static_oracle_matches_independent_cpu_closures -- --ignored --nocapture
```

It hard-fails missing physical hardware, compares complete closures with the
independent CPU oracle and prints the selected API. It uses automatic selection;
only a retained result actually naming Vulkan can support a Vulkan claim. This
single test does not qualify formula, lazy or tight execution.

## Gaps before a complete, correctly labelled campaign

| Boundary | Current gap | Bounded extension |
| --- | --- | --- |
| Repeated end-to-end matrix | `zetesis-perf` profiles and `selected::Backend` admit CPU/Metal only; matrix telemetry explicitly parses Metal | Add explicit Vulkan eager/lazy profiles, actual Vulkan route validation and negative controls; keep old Metal identities/results intact |
| Matched primitive benchmarks | `zetesis-bench` static, formula, lazy and tight use CPU/Metal configuration; lazy/tight routes explicitly name Metal | Parameterize requested native API and retain actual API/adapter in each device result; introduce Vulkan-specific or backend-neutral truthful route identities rather than writing Vulkan data under a Metal label |
| Physical regression tests | Formula, tight, lazy, the new transport tests and CLI physical groups construct Metal explicitly | Factor fixture assertions from selected-adapter creation; add an explicit hard-failing Vulkan qualification selection and check actual API/category. Preserve Metal coverage |
| Hardware evidence | Inventory gives a static eligibility summary; no single installed command produces the entire qualification archive | Compose the existing commands into a qualification archive preserving every log, exit and report |

Source locations: `crates/zetesis-cli/src/{options,devices}.rs`,
`crates/zetesis-wgpu/src/selection.rs`,
`crates/zetesis-validation/src/{main,execution,runner,selected}.rs`,
`crates/zetesis-validation/src/bin/zetesis-perf.rs`,
`crates/zetesis-validation/src/performance/matrix/telemetry.rs`,
`crates/zetesis-experiments/src/{measurement,lazy_measurement,tight_measurement}`,
and `crates/zetesis-wgpu/tests/hardware*.rs`.

Do not run `--backend metal` on Fedora or force a software/Noop adapter to make
these checks appear successful. The existing constructors already support a
proper Vulkan adaptation; unsupported benchmark enums are measurement-tool gaps.

## One manually transferable result bundle

Prepare one immutable source revision, then build and freeze native Linux binaries
before the quiet run. The returned archive should contain:

- **Identity:** Git commit/clean status; Cargo.lock and corpus-manifest hashes;
  exact commands, argument order, limits and RNG seed; Rust/Lean/reference versions;
  binary SHA-256 values before and after every campaign. Linux/macOS binary hashes
  naturally differ, while source revision and comparison protocol should match.
- **Host:** `/etc/os-release`, `uname -a`, `lscpu`, memory summary, installed Mesa/
  loader/tools package versions, complete `vulkaninfo` and `zetesis devices`
  stdout/stderr/exit. Record relevant GPU-driver overrides, power mode, AC/battery
  state and thermal conditions; do not dump the entire environment or credentials.
- **Qualification:** portable gate and explicit Vulkan primitive/CLI test logs,
  both 94-case corpus reports, and final completion/route summaries. Retain
  refusals, timeouts, capture/resource limits and incomplete prefixes unchanged.
- **Measurements:** separate CPU/Vulkan results with fixed rotated schedules,
  explicit workers and identical candidates or source/output contracts. Keep
  instrumentation, GPU setup/transport/readback and CPU residual boundaries
  visible. Collect CPU and Vulkan serially under controlled measurement conditions.
- **Archive integrity:** a relative-path SHA-256 manifest, compressed raw reports
  without lossy conversion, and a SHA-256 for the single transferred archive.

After the typed matrix extension, use all four CPU/Vulkan × eager/lazy profiles.
Keep every requested cell even when refused. Current corpus-wide explicit lazy
formula execution remains refused; relational network/matched-lazy inputs supply
actual lazy CPU/GPU evidence. A completed census containing refused lazy cells is
not an all-configurations parity pass. Same-host native CPU/Vulkan wall times are
more directly comparable than native versus clingo timings: native JSON retains
full atoms while clingo exposes selected output, so instrumentation/output volume
is asymmetric. Primitive synthetic-batch timings must stay separate from ordinary
solver timings and from cross-host comparisons.

The recommended first deliverable is one laptop archive proving that Vulkan
actually executes correct work, followed by repeated matched CPU/Vulkan data.
Neither the hardware specification, existing Metal proofs/tests nor a successful
inventory command substitutes for that evidence.
