# Wide Mastermind

`68/mm-68-examples.lp` is the unchanged entry program. It includes `mm-68.lp`,
beside it, and `../mm-base.lp`. The adjacent manifest records the exact original
bytes and include spellings. These supplied programs are preserved fixtures,
kept byte for byte with their comments and commented-out alternatives.

The program searches Mastermind codes of six positions over eight colours. One
scored guess is active: `(1, 2, 3, 3, 4, 5)`, scored `(0, 3)`, no colour in its
position and three colours elsewhere. Its expected result is the complete family
of the 45,832 codes consistent with that score, each shown as one `next_guess/6`
atom. Each campaign requires a fresh clingo comparison; the recorded count alone
does not establish model identity or completeness.

It is a different program from `examples/scalability/mastermind.lp`, the
maintained scalability workload. Run at eight colours, as the scalability
population also runs it, that program is a different encoding of the same
puzzle, with the same 45,832 codes. The
[contributor guide](../../CONTRIBUTING.md) names this program, by path, as the
workload on which the tight kernel's zero-fill decision was measured. The bundle
is not part of the benchmark corpus.

From the repository root:

```sh
zetesis-bench run --workloads examples/wide-mastermind/manifest.json \
  --clingo /absolute/path/to/clingo
```

Normal campaign resource and capture limits apply. The
[benchmark guide](../../docs/book/reference/benchmarking.md) explains the
full-model comparison, qualification evidence and refusal behavior.
