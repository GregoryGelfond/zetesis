# Ferraris/clingo sources and precise semantic boundary

Research checked 2026-09-05. These notes support the forward
[Ferraris profile](ferraris.md); they do not extend the implemented S0 language or
certify the original `kr-domains` corpus. Formula semantics, source translation,
and compatibility with a pinned clingo executable are separate obligations.

## Formula reduct

For atoms, false, conjunction, disjunction and implication, Ferraris defines:

```text
false^M = false
a^M = a                         if a ∈ M, else false
(F ◦ G)^M = F^M ◦ G^M           if M satisfies F ◦ G, else false
                                   ◦ ∈ {and, or, implies}
T^M = { F^M : F ∈ T }
```

Thus maximal subformulas false in M become false. Negation abbreviates implication
to false. M is stable exactly when it is an inclusion-minimal model of T^M;
M satisfies T iff M satisfies T^M. The reduct must remain frozen at M during the
proper-subset check. Classical equivalence suffices for rewriting a frozen reduct;
rewriting the original theory generally needs strong equivalence. See
[Ferraris, *Logic programs with propositional connectives and aggregates*,
§§2.1–2.4](https://arxiv.org/pdf/0812.1462).

FLP instead retains original rules whose bodies hold in M and minimizes models
of that retained program. These definitions coincide on some restricted classes,
but not generally with default-negated aggregates. Ferraris's §3.6.3 example
`p ← ¬(sum⟨p:1⟩ ≤ 0)` has stable models ∅ and {p} under his semantics; FLP has
only ∅. See the [same paper, §3.6.3](https://arxiv.org/pdf/0812.1462).

## Source translation and tuples

Use [*Abstract Gringo*, corrected v2, §§4.4–4.8](https://arxiv.org/pdf/1507.06576v2).
A ground singleton choice translates to `p or not p`; expanded choices conjoin
one such formula per atom. Substitute global variables first; each aggregate
element's remaining variables are local. A body conditional translates to
universally instantiated implications.

For a closed aggregate with a fixed evaluated guard, let A index element/local
substitution pairs, C_j their translated conditions, and U(Δ) the union of their
expanded tuple sets. Let good(Δ) mean the aggregate value on U(Δ) passes the guard.
The translation is:

```text
Aggregate = AND over Δ ⊆ A with not good(Δ):
              (AND_{j ∈ Δ} C_j) IMPLIES (OR_{j ∈ A \ Δ} C_j)
```

Empty conjunction/disjunction mean true/false. Guard expansions occur outside
this formula: `τ(not E) = OR_t not τ_t(E)`, which need not equal `not τ(E)`.
Footnote 14 corrects precisely the non-singleton guard case. Head aggregates use
choices plus a guard constraint. Section 5's counting simplifications require
their stated interval-free assumptions. The infinitary reduct in §4.1 distributes
unconditionally through conjunction/disjunction; it has classically equivalent
reducts to the finite definition above on finite formulas.

The official [Potassco aggregate FAQ](https://potassco.org/doc/faq/2017/04/20/how-do-sum-aggregates-work.html)
confirms whole-tuple set semantics: repeated true supports for one tuple contribute
once. For example, `#sum { 2,k:p; 2,k:q }` contributes 2 when both p and q hold;
changing the second tuple to `2,m` makes the contribution 4. Equal weights alone
are not equal tuple identities.

## Exact aggregate primitives without formula expansion

[Ferraris 2005, *Answer Sets for Propositional Theories*, Proposition 7(b),
§4.1](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf)
provides a particularly useful implementation result. For his weighted-multiset
aggregate A with condition formulas F_i:

```text
J satisfies A^M
  iff M satisfies A
      and aggregate({ weight_i : J satisfies F_i^M }) passes its guard.
```

This evaluates **frozen condition reducts**, not the original conditions under J.
It avoids constructing the exponentially expanded formula. The paper's
multiset-weight representation is not itself clingo's whole-tuple representation.
Section 5.1 also distinguishes Ferraris negative weights from older weight
constraint preprocessing; complementing a negative-weight condition and shifting
the bound is not a generally valid semantic rewrite. Proposition 2 characterizes
strong equivalence through reduct equivalence, and Proposition 4 gives a
restricted fresh-atom definition theorem. Auxiliary-atom compilation needs its
own model-projection argument.

**Proposed Zetesis refinement lemma, not an existing project proof:** for finite
closed aggregates with fixed tuple keys and one evaluated guard, reproduce the
displayed equivalence using the Abstract Gringo translation, replacing multiset
construction with union of tuple sets selected by `C_j^M`. This would justify
`FreezeConditions → EvaluateSubset → CoalesceTuple → ReduceInteger → Compare`.
Retain all alternative condition supports for a tuple. Discarding supports merely
because another support is true in M can change proper-subset behavior.

## Current language and version qualifications

The [official language guide](https://potassco.org/guide/language/) describes
`#sum` as summing integer first tuple terms and `#sum+` as ignoring negative
integers. It specifies local-variable safety, distinguishes tuple expansion from
guard expansion, and notes that recursive aggregates require minimization of
cyclic derivations. Its optimization statements also use unique tuples. Theory
atoms receive semantics from an external theory solver; ordinary aggregate
semantics does not implement clingcon.

The [guide landing page](https://potassco.org/guide/) identifies this online guide
as a clingo 6 work in progress and directs earlier versions to the PDF guide.
Consequently, record the target clingo version, admitted syntax, source
translation revision, arithmetic/undefined-term policy, and warning policy in
the compatibility contract. Abstract Gringo's specified language is a large
subset, not a blanket specification of every current clingo extension.

For a separate, explicitly restricted first-order result,
[Fandinno, Hansen and Lierler, *Axiomatization of Non-Recursive Aggregates in
First-Order Answer Set Programming*, JAIR 80 (2024)](https://www.unomaha.edu/college-of-information-science-and-technology/natural-language-processing-and-knowledge-representation-lab/research-projects/fahali24a.pdf)
relates its axiomatization to ASP-Core-2 and to clingo under stated recursion
conditions. Its nonrecursive/non-positive-recursion scope must not be cited as
a theorem for arbitrary recursive aggregates.

## Consequences for Zetesis's oracle contract

These are architectural deductions, not claims that the cited implementations
use the proposed hardware arrangement.

The general oracle checks M's theory truth and the absence of any J strictly
contained in M satisfying T^M. It does not recursively test whether J is stable.
For example, `a or b` at M={a,b} has two incomparable proper-subset models; the
intersection is not a model. A generic least-closure oracle cannot decide this
minimality problem.

An exact sufficient condition for retaining the S0 specialization is a proved
translation to a definite Horn theory D_M and positive constraints K_M such that,
for every J ⊆ M, satisfaction of T^M agrees with satisfaction of D_M ∪ K_M.
After M passes the model check, L=least(D_M) is contained in M and satisfies K_M;
then M is stable iff L=M. If only a gate seed is guessed, the additional
reconstruction/projection theorem remains necessary. Aggregate operators having
integer implementations does not establish this Horn condition.

Lazy source discovery must preserve both the outer truth check and all relevant
proper-subset checks. A final certificate covering only aggregate truth at M
does not establish minimality. General aggregate primitives may be exact and
hardware-friendly while the surrounding oracle still needs subset search.
Positive Horn regions can retain event counters; mixed-weight or nonmonotone
aggregate regions require an explicit semantic specialization or the general
formula path. No termination, performance, complete corpus, or hardware claim
follows from these source results alone.
