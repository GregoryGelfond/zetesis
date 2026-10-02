# Computing a finite normal reduct's least closure

[FiniteClosure](../Zetesis/FiniteClosure.lean) supplies an executable finite
algorithm and proves its result against the independently defined normalized
reduct semantics. Atoms need decidable equality; their ambient type need not
be finite. The result is reusable for finite ground normal programs.

## Input and execution

A `Rule` holds an optional head, positive body atoms, true gates and false
gates. Ground filters have already been discharged. Its `denote` operation
constructs the existing `Semantics.Rule` with a true filter, and `program`
maps the complete list. This is a finite input representation, not another
definition of answer-set membership. Source grounding and filter evaluation
are outside this module.

`step` reads gates only in the frozen seed. If a headed rule is enabled and
its positive body holds, it inserts its head unless already present. `scan`
visits the rules in list order, allowing later rules to use earlier insertions
immediately. This differs operationally from a synchronous consequence round,
although both compute the same least closure.

`iterate` performs at most its supplied number of complete scans. Zero fuel
returns `none`; an unchanged scan returns `some` of the completed closure.
An unfinished computation therefore does not stand for rejection.

## Why the computed closure is exact

The proof has three invariants and a termination argument:

1. `scan_retains` preserves all previous truth. `scan_nodup` preserves a
   duplicate-free retained list.
2. `scan_sound` preserves inclusion in the semantic least consequence set.
   Each new head follows from one actual rule whose body is already justified.
3. `unchanged_closed` proves that every enabled consequence is present after
   an unchanged complete scan. A scan prefix alone cannot establish this.
4. `changing_scan_grows` gives strict length growth. `scan_origin` confines
   every new atom to the finite list of rule heads. Consequently
   `iterate_completes` and `empty_completes` establish a constructive bound:
   one more scan than the number of listed heads suffices from empty truth.
   Duplicate heads make the bound conservative, without invalidating it.

`closure` runs with that bound and extracts the computed result using the
proved fact that it is present. It does not select a model by classical
choice or return a default closure on failure. `closure_exact` combines
soundness and closedness through `Transformers.exact_of_sound_and_closed`
to obtain exactly `Semantics.Gamma`.

## From closure to answer sets

`constraints` checks every enabled constraint against completed positive
truth. `agrees` checks both inclusions of the closure's carrier projection
and the seed, including rejection of an extra seed atom outside the carrier.
`accepts_exact` proves that their conjunction is precisely the existing
`Semantics.Accept` predicate.

`carrier` collects all gate atoms from the input rules; `carrier_covers`
proves its coverage directly. `accepted_stable` then applies the existing
seed-soundness theorem. In the other direction, `stable_has_seed` uses the
finite gate projection of an arbitrary answer set and proves that the
executable algorithm accepts it and reconstructs that whole answer set.
`stable_iff_accepted_seed` states the resulting equivalence. This existence
theorem does not itself enumerate the seeds.

## Cost and remaining boundaries

Fuel measures full scans, not primitive operations. For `r` rules, `h` listed
heads, `l` total body and gate occurrences, and seed-list length `s`, the list
implementation takes at most
`O((h + 1) * (r * (h + 1) + l * (h + s + 1)))` equality and list operations
for closure, treating atom equality as unit cost. Derived truth contains at
most `h` atoms. These are algorithmic bounds, not claims about Rust allocation,
physical memory or hardware running time.

The checked examples distinguish unsupported positive cycles, a frozen gate
from initial positive truth, an empty constraint, and zero fuel. The general
proofs permit duplicate rules, body entries and seeds, and unused atoms in the
ambient type.

The module proves an executable Lean algorithm. Rust packed representations,
source-to-ground translation, optimized join schedules, cancellation and
resource accounting, candidate enumeration and output delivery still require
their own correspondences. The normalized/Ferraris bridge remains
[NormalFerraris](normal-ferraris.md); this module does not replace its reduct
or minimality definitions.
