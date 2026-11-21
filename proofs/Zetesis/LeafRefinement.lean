import Zetesis.Search
import Zetesis.LearnedRegions

/-!
# Size-specialized narrowing refines refutation

Original-region narrowing refutes a region when no member of it can be an answer
set. Below a measured region size the solver reads the same question through one
of two specialized implementations (`regions/small.rs`):

* **enumeration** checks every completion of the region's open atoms, 64 per
  machine word, against a necessary condition `N` of validity;
* **learned search** runs a bounded conflict-learning search over clauses that are
  consequences of `N`, under the region's decided atoms as assumptions, and
  reports the assumptions a refutation used (its core).

Both only ever report *refuted* or decline; a declined region is left to the
general narrowing unchanged. This file states what each report means.

`N` is any predicate every valid interpretation satisfies (`Necessary`). For the
implementation it is classical satisfaction of the theory's roots together with
producer support, and, for active restrictions and objective bounds, their roots;
`Necessary` for that reading is the named premise the Rust encoding discharges
(an answer set is a classical model and is supported, `ExternalSupport`).

The laws: a refuted region yields the premise of `CoverageTree.refuted`; replacing
any subtree over a region with no valid member by a refutation leaf keeps the
tree's outputs, in order (`refutation_keeps_outputs`); enumeration is exact for
`N` (`enumeration_exact`), so it refutes exactly the regions `N` refutes; a learned
core whose literals the region decides refutes it under `CoreEntailed`, which
follows from a sound clause database and a derived clause over the core's
negations (`clause_entails_core`, `resolvent_sound`); and a core is a learned row
`RowOf N rest [negate l]` that flows back to the region ledger.

Not certified here: the Rust Tseitin and threshold encodings, watched-literal
propagation, analysis, clause deletion, the 64-lane packing beyond
`lanes_refute`, or the size metric that chooses a specialization; each enters as
the named premise of the law that uses it.
-/

namespace Zetesis.LeafRefinement

open Zetesis.LearnedRegions

universe u
variable {α : Type u}

/-- `N` is necessary for validity: every valid interpretation satisfies it. -/
def Necessary (valid N : Atoms α → Prop) : Prop := ∀ z, valid z → N z

/-- A region with no valid member contributes no outputs to any coverage tree
over it. So a refutation leaf put in place of such a subtree keeps the whole
tree's outputs and their order.

Proof. By `mem_outputs_iff` an output is a valid member of the region; there is
none, so the output list has no element. -/
theorem refutation_keeps_outputs {valid : Atoms α → Prop} {c : Region α}
    (tree : CoverageTree valid c) (empty : ∀ z, c.Contains z → ¬ valid z) :
    tree.outputs = [] := by
  have noOutput : ∀ z, z ∉ tree.outputs := by
    intro z listed
    have member : c.Contains z ∧ valid z := (tree.mem_outputs_iff z).mp listed
    exact empty z member.1 member.2
  exact List.eq_nil_iff_forall_not_mem.mpr noOutput

/-- A region no member of which satisfies a necessary condition has no valid
member: the premise of `CoverageTree.refuted`. -/
theorem necessary_refutes {valid N : Atoms α → Prop} (necessary : Necessary valid N)
    {c : Region α} (none : ∀ z, c.Contains z → ¬ N z) :
    ∀ z, c.Contains z → ¬ valid z := by
  intro z inside isValid
  exact none z inside (necessary z isValid)

/-- `N` reads only atom membership: interpretations agreeing on every atom agree
on `N`. -/
def Extensional (N : Atoms α → Prop) : Prop :=
  ∀ z w : Atoms α, (∀ a, z a ↔ w a) → N z → N w

/-- The completions `ws` cover the region: every member agrees atomwise with one
of them. Enumeration's completions are the assignments of the open atoms over
the region's decided ones. -/
def Covers (c : Region α) (ws : List (Atoms α)) : Prop :=
  ∀ z, c.Contains z → ∃ w ∈ ws, ∀ a, z a ↔ w a

/-- Enumeration is exact for `N`: when the completions cover the region and lie
in it, every completion failing `N` is the same as every member failing `N`.
Enumeration therefore refutes exactly the regions refutation by `N` refutes.

