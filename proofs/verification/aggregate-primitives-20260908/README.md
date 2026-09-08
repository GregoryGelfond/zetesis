# Aggregate primitives and cardinality consequences

`AggregateReduct` formalizes Proposition 7 of Paolo Ferraris's
[Answer Sets for Propositional Theories](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf).
For a fixed finite tuple carrier, a canonical aggregate is true precisely when
its guard accepts the eligibility mask. Its frozen reduct is true precisely when
the guard accepts both the original mask and the mask of recursively frozen
eligibility formulas. Neither direction assumes the second interpretation is a
subset of the first, and the guard need not be monotone or numerical.

The module proves coverage of an executable Boolean-mask enumeration and states
the agreement needed for a replacement evaluator. The reference definition
enumerates subsets to specify the formula; it does not prescribe that runtime
algorithm. This result is an attributed formalization of an existing semantic law.

`PartitionCapacities` proves the counting argument behind derived local lower
bounds. A selected group must supply the total lower bound minus the sum of all
other capacities, with natural subtraction clipping at zero. The argument counts
list occurrences. A caller must establish complete distinct group membership
and that the original theory entails the total and capacities under the same
activation before using these consequences to restrict candidates.

Together the modules add 18 laws. The complete record covers 812 laws across 71
semantic modules with the pinned Lean 4.33.1 toolchain. The previous 794 theorem
names, source locations, module bytes and transitive axiom sets are retained for
continuity checks. The current command logs record the clean build, strict check
of every module and complete axiom audit.

The source-to-group bridge, concrete numeric/term operations, compiled DAG
correspondence, resource accounting and Rust/WGSL refinement remain unproved.
Passing these checks does not certify native aggregate execution or hardware.

An independent review checked both modules against the cited Proposition 7 and
the partition API. It found no correctness or scope blocker. In particular, the
review checked arbitrary interpretations, the empty carrier, duplicate reference
masks, recursively frozen eligibility, occurrence counting and the explicitly
unproved source/implementation bridges. That review is an assessment of the
statements and arguments, separate from the retained kernel-checking logs.
