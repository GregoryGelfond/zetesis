import Zetesis.Search
import Zetesis.Frontier
import Zetesis.FormulaBounds
import Zetesis.ReductRegions
import Zetesis.LearnedRegions
import Zetesis.AxisFolds
import Zetesis.AlgebraicRows
import Zetesis.StreamedConsequences

/-!
# The solver's algebra

The solver decides membership by the definition of an answer set and proposes
candidates by narrowing regions. Both halves are one algebra. This module names
its objects, with the same names the implementation uses, and states the laws
that hold for every instance.

| Object | Here | Is a |
| --- | --- | --- |
| `Region` | atoms every member holds (`lower`) and atoms a member may hold (`upper`) | an interval of interpretations; under `FormulaBounds.Inside`, the precision order, a poset, so a thin category |
| `Class` | the interpretations a walk enumerates: `AnswerSetsSought` or `Countermodels` | a set of interpretations |
| `Subject` | a narrowing that only adds decisions, is `Monotone` and is sound for its class | a monotone inflationary map, a pointed endofunctor of the thin category |
| `joint` | the least region inside a start that every subject of a family leaves unchanged | a closure operator, so an idempotent monad on the thin category |
| walk | a class, subjects sound for it, a leaf decision (`LeafVerdict`) and a schedule; certified by `Search.CoverageTree`, stepped by `Frontier.Pending.Walk` | a recursive coalgebra whose unfold is the covering tree |
| `Reduction` | `AxisFolds.Reduction`, folded over a walk's accepted leaves, with a `FoldVerdict` when the walk may stop | a commutative monoid; a fold is a monoid homomorphism |
| meter | work charged as performed | a cost monoid; an implementation obligation, not stated here |

A walk narrows regions by its subjects and splits them. A decided region no subject
refutes is a **candidate**, and the walk's leaf decision accepts it, refutes it or
leaves it undecided. The definition of an answer set is a nested walk. The
candidate walk's candidates are classical models (`FormulaBounds.decided_leaf_models`),
and its leaf decision is each candidate's countermodel walk (`answerSetVerdict`);
the countermodel walk's leaf decision accepts every candidate but `M` itself
(`countermodelVerdict`), and the `any` reduction over its accepted leaves, whose
absorbing element `true` stops at the first countermodel, is the verdict
(`inner_verdict`, `answerSetVerdict_accepts`, `answer_sets_nested`).

The laws, by the numbering of the algebraic-core design:
- **L1, soundness composes:** `Subject.apply_sound`, `Reaches.sound`.
- **L2, walks are exact over accepted leaves:** `CoverageTree.mem_outputs_iff`.
  `Subject.node` makes a narrowing by a sound subject a node of that certificate,
  and `LeafVerdict.node` makes a decided candidate a leaf of it.
- **L3, the definition:** `ReductRegions.stable_iff_no_countermodel`, here as
  `inner_verdict`.
- **L4, folds:** `AxisFolds.Reduction.fold_perm`; early decision at an absorbing
  element, which is final (`foldVerdict_absorbed`, from `fold_decided_prefix`), and
  otherwise a value only on completion (`foldVerdict_complete`), with an empty
  family never the unit (`foldVerdict_empty`). Early decision needs the members of
  one walk, or, over a world view, an idempotent reduction or distinct members.
- **L5, order independence:** the joint fixed point is a closure operator
  (`joint_extensive`, `joint_monotone`, `joint_idempotent`, `joint_least`,
  `joint_closed`, `joint_sound`). Two chaotic iterations of a fixed family that stop
  at closed regions stop at the same one (`terminal_agreement`, `Reaches.eq_joint`),
  and over finitely many atoms every fair iteration stops there (`refines_wf`,
  `fair_iteration_reaches_joint`). This is the chaotic iteration theorem of
  constraint propagation (Apt 1999). It needs the family fixed: across learned
  rows, a tightening bound, added restrictions or declined allowances, only the
  answer sets stay identical (L1, L2).
- **L6, class discipline:** a subject sound for a class is sound for every
  narrower class (`Subject.sound_of_narrower`). The membership check is constructed
  from the theory and the candidate alone (`countermodelSubjects`), and its verdict
  depends on them alone (`countermodel_verdict_independent`).

The joint fixed point is defined as the meet of the closed regions inside the
start, not by iteration, so its existence and the closure laws need no
finiteness. A closure operator on a poset is exactly an idempotent monad on the
corresponding thin category; the regions it leaves unchanged are its algebras.

**The KR reading.** A region is a consistent pair of approximation fixpoint
theory (Denecker, Marek and Truszczyński). The readings subject reads formulas
with Kleene's strong three-valued connectives in both directions, and the
frozen subject reads the Ferraris reduct under the candidate. The downward
direction is constraint propagation, not an approximator of an
immediate-consequence operator, and learned and algebraic rows are consequences
of their class. So the joint fixed point is a sound closure relative to a class,
not a fixpoint of approximation fixpoint theory.

**Not proved here.** That the Rust narrower computes these subjects, that its
joint loop applies each subject until none decides more, that its walks are
`Frontier` walks, and that the meter charges the work performed are
implementation obligations; `ReductRegions.masked_known_sound` already relates
propagation under the frozen mask to the frozen subject's readings. The split is
chosen from the knowledge after the fixed point, so the same region gives the same
split only if the Rust closure equals the inductive closure `FormulaBounds.Known`.
The support cut, the streamed constraints and the objective bound are not yet
instances.

The region type and its geometry are `Zetesis.Region`, defined in
`Zetesis.Search`; this module uses that original owner directly.
-/

namespace Zetesis.Algebra

open Ferraris FormulaBounds LearnedRegions

universe u v

variable {α : Type u}

/-! ## Regions and the precision order -/

/-- Every region is inside itself. -/
theorem inside_refl (r : Region α) : Inside r r :=
  ⟨fun _ held => held, fun _ possible => possible⟩

