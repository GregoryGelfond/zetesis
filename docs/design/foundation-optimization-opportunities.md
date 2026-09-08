# Foundation optimization opportunities

Research exploration against `1dc167b`, 2026-09-08. This is a ranked experiment
inventory, not new implementation or performance evidence. It complements the
[next tranche](next-language-and-execution-tranche.md).

The aggregate proposal suggests a useful architectural rule: retain a meaningful
operation until its execution plan is selected. Numeric aggregation, keyed
eligibility, domain restriction and fixed-point growth deserve explicit
contracts before choosing arrays, bitsets, indexes or kernels. Shared semantics
need not require one physical representation throughout the solver.

There is a directly relevant published foundation for native aggregates:
[Ferraris, *Answer Sets for Propositional Theories*, proposition 7(b)](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf)
characterizes aggregate reduct truth by original candidate truth and evaluation
of recursively frozen element formulas. That law is established literature;
zetesis still needs its own Lean formulation, whole-tuple coalescing bridge,
ordered-value and machine-integer contracts, and executable refinement. Native
aggregates are a separate planned stream; the seven opportunities below extend
the same idea elsewhere.

## What the baseline already supplies

[Current evidence](../verification/language-execution-tranche-20260908/README.md)
qualifies shared CPU source rounds, world masks, bounded GPU transport reuse,
scalar evaluation storage reuse and tight-program checking. Structural DAG
interning, indexed possible-support joins and independent-body factorization
also exist. These are foundations to extend, not newly discovered features.

The evidence constrains priorities. Queens variant 02 spends its diagnostic
solve time mainly generating candidates despite already requiring zero inner
countermodel queries. SEND retains substantial grounding cost. Shared CPU joins
help dense larger fixtures but lose elsewhere. The latest lazy Metal comparison
shows mixed timing; its best portable route wins all twelve fixtures. Reduced
logical storage is not measured process RSS, and these observations establish
no universal GPU advantage.

## 1. Candidate cardinality domains — near term

**Current boundary.** [Choice IR](../../crates/zetesis-themelios/src/formula_choice_ir.rs)
and [head aggregates](../../crates/zetesis-themelios/src/formula_head_aggregate.rs)
retain eligibility during lowering; the
[candidate cursor](../../crates/zetesis-sat/src/search/cursor.rs) traverses its
Boolean encoding. Preserve a bounded group operation describing selected,
excluded and undecided members, rather than recovering every group consequence
through small gates. A partition certificate can derive local bounds from a
fixed total and group exclusions.

**Mechanism and obligation.** CPU bitsets/population counts and GPU batched group
reductions could reject or narrow partial candidates earlier. This is a search
hypothesis, distinct from native aggregate membership evaluation. Prove that
all stable models survive each restriction; retain original permissions and
reduct checking. Extend [AggregateBounds](../../proofs/Zetesis/AggregateBounds.lean)
with the precise partition premise, not an assumption that every group is
independent.

**Experiment.** Measure decisions, restriction cost and complete models over
arbitrary finite partitions plus all six queens encodings. Overlaps, missing
members, conditional eligibility, inconsistent bounds and tiny groups are
required negative controls. Shared group identity depends on the aggregate IR;
repeated extraction from generated clauses would weaken the design.

## 2. Checked numeric binding tiles — near term

**Current boundary.** [Flat scalar evaluation](../../crates/zetesis-themelios/src/formula_support/evaluation.rs)
already reuses empty value storage. It still evaluates operations for each
binding through general `Value` objects.
[Domain analysis](../../crates/zetesis-domain/src/analysis.rs) currently supplies
finite symbol enclosures and uncertainty, not production arithmetic narrowing.

**Proposal.** An admitted numeric plan could evaluate bounded columns of bindings
using integer arrays, then apply comparison masks before constructing surviving
atoms/formulas. Repeated expression keys can be computed once for an immutable
binding prefix. CPU vectorization and Rayon tiles come first; GPU tiles require
enough independent rows to amortize transfer and dispatch.

**Obligation and experiment.** Preserve each row's value or located fault,
including checked intermediate overflow, division, remainder and evaluation
order; [ScalarArithmetic](../../proofs/Zetesis/ScalarArithmetic.lean) provides an
operation-agreement starting point. Any domain narrowing must cover every
eligible source binding. Measure evaluations, lookups, retained bytes and time
on SEND and generated arithmetic joins; include mixed symbols, changing bindings,
overflow, zero divisors and small batches. Avoid algebraic reassociation until
its failure behavior is proved equivalent. This extends storage reuse rather
than claiming it was absent.