Proof. (→) A member agrees with a covering completion; that completion fails `N`,
so by extensionality the member does too. (←) The completions are members. -/
theorem enumeration_exact {N : Atoms α → Prop} (extensional : Extensional N)
    {c : Region α} {ws : List (Atoms α)} (covers : Covers c ws)
    (inside : ∀ w ∈ ws, c.Contains w) :
    (∀ w ∈ ws, ¬ N w) ↔ ∀ z, c.Contains z → ¬ N z := by
  constructor
  · intro checked z member holds
    obtain ⟨w, listed, agrees⟩ := covers z member
    have holdsAtW : N w := extensional z w agrees holds
    exact checked w listed holdsAtW
  · intro none w listed holds
    exact none w (inside w listed) holds

/-- An enumeration that finds no completion satisfying `N` refutes the region:
no member is valid. -/
theorem enumeration_refutes {valid N : Atoms α → Prop} (necessary : Necessary valid N)
    (extensional : Extensional N) {c : Region α} {ws : List (Atoms α)}
    (covers : Covers c ws) (inside : ∀ w ∈ ws, c.Contains w)
    (checked : ∀ w ∈ ws, ¬ N w) : ∀ z, c.Contains z → ¬ valid z := by
  have none : ∀ z, c.Contains z → ¬ N z :=
    (enumeration_exact extensional covers inside).mp checked
  exact necessary_refutes necessary none

/-- Checking the completions in lanes, one machine word at a time, checks them
all: every completion lies in some lane. -/
theorem lanes_refute {N : Atoms α → Prop} (lanes : List (List (Atoms α)))
    (checked : ∀ lane ∈ lanes, ∀ w ∈ lane, ¬ N w) :
    ∀ w ∈ lanes.flatten, ¬ N w := by
  intro w listed
  obtain ⟨lane, inLanes, inLane⟩ := List.mem_flatten.mp listed
  exact checked lane inLanes w inLane

/-- An interpretation satisfies a clause when it satisfies one of its literals. -/
def Satisfies (z : Atoms α) (clause : List (Literal α)) : Prop :=
  ∃ l ∈ clause, Holds z l

/-- Every clause of the database holds in every interpretation satisfying `N`:
the encoding's clauses are consequences of `N`, and learning keeps it so. -/
def DbSound (N : Atoms α → Prop) (db : List (List (Literal α))) : Prop :=
  ∀ z, N z → ∀ clause ∈ db, Satisfies z clause

/-- No interpretation satisfying `N` satisfies every literal of the core. -/
def CoreEntailed (N : Atoms α → Prop) (core : List (Literal α)) : Prop :=
  ∀ z, N z → ¬ ∀ l ∈ core, Holds z l

/-- Resolution keeps clauses consequences of `N`: resolving two such clauses on a
literal `p` and its negation gives one.

Proof. An interpretation satisfying `N` satisfies both clauses. If its literal in
the first is not `p`, that literal is in the resolvent. Otherwise `p` holds, so the
second clause's satisfied literal is not `negate p` and is in the resolvent. -/
theorem resolvent_sound [DecidableEq α] {N : Atoms α → Prop}
    {left right : List (Literal α)} (p : Literal α)
    (leftSound : ∀ z, N z → Satisfies z left)
    (rightSound : ∀ z, N z → Satisfies z right) :
    ∀ z, N z →
      Satisfies z (left.filter (fun l => l ≠ p) ++ right.filter (fun l => l ≠ negate p)) := by
  intro z holds
  obtain ⟨l, inLeft, lHolds⟩ := leftSound z holds
  by_cases isPivot : l = p
  · have pHolds : Holds z p := isPivot ▸ lHolds
    obtain ⟨r, inRight, rHolds⟩ := rightSound z holds
    have notNegated : r ≠ negate p := by
      intro equal
      exact holds_negate_excludes (equal ▸ rHolds) pHolds
    have kept : r ∈ right.filter (fun l => l ≠ negate p) := by
      simp [List.mem_filter, inRight, notNegated]
    exact ⟨r, List.mem_append_right _ kept, rHolds⟩
  · have kept : l ∈ left.filter (fun l => l ≠ p) := by
      simp [List.mem_filter, inLeft, isPivot]
    exact ⟨l, List.mem_append_left _ kept, lHolds⟩

/-- Clause minimization keeps a learned clause a consequence of `N`. A literal
`q` of a sound clause may be dropped when a sound reason clause
`negate q :: rest` has every literal of `rest` in the clause and distinct from
`q` (`redundant` in `regions/small.rs`). Recursive minimization drops literals
one at a time, each by this law, after first resolving away the intermediate
literals its search marked; each such resolution is `resolvent_sound`.

