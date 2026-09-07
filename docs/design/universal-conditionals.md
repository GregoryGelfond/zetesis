# Universal body conditionals

An admitted body conditional `H : C` denotes a finite conjunction of original
implications, one for each completed local binding:

\[
U = \bigwedge_{\theta \in D}(C\theta \rightarrow H\theta).
\]

The enclosing rule retains `U` in its original body. The compiler does not replace
implication with classical disjunction: classical equivalence alone need not
preserve the Ferraris reduct. Completed empty binding sets denote truth;
interrupted, refused or only partially discovered sets do not establish vacuity.

## Admitted finite profile

Ordinary body and head occurrences establish outer variables before conditional
local scopes are compiled. Each conditional then receives its own local scope.
Local variables require independent bindings from the condition through the
existing positive relation or scalar binding mechanisms. The consequent is a
test, and supplies neither local bindings nor possible-positive support.

Consequents can be signed atoms with default negation or an admitted atom-free
ground guard. Conditions use the existing finite literal profile. This bounded
slice does not implement every clingo grounding case: consequent-only variables
and bindings that require interactions between conditional elements can remain
refused even when clingo accepts the source. Objective-reachable conditional
producers are explicitly refused until the objective dependency contract is
extended. These restrictions describe native admission, not a universal claim
about clingo safety.

Possible-positive support construction conservatively ignores the conditional
test. Only after that finite support computation completes does lowering join
its conditions and emit each original implication. Support membership establishes
possible bindings, not model truth. Recursion therefore retains its original
meaning. For example, `p :- p:p.` has the answer set `{p}`; rewriting the inner
`p -> p` as a classical literal disjunction changes the reduct structure.

Scopes, generated-target dependencies, source origins and admission limits
remain explicit. Resource exhaustion propagates an error; no conjunction of a
partial prefix is returned as a complete conditional.

## Candidate-specific execution contract

For a fixed original candidate `M`, an instance with a condition false in `M`
has a false antecedent in every frozen reduct interpretation `J`. Its implication
can be omitted from that candidate's exact check. Every retained implication
still needs its condition's frozen reduct evaluated in `J`; truth in `M` does not
allow replacing `C -> H` with `H`.

The Lean module `UniversalConditionals` proves original and frozen semantics,
empty vacuity, omission under that fixed-candidate premise, preservation within
the enclosing rule and preservation of that candidate's stability. Static reuse
across a candidate domain requires uniform falsity on the domain. Counterexamples
show why reusing an `M`-specialized list for another candidate or erasing retained
antecedents can change truth.

This is a foundation for lazy, candidate-specific oracle execution. The current
formula compiler still constructs the finite universal family eagerly. A future
device representation must establish binding coverage, frozen candidate identity,
bounded chunk completion and original/frozen truth before skipping work. The Lean
laws assume a complete finite instance list; they do not prove source joins,
scope analysis, Rust compilation or WGSL refinement.