This grounder work complements candidate pruning and native aggregate checking:
it reduces the cost of producing bindings before either stage. Distinguish three
experiments: faster evaluation of an unchanged binding stream, reuse of an
expression for an unchanged binding prefix, and sound elimination of bindings.
The last two change the amount of work and need separate coverage and fault-order
arguments. The existing checked numeric helpers also avoid temporary upstream
term-tree construction; a new tile should measure beyond that baseline.

Relevant primary work includes Kersten et al.'s
[compiled/vectorized query comparison](https://ir.cwi.nl/pub/28470/28470.pdf)
(VLDB 2018), which compares execution models with common algorithms and data
structures. Its arithmetic primitives and fused loops motivate a matched
numeric-binding experiment, not a prediction that either route always wins.
[GPUlog](https://arshovon.com/publications/ASPLOS_2025.pdf) (ASPLOS 2025) supplies
related ideas for dense relational kernels, indexed joins and semi-naive
iteration. Applying those techniques to zetesis's arithmetic grounding is an
architectural proposal; that paper does not establish ASP grounding or reduct
preservation. Keep runtime specialization separate from algebraic source
rewriting: preserving mathematical values alone does not preserve checked
intermediate overflow, undefined operations or located failures.

## 3. Keyed logical groups — near term, after shared aggregate identity

**Current boundary.** [Support rows](../../crates/zetesis-themelios/src/formula_support.rs)
use per-column indexes;
[body factorization](../../crates/zetesis-themelios/src/formula_factor.rs) already
coalesces independent witnesses without introducing semantic atoms.
[Objective evaluation](../../crates/zetesis-objective/src/evaluate.rs) has its own
contribution identity. A common bounded grouping operator could expose complete
key segments and their eligibility formulas to several consumers.

**Mechanism.** Contiguous segments allow CPU range traversal and GPU segmented
OR/AND reduction; logical aggregation may avoid many pointer/map visits and
intermediate formula nodes. Whole-key equality must remain authoritative even
when hashes or column indexes accelerate discovery. Sorting and extra copies
can lose on small or skewed groups.

**Obligation and experiment.** Prove that grouping preserves every witness and
its original/frozen eligibility, drawing on
[CountEligibility](../../proofs/Zetesis/CountEligibility.lean) and
[RuleFactorization](../../proofs/Zetesis/RuleFactorization.lean). Test shared and
unique keys, collisions, empty groups, alternate conditions and mixed term
orders against independently enumerated bindings. Start with one logical
consumer. Objective reuse follows the separate presence contract; sharing a
physical grouping operator does not make aggregate and objective keys identical.

## 4. Delta-relation rounds — medium term

**Current boundary.** The [CPU closure oracle](../../crates/zetesis-cpu/src/oracle.rs)
rebuilds predicate relations from the growing closure and visits templates each
round. It collects a delta but does not restrict rule inputs to changed rows.
The existing [GPU Datalog study](gpu-datalog-20260906.md) proposed this direction;
it remains an implementation opportunity.

**Proposal and mechanism.** Retain indexed full/delta relations within a frozen
candidate epoch. Evaluate all necessary changed-occurrence variants of each
positive rule, then merge/deduplicate a bounded next delta. This targets repeated
scanning and index construction. [GPUlog, sections 2–4](https://arshovon.com/publications/ASPLOS_2025.pdf)
provides primary evidence for semi-naive GPU relational execution and indexed
layouts, not Metal or zetesis speed predictions.

**Obligation and experiment.** Prove schedule completeness and equal least
closure using [LazyRounds](../../proofs/Zetesis/LazyRounds.lean). Initially retain
an independent final full coverage/constraint pass. Self-joins with several
changed positions, nullary rules, duplicate-heavy projection, deep chains and
dense frontiers expose omissions and overhead. Measure scans, rounds, index
bytes and end-to-end time separately. Rule dependencies and candidate/world
epoch isolation are prerequisites; a currently empty delta is insufficient
without complete scheduled work.

## 5. Cross-candidate truth tiles — medium term

**Current boundary.** Both [formula WGSL](../../crates/zetesis-wgpu/src/formula.wgsl)
and [tight WGSL](../../crates/zetesis-wgpu/src/tight/check.wgsl) compute original
DAG truth sequentially in lane zero of each candidate workgroup. Candidate atoms
are packed, but node truth uses one `u32` per candidate/node. The existing
[bitwise gate evaluator](../../crates/zetesis-wgpu/src/formula/bitwise.wgsl)
packs an eight-row Boolean relation; those bits are not candidate worlds.

**Proposal.** Prototype a tile whose node value is a bitset of candidate
occurrences. AND, OR and implication operate across those occurrences together;
retain separate frozen truth for each world. Alternatively evaluate independent
nodes by DAG level. These are competing experiments, not changes to combine
without attribution.

**Obligation and experiment.** Extend
[FerrarisMask](../../proofs/Zetesis/FerrarisMask.lean) and DAG correspondence with
lane extraction and tail-bit laws. Compare every original/frozen node value,
not only final verdicts, over arbitrary `M/J`, aliases and false implications.
Vary DAG depth/width and irregular batches. Packing may reduce truth payload
but transpose cost, synchronization and small batches may erase savings. Full
propagation and exact residual completion remain necessary; faster truth alone
does not establish stability.

## 6. World-mask-aware device scheduling — after transport tracing

**Current boundary.** [World masks](lazy-world-masks.md) prune source prefixes
with no common world. Every offered instance still reaches
[lazy WGSL](../../crates/zetesis-wgpu/src/lazy.wgsl), which independently tests it
against every candidate world. Transport retention already exists and has its
own planned replacement-reason experiment.

**Proposal.** Carry positive eligibility masks into bounded instance/world work
lists. Count, scan and materialize only eligible pairs, while initially retaining
all device positive/frozen checks. Potential gains are fewer tests, uploads or
atomics; mask payload, compaction and skew are the costs. This changes scheduling,
not transport allocation policy.

**Obligation and experiment.** Extend
[WorldMasks](../../proofs/Zetesis/WorldMasks.lean) to the concrete pair schedule:
every enabled pair appears, occurrences remain distinct and snapshot refresh is
exact. Test disjoint, dense and newly overlapping worlds, duplicate seeds,
shrinking chunks and exact limits. [WGSL synchronization and subgroup rules](https://www.w3.org/TR/WGSL/#synchronization-builtin-functions)
require explicit dispatch boundaries for global work and capability-aware
subgroup paths. No barrier supplies missing source coverage. Keep output decoding
and active-range contracts explicit; benchmark against the strongest shared CPU
route as well as independent Rayon.

## 7. Certified component execution — longer term

**Current boundary.** [TightPlan](../../crates/zetesis-ferraris/src/tight/compile.rs)
already certifies complete theories and rejects positive cycles/unsupported
shapes. [Splitting](feedback-and-decomposition.md) remains proposed. The next
extension is a certified component interface, with deterministic bottom closure
or ranked support reused while general components retain full reduct checking.

**Mechanism and obligation.** Smaller conditional tasks might reduce repeated
search/storage and expose independent Rayon/GPU batches. Source SCC membership
alone is insufficient. Prove the applicable splitting hypotheses and exact
reconstruction of all original models; Ferraris's
[proposition 6](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf) supplies a
specific theorem to formalize. Definition elimination needs its own extension
mapping and provenance, not merely equal visible answers.

**Experiment.** Generated bottom/top programs should vary interface size,
recursion, shared constraints and conditional costs. Include top-UNSAT bottom
models, identical interfaces with distinct hidden models and cross-component
objective keys. Compare total enumeration, costs and all ties. Interface caches,
global objective presence and full source coverage make this a later project,
not a relaxation of current language admission.

## Adjacent work: normalization and transposition

These directions concern two different transformations. Semantic normalization
changes a program or expression under a proved equivalence. Physical
transposition changes how an unchanged logical object is stored and traversed.
Keeping them separate makes both their tests and their costs easier to assess.

The [MLIR sparse-tensor dialect](https://mlir.llvm.org/docs/Dialects/SparseTensorOps/)
separates semantic tensor dimensions from storage levels and makes conversion
between formats explicit. It also records that conversion can be expensive or
infeasible. The relevant architectural lesson for zetesis is to retain a logical
operator independently of its layout, then select an implementation with an
explicit conversion/residency budget. It does not require adopting MLIR's
implementation or introducing a C++ dependency.

For example, the current formula and lazy kernels assign a workgroup to each
candidate world. Section 5's alternative packs one atom/node's truth across
candidate occurrences. A transposition experiment must change layout and its
matching schedule together, preserving each occurrence's original/frozen
identity and tail-bit boundaries. It must include packing, conversion, upload,
readback and exact completion in the appropriate reported intervals. A
row-oriented reduction can already have good locality within its workgroup;
transposition is a competing plan, not an unconditional improvement. A useful
Lean target is a commuting representation law: decoding the physical result
equals the logical transform of the decoded input.

[Column-Oriented Datalog on the GPU](https://arxiv.org/abs/2501.13051)
provides a related CUDA relational execution design. Its relevance here is
column layout and relational operators, alongside section 4's delta rounds.
Applying them to source joins and aggregate acquisition is a proposal; that
paper's workload results do not establish ASP reduct preservation, Metal
performance or a GPU default for zetesis.

Semantic normalization needs stronger premises than equality of classical
Boolean values. [Strong equivalence](https://www.cs.utexas.edu/~vl/papers/ht.pdf)
provides a context-independent replacement criterion for the program class it
treats, characterized through the logic of here-and-there. Zetesis must establish
the corresponding criterion for its exact formula/aggregate representation.
For example, replacing `not not p` with `p` in `p :- not not p.` changes its
answer sets: the original has the empty model and `{p}`, while `p :- p.` has
only the empty model. A classical Boolean simplifier cannot authorize that
replacement. Context-dependent simplification instead needs explicit premises
and a theorem that the original program supplies them. Objective-slot presence,
full-model reconstruction and source diagnostics are additional boundaries when
those observations are affected.

The Rust [egg library](https://github.com/egraphs-good/egg) implements equality
saturation: retain equivalent expressions and extract a plan using a cost model.
It could eventually host a bounded rewrite search over a typed arithmetic or
execution-plan language. It supplies no ASP-specific equivalence proof. Start
with a small library of justified rewrites and interval premises; mathematical
identities such as cancelling a multiplication can change checked intermediate
overflow. Proof-carrying rewrites, finite exploration budgets and an unchanged
valid fallback must precede any broad optimizer integration.

The first experiments should therefore be narrow: certified simplification
before expensive source joins, and one alternative cross-candidate truth layout.
Keep the language slices independent. Both experiments must preserve the exact
reduct contract and account for complete source/candidate coverage, including
all retained failure outcomes; neither is a replacement solving semantics.

## First column-oriented grounder experiment

The eager grounder already keeps predicate-local atom rows and per-column
`Value` indexes, probes a short available posting list and schedules positive
joins by relation size. Its cursor retains a scalar assignment and undo trail.
The lazy grounder already uses borrowed predicate relations, bound-prefix
interval lookup and packed per-world membership intersection. Ready comparison
filters and some independent-witness factorization also exist. Column-oriented
work must compare against these mechanisms, rather than claiming indexing or
predicate pushdown is absent.

Start with one positive-pattern extension whose numeric equality/order filter
becomes ready. Preserve existing candidate row IDs, join order and source
snapshot. Compare the current row/`Value` evaluation with a bounded contiguous
`i32` tile, a selection mask and ordered reconstruction of surviving bindings.
Only required fields enter the tile. This does not require replacing the whole
relation store. Measure scalar rows, scalar columns and Rayon columns with
conversion, selection storage and reconstruction included. Vary relation arity,
selectivity, skew and tile size; tiny inputs are an explicit negative control.

Add checked arithmetic only after comparison-only selection preserves error
behavior. Existing partial filters can defer undefined arithmetic because a
prefix may have no complete extension. A new mask cannot convert that deferred
condition, or a complete-row fault, into Boolean false. Structured term identity
and ASP term order also remain distinct from internal canonical storage order.
SEND belongs to this later arithmetic slice; corpus queens and shortest-path
cases can serve as unchanged inputs where the initial profile applies.

The representation law should preserve complete tuples and ordered binding
occurrences. Support relations remain set-valued while source-instance
multiplicity is retained. Original/frozen interpretation identity, cancellation,
deterministic publication and source-exhaustion receipts remain unchanged.
`FiniteBindings`, `JoinFrames` and `ScalarArithmetic` supply starting points for
these Lean obligations, not a proof of the proposed implementation.

GPU tiles become a separate experiment when enough independent numeric rows
can remain resident across several operators. A single tiny comparison followed
by immediate readback must be measured as such. This is a route toward reducing
grounding intermediates; column storage alone does not bound their number or
establish complete lazy formula grounding.

## Selection and acceptance

Native aggregates, candidate cardinality and numeric tiles have the clearest
near-term connection to measured work. Logical grouping is shared infrastructure
worth validating with one consumer. Delta rounds and truth tiles warrant
independent exploratory prototypes; world scheduling follows the current GPU
trace. Component execution should begin with a theorem and applicability
recognizer.

For every experiment, keep the portable reference, unchanged sources, complete
ordered occurrence accounting and negative controls. Publish preparation and
conversion costs alongside kernels; retain slower, refused and interrupted
cases. A heuristic may select a valid plan or propose candidates, but cannot
certify source completeness or stable membership. Incomplete domain information
cannot remove possible models. Integer operations and ASP term identity/order
remain exact throughout. These boundaries make each primitive independently
reviewable and formalizable while preserving the reduct as the solver's
permanent foundation.
