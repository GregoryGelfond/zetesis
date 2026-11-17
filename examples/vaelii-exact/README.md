# Exact Vaelii test programs

These programs are **exact**: each is a statement vector that Vaelii's own test
suite hands to clasp or clingo, taken from
[`vaelii/vaelii`](https://github.com/vaelii/vaelii)
`test/vaelii/asp_aspif_test.clj` at commit `03c728ce8a66`. That repository
holds no `.lp` sources (Vaelii builds ASPIF in Clojure), so `gen.py`
transcribes each ASPIF statement into the equivalent ground rule: a shown atom
keeps its shown name, an unshown atom becomes `x<id>`, and each `#minimize`
element keeps its literal id as a term so that equal weights stay distinct, as
they do in ASPIF. The vertex covers reproduce the test's `java.util.Random(7)`
draws exactly.

| File | Vaelii test | Task | Contract (checked with clingo 5) |
|---|---|---|---|
| `fact.lp` | `a-fact-appears-in-the-answer-set` | enumeration | 1 answer set |
| `unsat.lp` | `an-unsatisfiable-program-reports-unsat-rather-than-throwing` | UNSAT proof | unsat |
| `free-choices.lp` | agreement program "two free choices" | enumeration | 4 answer sets |
| `integrity.lp` | `an-integrity-constraint-forces-the-other-branch` | all optimal ties | 1 at cost 0 |
| `two-level.lp` | `higher-minimize-priority-dominates` | all optimal ties | 1 at cost 0, 1 |
| `weight-constraint.lp` | `a-weight-constraint-bounds-a-cardinality` | all optimal ties | 1 at cost 0 |
| `pigeonhole6.lp` | `(pigeonhole 6)` | UNSAT proof | unsat |
| `pigeonhole9.lp` | `(pigeonhole 9)` | UNSAT proof | unsat |
| `cover60.lp` | `(weighted-cover 60)` | all optimal ties | 1 at cost 149 |
| `cover90.lp` | `(weighted-cover 90)` | all optimal ties | 1 at cost 260 |
| `pigeonhole13.lp` | `(pigeonhole 13)` | UNSAT proof | unsat (clingo does not finish in 100 s) |

`manifest.json` places the first nine in the maintained benchmark corpus.
`cover90.lp` and `pigeonhole13.lp` exceed the corpus's ten-second child
timeout, so they are kept in the separately selected
[`vaelii-slow`](../vaelii-slow/manifest.json) supplement instead.
