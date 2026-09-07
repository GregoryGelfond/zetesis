# Regression corpus

`kr-domains/` contains the **108 byte-exact required source files** at revision
`38f0660ded448ed268c5a68759ceb0e2840dd497` of
[GregoryGelfond/kr-domains](https://github.com/GregoryGelfond/kr-domains).
Relative paths are preserved so every original include resolves unchanged.
The 94 runnable entry points and every expected SHA-256 digest are in the
[target manifest](../../docs/verification/kr-domains-target-manifest.json).
The other 47 source files are excluded because they are clingcon encodings or
include those encodings. Their exclusion evidence remains in the manifest.

The snapshot retains its original [MIT license](kr-domains/LICENSE), copyright
Gregory Gelfond. Do not edit these sources to make the solver accept them. An
upstream refresh must update the revision, hashes, include graph and contracts
together; a minimized solver regression belongs in a separate authored fixture.

Native loading tests and full solving comparisons are distinct. A successful
parse or an expected unsupported-feature diagnostic does not satisfy a full
case. Full compatibility requires a completed native solve and parity with the
external oracle, including the relevant optimization and output contracts.
The initial snapshot is vendored to make these regressions available offline;
clingo is installed separately and is never a runtime solver dependency.