/-- A region inside a region inside `r` is inside `r`. -/
theorem inside_trans {r r' r'' : Region α} (inner : Inside r'' r') (outer : Inside r' r) :
    Inside r'' r :=
  ⟨fun a held => inner.1 a (outer.1 a held), fun a possible => outer.2 a (inner.2 a possible)⟩

/-- Two regions each inside the other are equal. With `inside_refl` and
`inside_trans`, the precision order is a partial order: regions form a poset, a
thin category. -/
theorem inside_antisymm {r r' : Region α} (inner : Inside r' r) (outer : Inside r r') :
    r' = r := by
  cases r with
  | mk lower upper =>
    cases r' with
    | mk lower' upper' =>
      have sameLower : lower' = lower := sub_antisymm outer.1 inner.1
      have sameUpper : upper' = upper := sub_antisymm inner.2 outer.2
      rw [sameLower, sameUpper]

/-- Narrowing a region to the bounds of a region inside it gives that region. -/
theorem narrow_to_inside {r r' : Region α} (inside : Inside r' r) :
    r.narrow r'.lower r'.upper = r' := by
  cases r' with
  | mk lower' upper' =>
    have sameLower : Union r.lower lower' = lower' :=
      atoms_ext fun a => ⟨fun held => Or.elim held (inside.1 a) id, Or.inr⟩
    have sameUpper : Inter r.upper upper' = upper' :=
      atoms_ext fun a => ⟨And.right, fun possible => ⟨inside.2 a possible, possible⟩⟩
    show Region.mk (Union r.lower lower') (Inter r.upper upper') = Region.mk lower' upper'
    rw [sameLower, sameUpper]

/-! ## Classes -/

/-- A **class**: the interpretations a walk enumerates. -/
abbrev Class (α : Type u) := Atoms α → Prop

/-- **The answer sets sought**: the stable models of `T` that satisfy the
restrictions `R`, the objective bound among them. Candidate generation walks this
class, and its subjects may use whatever holds in every member. -/
def AnswerSetsSought (T R : Theory α) : Class α := fun z => Stable z T ∧ Models z R

/-- **The countermodels** of a candidate `M`: the proper subsets of `M` that model
the frozen reduct of `T`. The membership check walks this class, and `M` is stable
exactly when it is empty (`ReductRegions.stable_iff_no_countermodel`). -/
def Countermodels (T : Theory α) (M : Atoms α) : Class α := ReductRegions.Countermodel M T

/-- The classical models of a theory. -/
def ModelsOf (U : Theory α) : Class α := fun z => Models z U

/-! ## Subjects -/

/-- A map of regions is **monotone** in the precision order when it maps a more
precise region to a more precise region. The library has no dependency that
defines `Monotone`; this is the notion for regions under `Inside`. -/
def Monotone (f : Region α → Region α) : Prop :=
  ∀ {r r' : Region α}, Inside r' r → Inside (f r') (f r)

/-- A **subject** for a class `C`: a narrowing of regions that only adds decisions,
is monotone in the precision order, and is sound for `C`: every member of `C` in a
region is a member of its narrowing. A narrowing that refutes a region narrows it
to a region with no members.

A subject is a monotone inflationary map of the region poset: categorically, a
pointed endofunctor `id ⇒ narrow` of the thin category. -/
structure Subject (C : Class α) where
  /-- The narrowing. -/
  narrow : Region α → Region α
  /-- It only adds decisions: the narrowing lies inside the region. -/
  extensive : ∀ r, Inside (narrow r) r
  /-- A more precise region narrows to a more precise region. -/
  monotone : Monotone narrow
  /-- No member of the class is lost. -/
  sound : ∀ {r : Region α} {z : Atoms α}, r.Contains z → C z → (narrow r).Contains z

namespace Subject

variable {C D : Class α}

/-- **L6, a narrower class.** A subject sound for `C` is sound for every class
inside `C`: soundness can only be lost by widening the class. This generalizes
`LearnedRegions.row_of_narrower_class` from rows to subjects. -/
theorem sound_of_narrower (s : Subject C) (inclusion : ∀ z, D z → C z)
    {r : Region α} {z : Atoms α} (inside : r.Contains z) (member : D z) :
    (s.narrow r).Contains z :=
  s.sound inside (inclusion z member)

/-- The same narrowing, as a subject of a narrower class (`sound_of_narrower`). -/
def toNarrower (s : Subject C) (inclusion : ∀ z, D z → C z) : Subject D where
  narrow := s.narrow
  extensive := s.extensive
  monotone := s.monotone
  sound := fun inside member => s.sound_of_narrower inclusion inside member

/-- **A narrowing is a node of the walk's certificate.** Narrowing a region by a
subject sound for `C` gives a `narrowed` node of the coverage tree whose valid
seeds are `C`. By `CoverageTree.mem_outputs_iff` (L2), the completed tree's leaves
are then exactly the members of `C` in its root, whichever subjects narrowed it. -/
def node (s : Subject C) (r : Region α) (child : CoverageTree C (s.narrow r)) :
    CoverageTree C r :=
  .narrowed r (s.narrow r).lower (s.narrow r).upper
    (fun _ inside member => (s.sound inside member).1)
    (fun _ inside member => (s.sound inside member).2)
    (cast (congrArg (CoverageTree C) (narrow_to_inside (s.extensive r)).symm) child)

/-- Apply a schedule of subjects, first to last. -/
def apply (schedule : List (Subject C)) (r : Region α) : Region α :=
  schedule.foldl (fun region s => s.narrow region) r

/-- **L1, soundness composes.** Every member of the class in a region is a member
of the region's narrowing by any schedule of subjects sound for the class. -/
theorem apply_sound (schedule : List (Subject C)) {r : Region α} {z : Atoms α}
    (inside : r.Contains z) (member : C z) : (apply schedule r).Contains z := by
  induction schedule generalizing r with
  | nil => exact inside
  | cons s rest ih => exact ih (s.sound inside member)

/-! ### Pointwise lifting to a finite cohort

A cohort here is just a finite indexed family of existing regions. Each position
keeps its own class and subject. Applying those subjects pointwise introduces no
new region representation and no schedule, storage layout or leaf decision.
The class may be the same at every position, or may differ, as for countermodel
queries belonging to different candidates. No membership assumption passes from
one position to another.
-/

/-- Pointwise application of subjects only adds decisions in each region.
The finite product uses the componentwise precision order. -/
theorem cohort_extensive {n : Nat} {classes : Fin n → Class α}
    (subjects : (i : Fin n) → Subject (classes i))
    (regions : Fin n → Region α) :
    ∀ i, Inside ((subjects i).narrow (regions i)) (regions i) := by
  intro i
  exact (subjects i).extensive (regions i)

/-- Pointwise application is monotone when each position uses the same subject
on both families. This does not compare different learned rows, objective bounds
or subject families. -/
theorem cohort_monotone {n : Nat} {classes : Fin n → Class α}
    (subjects : (i : Fin n) → Subject (classes i))
    {regions smaller : Fin n → Region α}
    (inside : ∀ i, Inside (smaller i) (regions i)) :
    ∀ i, Inside ((subjects i).narrow (smaller i))
      ((subjects i).narrow (regions i)) := by
  intro i
  exact (subjects i).monotone (inside i)

/-- Pointwise application retains every supplied class member in its own
region. Class membership is a premise; a cohort of candidate regions does not
thereby become a cohort of accepted answer sets. -/
theorem cohort_sound {n : Nat} {classes : Fin n → Class α}
    (subjects : (i : Fin n) → Subject (classes i))
    {regions : Fin n → Region α} {members : Fin n → Atoms α}
    (inside : ∀ i, (regions i).Contains (members i))
    (member : ∀ i, classes i (members i)) :
    ∀ i, ((subjects i).narrow (regions i)).Contains (members i) := by
  intro i
  exact (subjects i).sound (inside i) (member i)

/-- Each position keeps exactly its original class members. Extensivity gives
containment back in the original region; soundness gives the reverse direction.
The other positions need not have any members. -/
theorem cohort_members_iff {n : Nat} {classes : Fin n → Class α}
    (subjects : (i : Fin n) → Subject (classes i))
    (regions : Fin n → Region α) (i : Fin n) (z : Atoms α) :
    (((subjects i).narrow (regions i)).Contains z ∧ classes i z) ↔
      ((regions i).Contains z ∧ classes i z) := by
  constructor
  · intro narrowed
    have original : (regions i).Contains z :=
      StreamedConsequences.contains_of_inside
        ((subjects i).extensive (regions i)) narrowed.1
    exact ⟨original, narrowed.2⟩
  · intro original
    have retained : ((subjects i).narrow (regions i)).Contains z :=
      (subjects i).sound original.1 original.2
    exact ⟨retained, original.2⟩

/-- A position has no class member after pointwise narrowing exactly when it
had none before. This is a local semantic refutation law, not an executable
procedure for detecting emptiness and not a refutation of any other position.

Proof outline. A witness on either side transfers by `cohort_members_iff`,
contradicting the corresponding absence premise. -/
theorem cohort_refuted_iff {n : Nat} {classes : Fin n → Class α}
    (subjects : (i : Fin n) → Subject (classes i))
    (regions : Fin n → Region α) (i : Fin n) :
    (¬ ∃ z, ((subjects i).narrow (regions i)).Contains z ∧ classes i z) ↔
      (¬ ∃ z, (regions i).Contains z ∧ classes i z) := by
  constructor
  · intro noNarrowed original
    rcases original with ⟨z, witness⟩
    exact noNarrowed ⟨z, (cohort_members_iff subjects regions i z).mpr witness⟩
  · intro noOriginal narrowed
    rcases narrowed with ⟨z, witness⟩
    exact noOriginal ⟨z, (cohort_members_iff subjects regions i z).mp witness⟩

/-- Applying subjects to an empty indexed family leaves that family unchanged.
There is no position to visit. This creates neither an interpretation nor a
claim that a search is complete. -/
theorem cohort_empty (classes : Fin 0 → Class α)
    (subjects : (i : Fin 0) → Subject (classes i))
    (regions : Fin 0 → Region α) :
    (fun i => (subjects i).narrow (regions i)) = regions := by
  funext i
  exact Fin.elim0 i

end Subject

/-! ## The joint fixed point -/

variable {C : Class α}

/-- A region every subject of the family leaves unchanged. -/
def Closed (family : List (Subject C)) (r : Region α) : Prop :=
  ∀ s ∈ family, s.narrow r = r

/-- **The joint fixed point** of a family of subjects from a region `r`: it holds
what every closed region inside `r` holds and admits what some closed region
inside `r` admits. It is the meet, in the precision order, of the closed regions
inside `r`; `joint_closed` and `joint_least` make it the least closed region
inside `r`, the least common fixed point of the subjects above `r`. -/
def joint (family : List (Subject C)) (r : Region α) : Region α :=
  ⟨fun a => ∀ q, Closed family q → Inside q r → q.lower a,
   fun a => ∃ q, Closed family q ∧ Inside q r ∧ q.upper a⟩

/-- Every closed region inside `r` is inside the joint fixed point. -/
theorem joint_least {family : List (Subject C)} {r q : Region α}
    (closed : Closed family q) (inside : Inside q r) : Inside q (joint family r) :=
  ⟨fun _ held => held q closed inside, fun _ possible => ⟨q, closed, inside, possible⟩⟩

/-- The joint fixed point is the most precise region that every closed region
inside `r` lies inside: the meet's universal property. -/
theorem joint_most_precise_bound {family : List (Subject C)} {r b : Region α}
    (bound : ∀ q, Closed family q → Inside q r → Inside q b) : Inside (joint family r) b :=
  ⟨fun a held q closed inside => (bound q closed inside).1 a held,
   fun a ⟨q, closed, inside, possible⟩ => (bound q closed inside).2 a possible⟩

/-- **Extensive.** The joint fixed point lies inside its start: it only adds
decisions. -/
theorem joint_extensive (family : List (Subject C)) (r : Region α) :
    Inside (joint family r) r :=
  joint_most_precise_bound fun _ _ inside => inside

/-- **Closed.** Every subject of the family leaves the joint fixed point
unchanged.

Proof outline: a subject's narrowing of the joint fixed point lies inside it, since
the subject only adds decisions. Conversely, every closed region inside the start
lies inside the joint fixed point, so by monotonicity it lies inside the subject's
narrowing of it, as the subject leaves the closed region unchanged. Every closed
region inside the start thus lies inside the narrowing, and so does the joint
fixed point, their meet (`joint_most_precise_bound`). The two inclusions make
the narrowing equal to the joint fixed point. -/
theorem joint_closed (family : List (Subject C)) (r : Region α) :
    Closed family (joint family r) := by
  intro s listed
  have more : Inside (s.narrow (joint family r)) (joint family r) := s.extensive _
  have less : Inside (joint family r) (s.narrow (joint family r)) :=
    joint_most_precise_bound fun q closed inside => by
      have narrowed : Inside (s.narrow q) (s.narrow (joint family r)) :=
        s.monotone (joint_least closed inside)
      rwa [closed s listed] at narrowed
  exact inside_antisymm more less

/-- **Monotone.** A more precise start has a more precise joint fixed point. -/
theorem joint_monotone (family : List (Subject C)) {r r' : Region α} (inside : Inside r' r) :
    Inside (joint family r') (joint family r) :=
  joint_least (joint_closed family r') (inside_trans (joint_extensive family r') inside)

/-- A region the family leaves unchanged is its own joint fixed point. -/
theorem joint_of_closed {family : List (Subject C)} {q : Region α} (closed : Closed family q) :
    joint family q = q :=
  inside_antisymm (joint_extensive family q) (joint_least closed (inside_refl q))

/-- **Idempotent.** Narrowing a joint fixed point again changes nothing. With
`joint_extensive` and `joint_monotone`, the joint fixed point is a closure
operator on the region poset, that is, an idempotent monad on its thin category,
and the closed regions are its algebras. -/
theorem joint_idempotent (family : List (Subject C)) (r : Region α) :
    joint family (joint family r) = joint family r :=
  joint_of_closed (joint_closed family r)

/-- **More subjects never lose precision.** The joint fixed point of a larger
family lies inside that of a smaller one. -/
theorem joint_more_subjects {family more : List (Subject C)}
    (included : ∀ s ∈ family, s ∈ more) (r : Region α) :
    Inside (joint more r) (joint family r) :=
  joint_least (fun s listed => joint_closed more r s (included s listed)) (joint_extensive more r)

/-- **The ideal narrowing** of a region for a class: the most precise region holding
every member of the class in the region. It is the abstraction of the class's
members in the region under the Galois insertion between sets of interpretations
and regions. Computing it decides whether the region holds any member at all, so
subjects approximate it from outside (`Subject.narrow_ideal`). -/
def ideal (C : Class α) (r : Region α) : Region α :=
  ⟨fun a => ∀ z, r.Contains z → C z → z a, fun a => ∃ z, r.Contains z ∧ C z ∧ z a⟩

/-- Every member of the class in the region is a member of its ideal narrowing. -/
theorem ideal_contains {r : Region α} {z : Atoms α} (inside : r.Contains z) (member : C z) :
    (ideal C r).Contains z :=
  ⟨fun _ held => held z inside member, fun _ present => ⟨z, inside, member, present⟩⟩

/-- The ideal narrowing lies inside its region. -/
theorem ideal_inside (C : Class α) (r : Region α) : Inside (ideal C r) r :=
  ⟨fun a held _ inside _ => inside.1 a held, fun a ⟨_, inside, _, present⟩ => inside.2 a present⟩

/-- **No subject narrows past the ideal.** A sound subject's narrowing lies between
its region and the region's ideal narrowing. -/
theorem Subject.narrow_ideal (s : Subject C) (r : Region α) : Inside (ideal C r) (s.narrow r) :=
  ⟨fun a held _ inside member => (s.sound inside member).1 a held,
   fun a ⟨_, inside, member, present⟩ => (s.sound inside member).2 a present⟩

/-- Every family of subjects sound for the class leaves the ideal narrowing
unchanged: a subject keeps every member of the class, and the ideal narrowing is
already the most precise region that does. -/
theorem ideal_closed (family : List (Subject C)) (r : Region α) : Closed family (ideal C r) := by
  intro s _
  have more : Inside (s.narrow (ideal C r)) (ideal C r) := s.extensive _
  have members : ∀ z, r.Contains z → C z → (s.narrow (ideal C r)).Contains z :=
    fun _ inside member => s.sound (ideal_contains inside member) member
  have less : Inside (ideal C r) (s.narrow (ideal C r)) :=
    ⟨fun a held z inside member => (members z inside member).1 a held,
     fun a ⟨z, inside, member, present⟩ => (members z inside member).2 a present⟩
  exact inside_antisymm more less

/-- **Sound.** Every member of the class in a region is a member of its joint fixed
point: the ideal narrowing is closed and inside the region, so it lies inside the
joint fixed point. -/
theorem joint_sound (family : List (Subject C)) {r : Region α} {z : Atoms α}
    (inside : r.Contains z) (member : C z) : (joint family r).Contains z :=
  StreamedConsequences.contains_of_inside
    (joint_least (ideal_closed family r) (ideal_inside C r)) (ideal_contains inside member)

/-! ## Chaotic iteration -/

/-- **A chaotic iteration**: the family's subjects applied in any order, any number
of times. -/
inductive Reaches (family : List (Subject C)) : Region α → Region α → Prop
  | stay (r : Region α) : Reaches family r r
  | step {r p : Region α} (s : Subject C) (listed : s ∈ family)
      (rest : Reaches family (s.narrow r) p) : Reaches family r p

namespace Reaches

variable {family : List (Subject C)}

/-- An iteration only adds decisions. -/
theorem inside {r p : Region α} (reach : Reaches family r p) : Inside p r := by
  induction reach with
  | stay r => exact inside_refl r
  | step s _ _ ih => exact inside_trans ih (s.extensive _)

/-- **L1 for iterations.** Every member of the class in the start is a member of the
iteration's region. -/
theorem sound {r p : Region α} (reach : Reaches family r p) :
    ∀ {z : Atoms α}, r.Contains z → C z → p.Contains z := by
  induction reach with
  | stay r => exact fun inside _ => inside
  | step s _ _ ih => exact fun inside member => ih (s.sound inside member) member

/-- An iteration followed by one more of the family's subjects is an iteration. -/
theorem snoc {r p : Region α} (reach : Reaches family r p) {s : Subject C}
    (listed : s ∈ family) : Reaches family r (s.narrow p) := by
  induction reach with
  | stay r => exact .step s listed (.stay _)
  | step t listedT _ ih => exact .step t listedT ih

/-- **An iteration never passes a closed region.** Every closed region inside the
start lies inside every region of the iteration. -/
theorem below_closed {r p q : Region α} (reach : Reaches family r p)
    (closed : Closed family q) : Inside q r → Inside q p := by
  induction reach with
  | stay r => exact id
  | step s listed _ ih =>
    intro inside
    apply ih
    have narrowed : Inside (s.narrow q) (s.narrow _) := s.monotone inside
    rwa [closed s listed] at narrowed

/-- **L5, order independence.** A chaotic iteration that stops at a region every
subject leaves unchanged has reached the joint fixed point, whatever order applied
the subjects.

The family is fixed throughout. The solver's candidate subjects change during a
walk: learned rows arrive at tier boundaries, the bound tightens, restrictions are
added and optional allowances decline. Across such changes the answer sets stay
identical (L1 and L2), but regions and trees are identical only while the ledger
tier, the bound, the restrictions and the allowances are held fixed. -/
theorem eq_joint {r p : Region α} (reach : Reaches family r p) (closed : Closed family p) :
    p = joint family r :=
  inside_antisymm (joint_least closed reach.inside)
    (reach.below_closed (joint_closed family r) (joint_extensive family r))

/-- **L5, terminal agreement.** Two chaotic iterations of one fixed family from one
region that each stop at a region every subject leaves unchanged stop at the same
region. A refuted region is one without members, so both refute or neither does. -/
theorem terminal_agreement {r p q : Region α} (first : Reaches family r p)
    (firstClosed : Closed family p) (second : Reaches family r q) (secondClosed : Closed family q) :
    p = q :=
  (first.eq_joint firstClosed).trans (second.eq_joint secondClosed).symm

end Reaches

/-- Strict refinement: `r'` lies inside `r` and differs from it. -/
def Refines (r' r : Region α) : Prop := Inside r' r ∧ r' ≠ r

/-- The iterates of a schedule from a region. -/
def iterate (schedule : Nat → Subject C) (r : Region α) : Nat → Region α
  | 0 => r
  | n + 1 => (schedule n).narrow (iterate schedule r n)

/-- A schedule is **fair** for a family when it applies only the family's subjects
and applies each of them infinitely often. -/
def Fair (family : List (Subject C)) (schedule : Nat → Subject C) : Prop :=
  (∀ n, schedule n ∈ family) ∧ ∀ s ∈ family, ∀ n, ∃ k, n ≤ k ∧ schedule k = s

/-- A schedule's iterates are iterations of any family that holds its subjects. -/
theorem iterate_reaches {family : List (Subject C)} {schedule : Nat → Subject C}
    (uses : ∀ n, schedule n ∈ family) (r : Region α) :
    ∀ n, Reaches family r (iterate schedule r n)
  | 0 => .stay r
  | n + 1 => (iterate_reaches uses r n).snoc (uses n)

/-- A later iterate lies inside an earlier one. -/
theorem iterate_inside {schedule : Nat → Subject C} {r : Region α} {n m : Nat} (le : n ≤ m) :
    Inside (iterate schedule r m) (iterate schedule r n) := by
  induction le with
  | refl => exact inside_refl _
  | @step m _ ih => exact inside_trans ((schedule m).extensive (iterate schedule r m)) ih

/-- **Under a well-founded refinement, a schedule's iterates stop changing.**

Proof outline: by well-founded induction on the region reached. If every later
iterate equals the current one, it has stopped. Otherwise some later iterate
differs; it lies inside the current one, so it strictly refines it, and the
induction hypothesis stops the iterates from there. -/
theorem iterate_stabilizes (wf : WellFounded (fun r' r : Region α => Refines r' r))
    (schedule : Nat → Subject C) (r : Region α) :
    ∃ K, ∀ k, K ≤ k → iterate schedule r k = iterate schedule r K := by
  classical
  have claim : ∀ y : Region α, ∀ n, iterate schedule r n = y →
      ∃ K, n ≤ K ∧ ∀ k, K ≤ k → iterate schedule r k = iterate schedule r K := by
    intro y
    refine wf.induction (C := fun y => ∀ n, iterate schedule r n = y →
      ∃ K, n ≤ K ∧ ∀ k, K ≤ k → iterate schedule r k = iterate schedule r K) y ?_
    intro x ih n atX
    by_cases settled : ∀ k, n ≤ k → iterate schedule r k = iterate schedule r n
    · exact ⟨n, Nat.le_refl n, settled⟩
    · have later : ∃ k, n ≤ k ∧ iterate schedule r k ≠ iterate schedule r n :=
        Classical.byContradiction fun none =>
          settled fun k le => Classical.byContradiction fun differs => none ⟨k, le, differs⟩
      obtain ⟨k, le, differs⟩ := later
      have refines : Refines (iterate schedule r k) x := by
        rw [← atX]
        exact ⟨iterate_inside le, differs⟩
      obtain ⟨K, kLe, stable⟩ := ih (iterate schedule r k) refines k rfl
      exact ⟨K, Nat.le_trans le kLe, stable⟩
  obtain ⟨K, _, stable⟩ := claim (iterate schedule r 0) 0 rfl
  exact ⟨K, stable⟩

/-- **L5, a fair iteration reaches the joint fixed point.** When strict refinement is
well founded, as it is over finitely many atoms (`refines_wf`), the iterates of
every fair schedule are eventually the joint fixed point: whatever order applies the
subjects, the iteration ends at the same region.

Proof outline: the iterates stop changing (`iterate_stabilizes`). Fairness applies
every subject after that point, so the final region is closed, and an iteration
that stops at a closed region has reached the joint fixed point
(`Reaches.eq_joint`). -/
theorem fair_iteration_reaches_joint (wf : WellFounded (fun r' r : Region α => Refines r' r))
    {family : List (Subject C)} {schedule : Nat → Subject C} (fair : Fair family schedule)
    (r : Region α) : ∃ K, ∀ k, K ≤ k → iterate schedule r k = joint family r := by
  obtain ⟨K, stable⟩ := iterate_stabilizes wf schedule r
  have closed : Closed family (iterate schedule r K) := by
    intro s listed
    obtain ⟨k, le, chosen⟩ := fair.2 s listed K
    have atK : iterate schedule r k = iterate schedule r K := stable k le
    have next : iterate schedule r (k + 1) = iterate schedule r K :=
      stable (k + 1) (Nat.le_succ_of_le le)
    calc s.narrow (iterate schedule r K)
        = (schedule k).narrow (iterate schedule r k) := by rw [chosen, atK]
      _ = iterate schedule r (k + 1) := rfl
      _ = iterate schedule r K := next
  have atJoint : iterate schedule r K = joint family r :=
    (iterate_reaches fair.1 r K).eq_joint closed
  exact ⟨K, fun k le => (stable k le).trans atJoint⟩

/-! ## Finitely many atoms -/

/-- For two Boolean predicates on a list: if `p` implies `q` at every element and `q`
holds without `p` at some element, `q` counts strictly more elements. -/
theorem countP_lt {β : Type v} {p q : β → Bool} :
    ∀ (l : List β), (∀ x ∈ l, p x = true → q x = true) →
      (∃ x ∈ l, q x = true ∧ p x = false) → l.countP p < l.countP q
  | [], _, ⟨_, member, _⟩ => nomatch member
  | y :: ys, imp, ⟨x, member, qx, px⟩ => by
    have impRest : ∀ x ∈ ys, p x = true → q x = true :=
      fun x listed => imp x (List.mem_cons_of_mem y listed)
    have witnessRest : x ≠ y → ∃ x ∈ ys, q x = true ∧ p x = false := fun distinct => by
      rcases List.mem_cons.mp member with same | rest
      · exact absurd same distinct
      · exact ⟨x, rest, qx, px⟩
    cases hp : p y with
    | true =>
      have hq : q y = true := imp y (List.mem_cons_self ..) hp
      have distinct : x ≠ y := fun same => by
        rw [same, hp] at px
        cases px
      rw [List.countP_cons_of_pos hp, List.countP_cons_of_pos hq]
      exact Nat.succ_lt_succ (countP_lt ys impRest (witnessRest distinct))
    | false =>
      have notP : ¬ p y = true := by rw [hp]; decide
      rw [List.countP_cons_of_neg notP]
      cases hq : q y with
      | true =>
        rw [List.countP_cons_of_pos hq]
        exact Nat.lt_succ_of_le (List.countP_mono_left impRest)
      | false =>
        have notQ : ¬ q y = true := by rw [hq]; decide
        have distinct : x ≠ y := fun same => by
          rw [same, hq] at qx
          cases qx
        rw [List.countP_cons_of_neg notQ]
        exact countP_lt ys impRest (witnessRest distinct)

open Classical in
/-- The atoms of a list a region does not hold. -/
noncomputable def unheld (atoms : List α) (r : Region α) : Nat :=
  atoms.countP fun a => decide (¬ r.lower a)

open Classical in
/-- The atoms of a list a region admits. -/
noncomputable def admitted (atoms : List α) (r : Region α) : Nat :=
  atoms.countP fun a => decide (r.upper a)

/-- **The openness** of a region over a list of atoms: the atoms it does not hold
plus the atoms it admits. -/
noncomputable def openness (atoms : List α) (r : Region α) : Nat :=
  unheld atoms r + admitted atoms r

/-- A region inside another holds at least the other's atoms. -/
theorem unheld_le (atoms : List α) {r r' : Region α} (inside : Inside r' r) :
    unheld atoms r' ≤ unheld atoms r := by
  unfold unheld
  exact List.countP_mono_left fun a _ notHeld => by
    simp only [decide_eq_true_eq] at notHeld ⊢
    exact fun held => notHeld (inside.1 a held)

/-- A region inside another admits at most the other's atoms. -/
theorem admitted_le (atoms : List α) {r r' : Region α} (inside : Inside r' r) :
    admitted atoms r' ≤ admitted atoms r := by
  unfold admitted
  exact List.countP_mono_left fun a _ possible => by
    simp only [decide_eq_true_eq] at possible ⊢
    exact inside.2 a possible

/-- **A strict refinement lowers the openness** over a list holding every atom: it
holds an atom the region did not, or cuts an atom the region admitted, and decides
nothing back. -/
theorem openness_lt (atoms : List α) (every : ∀ a, a ∈ atoms) {r r' : Region α}
    (refines : Refines r' r) : openness atoms r' < openness atoms r := by
  classical
  obtain ⟨inside, differs⟩ := refines
  have decided : ∃ a, (r'.lower a ∧ ¬ r.lower a) ∨ (r.upper a ∧ ¬ r'.upper a) :=
    Classical.byContradiction fun none => differs <| inside_antisymm inside
      ⟨fun a held => Classical.byContradiction fun notHeld => none ⟨a, Or.inl ⟨held, notHeld⟩⟩,
       fun a possible => Classical.byContradiction fun cut => none ⟨a, Or.inr ⟨possible, cut⟩⟩⟩
  obtain ⟨a, side⟩ := decided
  have unheldLe := unheld_le atoms inside
  have admittedLe := admitted_le atoms inside
  unfold openness
  rcases side with ⟨held, notHeld⟩ | ⟨possible, cut⟩
  · have strict : unheld atoms r' < unheld atoms r := by
      unfold unheld
      refine countP_lt atoms (fun b _ notHeld' => ?_) ⟨a, every a, ?_, ?_⟩
      · simp only [decide_eq_true_eq] at notHeld' ⊢
        exact fun held' => notHeld' (inside.1 b held')
      · simp only [decide_eq_true_eq]
        exact notHeld
      · simp only [decide_eq_false_iff_not, Classical.not_not]
        exact held
    omega
  · have strict : admitted atoms r' < admitted atoms r := by
      unfold admitted
      refine countP_lt atoms (fun b _ possible' => ?_) ⟨a, every a, ?_, ?_⟩
      · simp only [decide_eq_true_eq] at possible' ⊢
        exact inside.2 b possible'
      · simp only [decide_eq_true_eq]
        exact possible
      · simp only [decide_eq_false_iff_not]
        exact cut
    omega

/-- **Over finitely many atoms, refinement is well founded.** When a list holds every
atom, a region can be strictly refined only finitely often, so the lattice of
regions has no infinite ascending chain in the precision order. -/
theorem refines_wf (atoms : List α) (every : ∀ a, a ∈ atoms) :
    WellFounded (fun r' r : Region α => Refines r' r) :=
  Subrelation.wf (fun refines => openness_lt atoms every refines)
    (InvImage.wf (openness atoms) Nat.lt_wfRel.wf)

/-- **L5 over finitely many atoms**: every fair iteration reaches the joint fixed
point. -/
theorem fair_iteration_reaches_joint_of_atoms (atoms : List α) (every : ∀ a, a ∈ atoms)
    {family : List (Subject C)} {schedule : Nat → Subject C} (fair : Fair family schedule)
    (r : Region α) : ∃ K, ∀ k, K ≤ k → iterate schedule r k = joint family r :=
  fair_iteration_reaches_joint (refines_wf atoms every) fair r

/-! ## Subjects from the library's laws -/

/-- Some formula is known both to hold and to fail under the region. -/
def Contradicted (U : Theory α) (r : Region α) : Prop :=
  ∃ F, Known U r F true ∧ Known U r F false

/-- **The readings subject** of a theory `U`: it holds the atoms `U`'s knowledge
under the region says hold (`FormulaBounds.Known`), cuts the atoms it says fail,
and refutes the region when some formula is known both ways. It is sound for the
classical models of `U` by `known_sound`, and monotone by `known_mono`. -/
def readings (U : Theory α) : Subject (ModelsOf U) where
  narrow r :=
    ⟨fun a => r.lower a ∨ Known U r (.atom a) true ∨ Contradicted U r,
     fun a => r.upper a ∧ ¬ Known U r (.atom a) false ∧ ¬ Contradicted U r⟩
  extensive _ := ⟨fun _ held => Or.inl held, fun _ possible => possible.1⟩
  monotone := by
    intro r r' inside
    refine ⟨fun a held => ?_, fun a possible => ?_⟩
    · rcases held with held | known | ⟨F, sure, never⟩
      · exact Or.inl (inside.1 a held)
      · exact Or.inr (Or.inl (known_mono U inside known))
      · exact Or.inr (Or.inr ⟨F, known_mono U inside sure, known_mono U inside never⟩)
    · obtain ⟨possible, notCut, consistent⟩ := possible
      refine ⟨inside.2 a possible, fun known => notCut (known_mono U inside known), ?_⟩
      exact fun ⟨F, sure, never⟩ =>
        consistent ⟨F, known_mono U inside sure, known_mono U inside never⟩
  sound := by
    intro r z inside models
    refine ⟨fun a held => ?_, fun a present => ?_⟩
    · rcases held with held | known | ⟨F, sure, never⟩
      · exact inside.1 a held
      · exact (known_sound U r inside models known).1 rfl
      · exact absurd ((known_sound U r inside models sure).1 rfl)
          ((known_sound U r inside models never).2 rfl)
    · refine ⟨inside.2 a present, fun known => (known_sound U r inside models known).2 rfl present, ?_⟩
      exact fun ⟨F, sure, never⟩ =>
        (known_sound U r inside models never).2 rfl ((known_sound U r inside models sure).1 rfl)

/-- **The original subject**: the readings of the original theory, a subject of the
answer sets sought, since every stable model is a classical model. -/
def original (T R : Theory α) : Subject (AnswerSetsSought T R) :=
  (readings T).toNarrower fun _ sought => sought.1.1

/-- **The restriction subject**: the readings of the restrictions, the objective
bound among them, without producers (`FormulaBounds.restricted_stable_narrowing`). -/
def restriction (T R : Theory α) : Subject (AnswerSetsSought T R) :=
  (readings R).toNarrower fun _ sought => sought.2

/-- **The frozen subject** of a candidate `M`: the readings of `T`'s frozen reduct
under `M`, a subject of the countermodels. It is built from `T` and `M` alone. The
Rust narrower propagates under the candidate's frozen mask instead of building the
reduct; `ReductRegions.masked_known_sound` says that propagation knows only what
these readings know. -/
noncomputable def frozen (T : Theory α) (M : Atoms α) : Subject (Countermodels T M) :=
  (readings (ReductTheory M T)).toNarrower fun _ countermodel => countermodel.2

/-- A region decides a literal in every region inside it. -/
theorem decides_mono {r r' : Region α} (inside : Inside r' r) {l : Literal α}
    (decided : Decides r l) : Decides r' l := by
  rcases l with ⟨a, _ | _⟩
  · exact fun possible => decided (inside.2 a possible)
  · exact inside.1 a decided

/-- **The row subject** of a row of a class: once a region decides every condition,
it holds the outputs required present and cuts those required absent
(`LearnedRegions.consequence_row_narrows`). Learned rows and the rows of algebraic
forcing (`AlgebraicRows.equation_forces_true`, `equation_forces_false`) are its
instances. A row is a subject of its own class and of narrower ones only: a row of
the answer sets sought is not a subject of a candidate's countermodels. -/
def row {C : Class α} (B O : List (Literal α)) (of : RowOf C B O) : Subject C where
  narrow r :=
    ⟨fun a => r.lower a ∨ ((∀ l ∈ B, Decides r l) ∧ required O a),
     fun a => r.upper a ∧ ((∀ l ∈ B, Decides r l) → permitted O a)⟩
  extensive _ := ⟨fun _ held => Or.inl held, fun _ possible => possible.1⟩
  monotone := by
    intro r r' inside
    have decidedMono : (∀ l ∈ B, Decides r l) → ∀ l ∈ B, Decides r' l :=
      fun decided l listed => decides_mono inside (decided l listed)
    refine ⟨fun a held => ?_, fun a possible => ?_⟩
    · rcases held with held | ⟨decided, req⟩
      · exact Or.inl (inside.1 a held)
      · exact Or.inr ⟨decidedMono decided, req⟩
    · exact ⟨inside.2 a possible.1, fun decided => possible.2 (decidedMono decided)⟩
  sound := by
    intro r z inside member
    refine ⟨fun a held => ?_, fun a present => ⟨inside.2 a present, fun decided => ?_⟩⟩
    · rcases held with held | ⟨decided, req⟩
      · exact inside.1 a held
      · exact (Region.contains_narrow.mp
          (consequence_row_narrows C B O of r decided z inside member)).2.1 a req
    · exact (Region.contains_narrow.mp
        (consequence_row_narrows C B O of r decided z inside member)).2.2 a present

/-- **A refuting subject**: it refutes every region a condition marks, narrowing it
to a region with no members, and leaves every other region unchanged. The mark must
persist into the regions inside a marked one, and exclude the class from a marked
region. -/
def refuting {C : Class α} (Marks : Region α → Prop)
    (persists : ∀ {r r' : Region α}, Inside r' r → Marks r → Marks r')
    (excludes : ∀ {r : Region α} {z : Atoms α}, Marks r → r.Contains z → ¬ C z) :
    Subject C where
  narrow r := ⟨fun a => r.lower a ∨ Marks r, fun a => r.upper a ∧ ¬ Marks r⟩
  extensive _ := ⟨fun _ held => Or.inl held, fun _ possible => possible.1⟩
  monotone := by
    intro r r' inside
    refine ⟨fun a held => ?_, fun a possible => ?_⟩
    · rcases held with held | marked
      · exact Or.inl (inside.1 a held)
      · exact Or.inr (persists inside marked)
    · exact ⟨inside.2 a possible.1, fun marked => possible.2 (persists inside marked)⟩
  sound := by
    intro r z inside member
    refine ⟨fun a held => ?_, fun a present => ⟨inside.2 a present, fun marked => ?_⟩⟩
    · rcases held with held | marked
      · exact inside.1 a held
      · exact absurd member (excludes marked inside)
    · exact excludes marked inside member

/-- **The exclusion subject** of conditions that exclude a class, the refutation of an
algebraic row (`AlgebraicRows.equation_excludes`): it refutes every region deciding
the conditions (`AlgebraicRows.excludes_region`). -/
def exclusion {C : Class α} (B : List (Literal α)) (excluded : AlgebraicRows.Excludes C B) :
    Subject C :=
  refuting (fun r => ∀ l ∈ B, Decides r l)
    (fun inside decided l listed => decides_mono inside (decided l listed))
    (fun decided inside member =>
      AlgebraicRows.excludes_region C B excluded _ decided _ inside member)

/-- **The double-count subject** of an ordered commutative monoid: it refutes every
region in which two families of rows covering the same terms have lower bounds
whose fold does not lie below their upper bounds' fold
(`AlgebraicRows.double_count_excludes_region`), the pigeonhole principle refuted by
one comparison. The bounds of a region hold in every region inside it. -/
def doubleCount {V : Type v} (o : AlgebraicRows.OrderedReduction V) (C : Class α) : Subject C :=
  refuting
    (fun r => ∃ (S : List (AlgebraicRows.Term α V))
        (first second : List (List (AlgebraicRows.Term α V) × V)),
      (first.map Prod.fst).flatten.Perm S ∧ (second.map Prod.fst).flatten.Perm S ∧
      (∀ row ∈ first, AlgebraicRows.LowerIn o C r row.1 row.2) ∧
      (∀ row ∈ second, AlgebraicRows.UpperIn o C r row.1 row.2) ∧
      ¬ o.le (o.toReduction.fold Prod.snd first) (o.toReduction.fold Prod.snd second))
    (by
      intro r r' inside ⟨S, first, second, firstCovers, secondCovers, lower, upper, exceeds⟩
      exact ⟨S, first, second, firstCovers, secondCovers,
        fun row listed z within member =>
          lower row listed z (StreamedConsequences.contains_of_inside inside within) member,
        fun row listed z within member =>
          upper row listed z (StreamedConsequences.contains_of_inside inside within) member,
        exceeds⟩)
    (by
      intro r z ⟨S, first, second, firstCovers, secondCovers, lower, upper, exceeds⟩ inside member
      exact AlgebraicRows.double_count_excludes_region o C r S first second firstCovers
        secondCovers lower upper exceeds z inside member)

/-! ## Guarded counting bounds

These are values of the existing counting reduction, not a second subject
algebra. A signed guard activates a bound only when every literal is decided.
The fallback is the held or possible measure of the same terms.
-/

section GuardedCounting

open AlgebraicRows

open Classical in
/-- A guarded lower bound, strengthened by the atoms already held. -/
noncomputable def guardedLower (r : Region α) (terms : List (Term α Nat))
    (guard : List (Literal α)) (bound : Nat) : Nat :=
  if ∀ l ∈ guard, Decides r l then
    max bound (measure counting.toReduction r.lower terms)
  else measure counting.toReduction r.lower terms

open Classical in
/-- A guarded upper bound, capped by the atoms still possible. -/
noncomputable def guardedUpper (r : Region α) (terms : List (Term α Nat))
    (guard : List (Literal α)) (bound : Nat) : Nat :=
  if ∀ l ∈ guard, Decides r l then
    min bound (measure counting.toReduction r.upper terms)
  else measure counting.toReduction r.upper terms

/-- The effective lower bound holds for every member of the class in the
region, provided the original guarded bound holds for that class.

Proof. Held atoms already give a lower bound. When the guard is decided,
every contained interpretation satisfies it, so the original bound holds too;
the maximum of these two lower bounds remains a lower bound. -/
theorem guardedLower_sound (C : Class α) (r : Region α)
    (terms : List (Term α Nat)) (guard : List (Literal α)) (bound : Nat)
    (guarded : ∀ z, C z → (∀ l ∈ guard, Holds z l) →
      bound ≤ measure counting.toReduction z terms) :
    LowerIn counting C r terms (guardedLower r terms guard bound) := by
  classical
  intro z within member
  change guardedLower r terms guard bound ≤ measure counting.toReduction z terms
  have held : measure counting.toReduction r.lower terms ≤
      measure counting.toReduction z terms :=
    measure_mono counting within.1 terms (fun t _ => Nat.zero_le t.2)
  by_cases active : ∀ l ∈ guard, Decides r l
  · have released : bound ≤ measure counting.toReduction z terms :=
      guarded z member (fun l listed => decides_holds within (active l listed))
    simpa only [guardedLower, if_pos active] using (Nat.max_le.mpr ⟨released, held⟩)
  · simpa only [guardedLower, if_neg active] using held

/-- The effective upper bound holds for every member of the class in the
region, provided the original guarded bound holds for that class.

Proof. Possible atoms give an upper bound. A decided guard also releases the
original upper bound; the minimum of the two is still an upper bound. -/
theorem guardedUpper_sound (C : Class α) (r : Region α)
    (terms : List (Term α Nat)) (guard : List (Literal α)) (bound : Nat)
    (guarded : ∀ z, C z → (∀ l ∈ guard, Holds z l) →
      measure counting.toReduction z terms ≤ bound) :
    UpperIn counting C r terms (guardedUpper r terms guard bound) := by
  classical
  intro z within member
  change measure counting.toReduction z terms ≤ guardedUpper r terms guard bound
  have possible : measure counting.toReduction z terms ≤
      measure counting.toReduction r.upper terms :=
    measure_mono counting within.2 terms (fun t _ => Nat.zero_le t.2)
  by_cases active : ∀ l ∈ guard, Decides r l
  · have released : measure counting.toReduction z terms ≤ bound :=
      guarded z member (fun l listed => decides_holds within (active l listed))
    simpa only [guardedUpper, if_pos active] using (Nat.le_min.mpr ⟨released, possible⟩)
  · simpa only [guardedUpper, if_neg active] using possible

/-- Refining a region cannot lower an effective guarded lower bound.

Proof. The held measure grows. An active guard remains active. A newly active
guard can only replace the new held measure by a greater maximum. -/
theorem guardedLower_mono {r r' : Region α} (inside : Inside r' r)
    (terms : List (Term α Nat)) (guard : List (Literal α)) (bound : Nat) :
    guardedLower r terms guard bound ≤ guardedLower r' terms guard bound := by
  classical
  have grows : measure counting.toReduction r.lower terms ≤
      measure counting.toReduction r'.lower terms :=
    measure_mono counting inside.1 terms (fun t _ => Nat.zero_le t.2)
  by_cases active : ∀ l ∈ guard, Decides r l
  · have persists : ∀ l ∈ guard, Decides r' l :=
      fun l listed => decides_mono inside (active l listed)
    simpa only [guardedLower, if_pos active, if_pos persists] using
      (Nat.max_le.mpr ⟨Nat.le_max_left ..,
        Nat.le_trans grows (Nat.le_max_right ..)⟩)
  · by_cases later : ∀ l ∈ guard, Decides r' l
    · simpa only [guardedLower, if_neg active, if_pos later] using
        (Nat.le_trans grows (Nat.le_max_right bound
          (measure counting.toReduction r'.lower terms)))
    · simpa only [guardedLower, if_neg active, if_neg later] using grows

/-- Refining a region cannot raise an effective guarded upper bound.

Proof. The possible measure shrinks. An active guard remains active. A newly
active guard can only replace the new possible measure by a smaller minimum. -/
theorem guardedUpper_antitone {r r' : Region α} (inside : Inside r' r)
    (terms : List (Term α Nat)) (guard : List (Literal α)) (bound : Nat) :
    guardedUpper r' terms guard bound ≤ guardedUpper r terms guard bound := by
  classical
  have shrinks : measure counting.toReduction r'.upper terms ≤
      measure counting.toReduction r.upper terms :=
    measure_mono counting inside.2 terms (fun t _ => Nat.zero_le t.2)
  by_cases active : ∀ l ∈ guard, Decides r l
  · have persists : ∀ l ∈ guard, Decides r' l :=
      fun l listed => decides_mono inside (active l listed)
    simpa only [guardedUpper, if_pos active, if_pos persists] using
      (Nat.le_min.mpr ⟨Nat.min_le_left ..,
        Nat.le_trans (Nat.min_le_right ..) shrinks⟩)
  · by_cases later : ∀ l ∈ guard, Decides r' l
    · simpa only [guardedUpper, if_neg active, if_pos later] using
        (Nat.le_trans (Nat.min_le_right bound
          (measure counting.toReduction r'.upper terms)) shrinks)
    · simpa only [guardedUpper, if_neg active, if_neg later] using shrinks

end GuardedCounting

/-! ## Folds over walks that may stop -/

/-- **The verdict of a fold over a walk that may stop.** A walk's reduction decides
early only when its value reaches an absorbing element, and then names the members
that decided it. Otherwise it decides only when the walk is exhausted: the fold of
a family with members, or `empty` for a family without members, which is never the
reduction's unit. A stopped walk that did not absorb is `undecided` and establishes
no value. The reduction itself stays the monoid, `AxisFolds.Reduction`. -/
inductive FoldVerdict (V : Type v) (M : Type u) where
  | absorbed (value : V) (witnesses : List M)
  | complete (value : V)
  | empty
  | undecided

/-- The verdict of folding a walk's emitted members, given a test for the
reduction's absorbing elements and whether the walk was exhausted. -/
def foldVerdict {V : Type v} {M : Type u} (r : AxisFolds.Reduction V) (absorbs : V → Bool)
    (value : M → V) (emitted : List M) (exhausted : Bool) : FoldVerdict V M :=
  if absorbs (r.fold value emitted) && !emitted.isEmpty then
    .absorbed (r.fold value emitted) emitted
  else if exhausted then
    if emitted.isEmpty then .empty else .complete (r.fold value emitted)
  else .undecided

section FoldVerdicts

variable {V : Type v} {M : Type u} (r : AxisFolds.Reduction V) (absorbs : V → Bool)
  (value : M → V) (emitted : List M) (exhausted : Bool)

/-- **L4, an early decision is final.** When the verdict is `absorbed`, every
continuation of the walk folds to the same value, whatever it emits next. This
needs only that the members come from one walk, which emits each leaf once
(`AxisFolds.walk_exhausted_fold`). Over a world view, where members may repeat,
`AxisFolds.world_view_fold_decided` asks for an idempotent reduction instead, so a
count must fold distinct members there. -/
theorem foldVerdict_absorbed (absorbing : ∀ v, absorbs v = true → r.Absorbing v)
    {v : V} {witnesses : List M}
    (verdict : foldVerdict r absorbs value emitted exhausted = .absorbed v witnesses) :
    ∀ rest, r.fold value (emitted ++ rest) = v := by
  unfold foldVerdict at verdict
  split at verdict
  · rename_i decided
    cases verdict
    have absorbed : absorbs (r.fold value emitted) = true := (Bool.and_eq_true _ _ ▸ decided).1
    exact r.fold_decided_prefix value (absorbing _ absorbed) rfl
  · split at verdict
    · split at verdict <;> cases verdict
    · cases verdict

/-- **L4, otherwise only on completion.** A value that did not come from absorption
comes from an exhausted walk with at least one member, and is the fold of its
members. -/
theorem foldVerdict_complete {v : V}
    (verdict : foldVerdict r absorbs value emitted exhausted = .complete v) :
    exhausted = true ∧ emitted ≠ [] ∧ v = r.fold value emitted := by
  unfold foldVerdict at verdict
  split at verdict
  · cases verdict
  · split at verdict
    · rename_i done
      split at verdict
      · cases verdict
      · rename_i nonempty
        cases verdict
        exact ⟨done, fun none => nonempty (List.isEmpty_iff.mpr none), rfl⟩
    · cases verdict

/-- An exhausted walk without members gives `empty`, never the reduction's unit. -/
theorem foldVerdict_empty
    (verdict : foldVerdict r absorbs value emitted exhausted = .empty) :
    exhausted = true ∧ emitted = [] := by
  unfold foldVerdict at verdict
  split at verdict
  · cases verdict
  · split at verdict
    · rename_i done
      split at verdict
      · rename_i none
        exact ⟨done, List.isEmpty_iff.mp none⟩
      · cases verdict
    · cases verdict

/-- A walk whose verdict is undecided was stopped. -/
theorem foldVerdict_undecided
    (verdict : foldVerdict r absorbs value emitted exhausted = .undecided) :
    exhausted = false := by
  unfold foldVerdict at verdict
  split at verdict
  · cases verdict
  · split at verdict
    · split at verdict <;> cases verdict
    · rename_i stopped
      simpa using stopped

/-- **A reduction without an absorbing element decides only on completion.**
Optimization's `min` is such a reduction in general: its verdict is `complete`,
`empty` or `undecided`, so a verified incumbent of a stopped walk is not a proved
optimum. -/
theorem foldVerdict_without_absorber (never : ∀ v, absorbs v = false) (v : V)
    (witnesses : List M) : foldVerdict r absorbs value emitted exhausted ≠ .absorbed v witnesses := by
  intro verdict
  unfold foldVerdict at verdict
  split at verdict
  · rename_i decided
    rw [never] at decided
    cases decided
  · split at verdict
    · split at verdict <;> cases verdict
    · cases verdict

end FoldVerdicts

/-! ## Walks decide their leaves -/

/-- **A leaf verdict** for a candidate `z` of a walk of class `C`: a candidate is a
decided region no subject refutes, and the walk's leaf decision accepts it as a
member of `C`, refutes it, or leaves it undecided, when the decision stopped. -/
inductive LeafVerdict (C : Class α) (z : Atoms α) : Type where
  | accepted (member : C z)
  | refuted (excluded : ¬ C z)
  | undecided

namespace LeafVerdict

variable {C : Class α} {z : Atoms α}

/-- The verdict accepts its candidate. -/
def Accepts : LeafVerdict C z → Prop
  | .accepted _ => True
  | .refuted _ => False
  | .undecided => False

/-- The certificate leaf a decided verdict gives: an accepted or a refuted
singleton. An undecided verdict gives none, so a stopped leaf decision certifies
neither acceptance nor exhaustion. With these leaves, `CoverageTree.mem_outputs_iff`
(L2) says the completed walk's accepted leaves are exactly the members of `C` in
its root: L2 is a statement about accepted leaves, not about candidates. -/
def node : LeafVerdict C z → Option (CoverageTree C (Region.singleton z))
  | .accepted member => some (.accepted z member)
  | .refuted excluded =>
    some (.refuted (Region.singleton z) fun _ inside member =>
      excluded ((Region.contains_singleton.mp inside) ▸ member))
  | .undecided => none

end LeafVerdict

open AxisFolds in
/-- The `any` fold of a list of `true` values is `false` exactly when the list is
empty. -/
theorem any_true_eq_false_iff {β : Type v} (members : List β) :
    any.fold (fun _ => true) members = false ↔ members = [] := by
  cases members with
  | nil => exact ⟨fun _ => rfl, fun _ => rfl⟩
  | cons head rest =>
    have absorbed : any.fold (fun _ => true) (head :: rest) = true := by
      rw [any.fold_cons]
      rfl
    constructor
    · intro isFalse
      rw [absorbed] at isFalse
      cases isFalse
    · intro isEmpty
      cases isEmpty

/-- **The inner verdict (L3).** A classical model `M` of `T` is stable exactly when the
`any` fold over the accepted leaves of its countermodel walk is `false`, whichever
subjects of the countermodels narrowed the walk. -/
theorem inner_verdict (T : Theory α) (M : Atoms α) (original : Models M T)
    (inner : CoverageTree (Countermodels T M) (ReductRegions.query M)) :
    AxisFolds.any.fold (fun _ => true) inner.outputs = false ↔ Stable M T :=
  (any_true_eq_false_iff inner.outputs).trans
    (ReductRegions.stable_iff_no_countermodel M T original inner).symm

/-- **The countermodel walk's leaf decision.** A candidate `J` of the inner walk, a
model of the frozen reduct inside `M`, validated independently, is accepted as a
countermodel unless it is `M` itself, which models its own reduct and is no
countermodel. The decision depends on `T` and `M` alone. -/
noncomputable def countermodelVerdict (T : Theory α) (M J : Atoms α)
    (models : Models J (ReductTheory M T)) (inside : Sub J M) : LeafVerdict (Countermodels T M) J :=
  open Classical in
  if whole : Sub M J then .refuted fun countermodel => countermodel.1.2 whole
  else .accepted ⟨⟨inside, whole⟩, models⟩

/-- The countermodel walk's leaf decision accepts exactly the countermodels. -/
theorem countermodelVerdict_accepts (T : Theory α) (M J : Atoms α)
    (models : Models J (ReductTheory M T)) (inside : Sub J M) :
    (countermodelVerdict T M J models inside).Accepts ↔ Countermodels T M J := by
  unfold countermodelVerdict
  split
  · rename_i whole
    exact ⟨False.elim, fun countermodel => countermodel.1.2 whole⟩
  · rename_i proper
    exact ⟨fun _ => ⟨⟨inside, proper⟩, models⟩, fun _ => trivial⟩

/-- **The candidate walk's leaf decision is the inner walk.** A candidate `M`, a
classical model of `T` and the restrictions, is decided by its countermodel walk:
a completed walk accepts it when the `any` fold over its accepted leaves is `false`
and refutes it otherwise, and a stopped walk, `none`, leaves it undecided. -/
def answerSetVerdict (T R : Theory α) (M : Atoms α) (modelsT : Models M T) (modelsR : Models M R) :
    Option (CoverageTree (Countermodels T M) (ReductRegions.query M)) →
      LeafVerdict (AnswerSetsSought T R) M
  | none => .undecided
  | some inner =>
    if verdict : AxisFolds.any.fold (fun _ => true) inner.outputs = false then
      .accepted ⟨(inner_verdict T M modelsT inner).mp verdict, modelsR⟩
    else
      .refuted fun sought => verdict ((inner_verdict T M modelsT inner).mpr sought.1)

/-- **The answer sets are a nested walk.** A completed countermodel walk decides its
candidate exactly: the candidate walk's leaf decision accepts it if and only if it
is an answer set sought. The outer walk's accepted leaves are then exactly the
answer sets sought in its root (`answer_sets_nested`), each accepted by its own
inner walk's `any` verdict (`inner_verdict`). -/
theorem answerSetVerdict_accepts (T R : Theory α) (M : Atoms α) (modelsT : Models M T)
    (modelsR : Models M R) (inner : CoverageTree (Countermodels T M) (ReductRegions.query M)) :
    (answerSetVerdict T R M modelsT modelsR (some inner)).Accepts ↔ AnswerSetsSought T R M := by
  simp only [answerSetVerdict]
  split
  · rename_i verdict
    exact ⟨fun _ => ⟨(inner_verdict T M modelsT inner).mp verdict, modelsR⟩, fun _ => trivial⟩
  · rename_i verdict
    exact ⟨False.elim, fun sought => verdict ((inner_verdict T M modelsT inner).mpr sought.1)⟩

/-- **The definition at the outermost level (L2 with L3).** The accepted leaves of a
completed candidate walk, whose leaves the inner walks decided, are exactly the
answer sets sought in its root: the stable models of the theory that satisfy the
restrictions. -/
theorem answer_sets_nested (T R : Theory α) (root : Region α)
    (outer : CoverageTree (AnswerSetsSought T R) root) (M : Atoms α) :
    M ∈ outer.outputs ↔ root.Contains M ∧ Stable M T ∧ Models M R :=
  outer.mem_outputs_iff M

/-- A countermodel walk lists no accepted leaf exactly when the candidate has no
countermodel. -/
theorem countermodel_outputs_empty_iff (T : Theory α) (M : Atoms α)
    (inner : CoverageTree (Countermodels T M) (ReductRegions.query M)) :
    inner.outputs = [] ↔ ∀ J, ¬ Countermodels T M J := by
  constructor
  · intro empty J countermodel
    have listed : J ∈ inner.outputs := (ReductRegions.countermodels_exact M T inner J).mpr countermodel
    rw [empty] at listed
    exact nomatch listed
  · intro none
    cases outputs : inner.outputs with
    | nil => rfl
    | cons J rest =>
      have listed : J ∈ inner.outputs := by rw [outputs]; exact List.mem_cons_self ..
      exact absurd ((ReductRegions.countermodels_exact M T inner J).mp listed) (none J)

/-- **L6, membership depends on the theory and the candidate alone.** Two completed
walks of a candidate's countermodels, whatever subjects of the countermodels
narrowed them, reach the same `any` verdict. -/
theorem countermodel_verdict_independent (T : Theory α) (M : Atoms α)
    (first second : CoverageTree (Countermodels T M) (ReductRegions.query M)) :
    AxisFolds.any.fold (fun _ => true) first.outputs =
      AxisFolds.any.fold (fun _ => true) second.outputs := by
  have sameVerdict :
      AxisFolds.any.fold (fun _ => true) first.outputs = false ↔
        AxisFolds.any.fold (fun _ => true) second.outputs = false :=
    (any_true_eq_false_iff _).trans ((countermodel_outputs_empty_iff T M first).trans
      ((countermodel_outputs_empty_iff T M second).symm.trans (any_true_eq_false_iff _).symm))
  cases hf : AxisFolds.any.fold (fun _ => true) first.outputs <;>
    cases hs : AxisFolds.any.fold (fun _ => true) second.outputs
  · rfl
  · exact Bool.noConfusion (hs.symm.trans (sameVerdict.mp hf))
  · exact Bool.noConfusion (hf.symm.trans (sameVerdict.mpr hs))
  · rfl

/-! ## Rows of the countermodels from refuted patterns -/

/-- The region deciding every atom of `S` as `σ` does, inside `r`: an atom of `S`
that `σ` holds joins the lower bound, one it rejects leaves the upper bound. -/
def _root_.Zetesis.Region.decideAll (r : Region α) (S : List α) (σ : Atoms α) : Region α :=
  ⟨fun a => r.lower a ∨ (a ∈ S ∧ σ a), fun a => r.upper a ∧ ¬ (a ∈ S ∧ ¬ σ a)⟩

/-- A member of a region lies in the region deciding `S` as any pattern that agrees
with it on `S`, in particular as its own restriction to `S`. -/
theorem contains_decideAll {r : Region α} {S : List α} {z σ : Atoms α}
    (inside : r.Contains z) (agree : ∀ a ∈ S, (σ a ↔ z a)) :
    (r.decideAll S σ).Contains z := by
  refine ⟨?_, ?_⟩
  · intro a lower
    cases lower with
    | inl held => exact inside.1 a held
    | inr decided => exact (agree a decided.1).mp decided.2
  · intro a held
    exact ⟨inside.2 a held, fun rejected => rejected.2 ((agree a rejected.1).mpr held)⟩

/-- **Rows from refuted patterns.** For a subject `s` sound for `C` and a region
`r`: if every pattern over the support `S` outside `Allowed`, taken as an
interpretation supported on `S`, is refuted by narrowing the region that decides
it, then every member of `C` in `r` is allowed. `Allowed` depends on `S` alone.

Proof. A member lies in the region deciding its own restriction to `S`
(`contains_decideAll`); soundness keeps it in the narrowing, so its restriction is
not among the refuted patterns, and `Allowed` transfers to the member. The Rust
discharges the premise by the refutations of each probe's narrowing: in the first
probing round the frozen subject's, whose soundness is the walk's own obligation
(`ReductRegions.masked_known_sound`), and in a later round the frozen subject's
composed with the row subjects of the round before, sound for the class as their
joint closure is (`joint_sound`); with
`AlgebraicRows.measure_congr` an equation holding on every allowed pattern is an
equation of the class in `r`, and `AlgebraicRows.bounds_of_allowed` gives its
count bounds. -/
theorem entailed_by_refutations (C : Class α) (s : Subject C) (r : Region α) (S : List α)
    (Allowed : Atoms α → Prop)
    (depends : ∀ σ τ : Atoms α, (∀ a ∈ S, (σ a ↔ τ a)) → Allowed σ → Allowed τ)
    (refuted : ∀ σ : Atoms α, (∀ a, σ a → a ∈ S) → ¬ Allowed σ →
      ∀ z, (s.narrow (r.decideAll S σ)).Contains z → ¬ C z) :
    ∀ z, r.Contains z → C z → Allowed z := by
  intro z inside member
  refine Classical.byContradiction fun notAllowed => ?_
  have agree : ∀ a ∈ S, ((fun b => b ∈ S ∧ z b) a ↔ z a) := by
    intro a listed
    exact ⟨fun restricted => restricted.2, fun held => ⟨listed, held⟩⟩
  have supported : ∀ a, (fun b => b ∈ S ∧ z b) a → a ∈ S := fun _ restricted => restricted.1
  have restrictionNotAllowed : ¬ Allowed (fun b => b ∈ S ∧ z b) := by
    intro allowed
    exact notAllowed (depends _ z agree allowed)
  have narrowed : (s.narrow (r.decideAll S (fun b => b ∈ S ∧ z b))).Contains z :=
    s.sound (contains_decideAll inside agree) member
  exact refuted _ supported restrictionNotAllowed z narrowed member

/-- **Saturation units.** For a candidate `M` and a subject `s` sound for its
countermodels: if narrowing the query root with `a` held leaves only interpretations
that contain `M` or are no countermodel, then `¬ a` is a row of the countermodels
with no conditions. A countermodel holding `a` lies in that narrowing, so it would
contain `M`, contradicting `ProperSub`, or it is no countermodel. The Rust probes
from the closed root, `s.narrow (query M)`, with `a` held: a countermodel holding
`a` lies in the query root and, by `s`'s soundness, in the closed root and in that
probe's region, so a probe that reaches `M` or is refuted discharges the premise. -/
theorem saturation_unit (T : Theory α) (M : Atoms α) (s : Subject (Countermodels T M)) (a : α)
    (reaches : ∀ z,
      (s.narrow ((ReductRegions.query M).decideAll [a] (fun _ => True))).Contains z →
        Sub M z ∨ ¬ Countermodels T M z) :
    RowOf (Countermodels T M) [] [(a, false)] := by
  intro z member _ o output
  rw [List.mem_singleton.mp output]
  intro held
  have inRoot : (ReductRegions.query M).Contains z :=
    (ReductRegions.query_contains M z).mpr member.1.1
  have agree : ∀ b ∈ [a], ((fun _ => True) b ↔ z b) := by
    intro b listed
    rw [List.mem_singleton.mp listed]
    exact ⟨fun _ => held, fun _ => trivial⟩
  have narrowed :
      (s.narrow ((ReductRegions.query M).decideAll [a] (fun _ => True))).Contains z :=
    s.sound (contains_decideAll inRoot agree) member
  cases reaches z narrowed with
  | inl contains => exact member.1.2 contains
  | inr none => exact none member

/-- **The membership check's construction.** The countermodel walk takes only the
theory and the candidate: its root is `ReductRegions.query M` and its subjects are
built from `T` and `M` alone: the frozen subject; the parity rows and units a
preparation installs, each a subject of the class through `row` with a `RowOf`
proof from `entailed_by_refutations` or `saturation_unit`, over the frozen subject
in the first probing round and, in a later round, over the frozen subject composed
with the rows of the round before (`joint_sound`); and its count bounds, subjects
through `doubleCount` and the guarded bounds, with `LowerOf` and `UpperOf` from
`AlgebraicRows.bounds_of_allowed`, `upper_of_clique` and `lower_of_clause`.
It takes no supplied subject and no supplied root knowledge. That the Rust reduct
query keeps to this construction, probing from the closed query root and never
sharing a preparation across candidates, is an implementation obligation, a
discipline made visible here, not a type fact. -/
noncomputable def countermodelSubjects (T : Theory α) (M : Atoms α)
    (rows : List (Subject (Countermodels T M))) : List (Subject (Countermodels T M)) :=
  frozen T M :: rows

end Zetesis.Algebra
