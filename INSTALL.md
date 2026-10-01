# Installing zetesis

zetesis builds from a checkout of this repository with the Rust toolchain it
pins. Installed commands need neither Rust nor Cargo at runtime.

## What gets installed

| Tool | What it is for |
| --- | --- |
| `zetesis` | The answer-set solver: `zetesis solve program.lp`. |
| `zetesis-bench` | Times the installed `zetesis` on the example programs in this repository, against clingo when there is one, and compares saved runs, so you can check performance claims yourself. |

`zetesis bench` is no longer a command: benchmarking is the separate
`zetesis-bench` tool. `zetesis-bench` runs `zetesis` and clingo as separate
programs. It uses the `zetesis` installed beside it, or the first one on
`PATH`, and compares it with clingo, found on `PATH` or named with `--clingo`;
without clingo it times `zetesis` alone and checks its answers against the ones
each example records. Run it from the root of this checkout, where its example
programs live.

## Install with the script

```sh
rustup toolchain install 1.97.1 --profile minimal
./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
zetesis --help
```

The script builds locked release binaries and installs both tools into
`~/.local/bin`; pass a directory to install elsewhere. The first build needs
GitHub read access for the pinned themelios dependency.

The default build includes GPU support: Metal on macOS, Vulkan elsewhere.
`./scripts/install.sh --cpu-only` builds zetesis without it, for machines with
no supported GPU or no graphics drivers; that build runs on the CPU only and
refuses `--backend gpu`.

## Install with Cargo

```sh
cargo install --locked --path crates/zetesis-cli
cargo install --locked --path crates/zetesis-bench
```

Add `--no-default-features` to the first command for the CPU-only build.

## Check the build

`zetesis devices` lists the GPU APIs compiled into this build, the adapters it
finds and the one `--backend gpu` would use. The CPU backend, the default, works
everywhere. On macOS the GPU backend uses Metal; on Linux it uses Vulkan, which
needs a Vulkan driver. Vulkan has not yet been qualified on physical hardware.

## Contributing

The installed `zetesis test` commands check corpus, scalability and backend
contracts. Corpus and scalability checks read fixtures from the checkout.
Repository maintenance and qualification scripts, and the separate
`zetesis-corpus` and `zetesis-validate` developer tools, run from the checkout
and are not installed by the script; see [CONTRIBUTING.md](CONTRIBUTING.md).