Proof. Resolve the clause with the reason on `q` (`resolvent_sound`): a satisfied
literal of the resolvent is either in the clause and not `q`, or a literal of
`rest`, which is in the clause and not `q` as well. -/
theorem minimized_sound [DecidableEq α] {N : Atoms α → Prop}
    {clause rest : List (Literal α)} (q : Literal α)
    (sound : ∀ z, N z → Satisfies z clause)
    (reason : ∀ z, N z → Satisfies z (negate q :: rest))
    (covered : ∀ l ∈ rest, l ∈ clause ∧ l ≠ q) :
    ∀ z, N z → Satisfies z (clause.filter (fun l => l ≠ q)) := by
  intro z holds
  obtain ⟨l, inResolvent, lHolds⟩ := resolvent_sound q sound reason z holds
  rcases List.mem_append.mp inResolvent with inLeft | inRight
  · exact ⟨l, inLeft, lHolds⟩
  · have inReason : l ∈ negate q :: rest := (List.mem_filter.mp inRight).1
    have notNegated : l ≠ negate q := by
      simpa using (List.mem_filter.mp inRight).2
    have inRest : l ∈ rest := by
      rcases List.mem_cons.mp inReason with equal | listed
      · exact absurd equal notNegated
      · exact listed
    obtain ⟨inClause, notPivot⟩ := covered l inRest
    exact ⟨l, List.mem_filter.mpr ⟨inClause, by simpa using notPivot⟩, lHolds⟩

/-- A sound clause all of whose literals negate core literals entails the core.
Final-conflict analysis derives such a clause: the negated assumptions of a
refutation.

Proof. Under `N` the clause has a satisfied literal `l`; `negate l` is in the core,
so not every core literal holds. -/
theorem clause_entails_core {N : Atoms α → Prop} {clause core : List (Literal α)}
    (sound : ∀ z, N z → Satisfies z clause)
    (negated : ∀ l ∈ clause, negate l ∈ core) : CoreEntailed N core := by
  intro z holds all
  obtain ⟨l, listed, lHolds⟩ := sound z holds
  have opposite : Holds z (negate l) := all (negate l) (negated l listed)
  exact holds_negate_excludes opposite lHolds

/-- A learned refutation is a sound one: a region deciding every literal of an
entailed core has no valid member, the premise of `CoverageTree.refuted`.

Proof. A member satisfies every decided literal (`decides_holds`), so it satisfies
the whole core and fails `N`; being necessary, it is not valid. -/
theorem learned_refutes {valid N : Atoms α → Prop} (necessary : Necessary valid N)
    {core : List (Literal α)} (entailed : CoreEntailed N core) {c : Region α}
    (decided : ∀ l ∈ core, Decides c l) : ∀ z, c.Contains z → ¬ valid z := by
  have none : ∀ z, c.Contains z → ¬ N z := by
    intro z inside holds
    have all : ∀ l ∈ core, Holds z l := fun l listed => decides_holds inside (decided l listed)
    exact entailed z holds all
  exact necessary_refutes necessary none

/-- An entailed core is a learned row: with the rest of the core as conditions,
the negation of its first literal follows. Rows of `N` are rows of the valid
class (`row_of_narrower_class`), so the core flows back to the region ledger.

Proof. If the conditions hold and `negate l` failed, `l` would hold
(`holds_or_negate`) and the whole core would hold, which `N` excludes. -/
theorem core_row {N : Atoms α → Prop} {l : Literal α} {rest : List (Literal α)}
    (entailed : CoreEntailed N (l :: rest)) : RowOf N rest [negate l] := by
  intro z holds conditions o listed
  have isNegation : o = negate l := List.mem_singleton.mp listed
  subst isNegation
  cases holds_or_negate z l with
  | inl lHolds =>
    have all : ∀ k ∈ l :: rest, Holds z k := by
      intro k inCore
      cases List.mem_cons.mp inCore with
      | inl equal => exact equal ▸ lHolds
      | inr inRest => exact conditions k inRest
    exact absurd all (entailed z holds)
  | inr negated => exact negated

/-- The core row holds for the valid class too. -/
theorem core_row_valid {valid N : Atoms α → Prop} (necessary : Necessary valid N)
    {l : Literal α} {rest : List (Literal α)} (entailed : CoreEntailed N (l :: rest)) :
    RowOf valid rest [negate l] :=
  row_of_narrower_class N valid necessary rest [negate l] (core_row entailed)

end Zetesis.LeafRefinement
