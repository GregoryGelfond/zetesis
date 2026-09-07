# Sources and limits

Inspected 2026-09-06. Public sources below were read through the web tool; local
sources were read without modification. No private code or manuscript content
was uploaded. The architecture recommendations, API sketch, counterexamples,
and checkpoint plan are proposed zetesis design, not quotations or claims of
existing implementation.

## Primary public sources

1. [Cabalar, Fandinno, Schaub and Wanko, *On the Semantics of Hybrid ASP Systems
   Based on Clingo*, Algorithms 16(4), 185 (2023)](https://www.cs.uni-potsdam.de/wv/publications/DBLP_journals/algorithms/CabalarFSW23.pdf).
   Read the theory-atom distinctions and structured witness semantics, especially
   §§2–5. This provides semantic reference points, not refinement of zetesis's
   broader Ferraris compiler or arbitrary theory language. Its §2 excludes founded
   theory atoms from rule bodies and gives a single atom/falsum head rule form.
   The generic founded-cycle counterexample in our report deliberately exceeds
   that source profile and is not asserted to be valid clingcon syntax.
2. [Cabalar et al., *Bound-Founded Semantics for Answer Set Programming with
   Difference Constraints: Preliminary Report*, arXiv:2607.21201v1 (July 2026)](https://arxiv.org/html/2607.21201v1).
   Read the distinctions among clingcon/flingo/clingo[DL] numerical witnesses and
   the qualified implementation correspondence. Treated as preliminary research,
   not a replacement for version-pinned executable conformance.
3. [clingo 5.8 C API: Theory Propagation](https://potassco.org/clingo/c-api/5.8/group__Propagator.html).
   Read typed roles, initialization, partial assignments, checks and clause scopes.
   An adapter reference; it does not require zetesis to copy the search engine.
4. [clingo 5.8 theory API and implementation](https://potassco.org/clingo/python-api/5.8/clingo/theory.html).
   Read the clingcon example, C extension function contract, and registration,
   rewrite, preparation, model notification and assignment retrieval paths.
   This is interface evidence; Python is not proposed as a runtime dependency.
5. [clingcon repository](https://github.com/potassco/clingcon) and
   [release notes](https://github.com/potassco/clingcon/blob/master/CHANGES.md).
   Current project purpose and versioned compatibility dimensions. The official
   [landing page](https://potassco.org/clingcon/) advertises older 3.x material as
   well, so it was not used as an exhaustive current feature list.
6. [clingo-dl repository](https://github.com/potassco/clingo-dl).
   Establishes the upstream extension's clingo theory/C++ interface context;
   numerical semantics are treated separately above.

The journal landing page returned HTTP 429; its author's PDF copy was readable.
Several GitHub raw/blob source URLs returned cache misses or unavailable content.
No internal clingcon propagation algorithm is claimed to have been fully audited.
The installed public C header and Python wrapper were available and provided
direct source evidence of the clingo object dependencies.

## Local sources

The themelios design of record is `docs/design/solve.md`, SHA-256
`c1a0360242ea123e24dc121678d685386b52a93f72fe25d65e07acda8eef8513`,
identical at the current read-only checkout
`c4d4045dd0e248ce1922dfa6aa3bfa968b7c6d1d` and zetesis's dependency pin
`87c11a3f2b72b81a12fd53226941fdf95e7294d3`. Its §8 is explicitly an
interface shape governed by the DL/CP/LP litmus, not a frozen implementation.
The public solve tier is absent from the inspected workspace crate roster.

The report also reads the existing owned theory algebra, parser, rewriter,
analysis safety boundary, and zetesis's immutable formula/proposal/check/commit
code and design notes. The reviewed source hashes are in `reviewed-inputs.json`.
Those hashes are a local inspection record, not a build or release attestation.
Concurrent zetesis implementation work may change those files after inspection.

Installed clingcon evidence comes from
`<clingcon-package>/include/clingcon.h`, its two Python wrapper
files, and the `clingcon-5.2.1-py314h93ecee7_2` package metadata. No binary was
executed. Version identification is from those local files, not an asserted
fresh `--version` run. The prospective corpus inventory is separately hashed;
its embedded expected outcomes are original source contracts, not newly measured
zetesis or clingcon results.

## Claims expressly left open

No runtime theory support, source semantic correspondence proof, Lean theorem,
device qualification, performance result, complete clingcon feature matrix,
or direct binary-plugin compatibility was established by this work. The
existing 527-law checkpoint concerns the ordinary solver and cannot be cited as
proof of these new extension obligations.
