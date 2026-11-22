import Zetesis.Core

/-!
# Adjacent coalescing of ordered rows

An adjacent pass preserves set membership for any input. If its adjacent pairs
are nondecreasing under an explicitly transitive and antisymmetric relation,
its result is strictly increasing and therefore unique. The relation is supplied;
no ASP term order or raw-identity interpretation is assumed.

The laws concern lists. Rust's column decoding, comparison agreement, in-place
ordinal compaction, inversion fallback, accounting and failure order remain
separate correspondence obligations.
-/

namespace Zetesis.OrderedNormalization

universe u
variable {α : Type u}

/-- Every adjacent pair follows the supplied non-strict relation. -/
def Adjacent (le : α → α → Prop) : List α → Prop
  | [] => True
  | [_] => True
  | first :: second :: rest => le first second ∧ Adjacent le (second :: rest)

/-- The supplied non-strict relation composes. -/
def Transitive (le : α → α → Prop) : Prop :=
  ∀ a b c, le a b → le b c → le a c

/-- Two mutually ordered rows have the same whole value. -/
def Antisymmetric (le : α → α → Prop) : Prop :=
  ∀ a b, le a b → le b a → a = b

/-- Keep the next distinct row, updating the preceding row at each step. -/
def following [DecidableEq α] (previous : α) : List α → List α
  | [] => []
  | first :: rest =>
      if first = previous then following first rest
      else first :: following first rest

/-- Retain the first row of each consecutive run of equal rows. -/
def coalesce [DecidableEq α] : List α → List α
  | [] => []
  | first :: rest => first :: following first rest

/-- Coalescing a suffix preserves membership when its preceding row is included. -/
theorem following_members [DecidableEq α] (rows : List α) (value : α) :
    ∀ previous, value ∈ previous :: following previous rows ↔
      value ∈ previous :: rows := by
  induction rows with
  | nil => intro previous; simp [following]
  | cons first rest ih =>
    intro previous
    by_cases same : first = previous
    · subst first
      simpa [following] using ih previous
    · simp only [following, if_neg same, List.mem_cons]
      simpa only [List.mem_cons] using
        (or_congr (Iff.rfl : value = previous ↔ value = previous) (ih first))

/-- Dropping consecutive equal rows preserves the entire input set. -/
theorem coalesce_members [DecidableEq α] (rows : List α) (value : α) :
    value ∈ coalesce rows ↔ value ∈ rows := by
  cases rows with
  | nil => simp [coalesce]
  | cons first rest => exact following_members rest value first

/-- Transitivity turns adjacent nondecreasing comparisons into global order. -/
theorem adjacent_pairwise (le : α → α → Prop) (transitive : Transitive le)
    (rows : List α) (ordered : Adjacent le rows) : rows.Pairwise le := by
  induction rows with
  | nil => exact List.Pairwise.nil
  | cons first rest ih =>
    cases rest with
    | nil => simp
    | cons second tail =>
      have next : le first second ∧ Adjacent le (second :: tail) := ordered
      have tailOrdered : (second :: tail).Pairwise le := ih next.2
      have headBound : ∀ value ∈ second :: tail, le first value := by
        intro value present
        rcases List.mem_cons.mp present with same | later
        · subst value; exact next.1
        · exact transitive first second value next.1
            ((List.pairwise_cons.mp tailOrdered).1 value later)
      exact List.pairwise_cons.mpr ⟨headBound, tailOrdered⟩

/-- Coalescing a globally ordered suffix removes equal values from its order. -/
theorem following_strict [DecidableEq α] (le : α → α → Prop)
    (antisymmetric : Antisymmetric le) (rows : List α) :
    ∀ previous, (previous :: rows).Pairwise le →
      (previous :: following previous rows).Pairwise
        (fun a b => le a b ∧ a ≠ b) := by
  induction rows with
  | nil => intro previous ordered; simp [following]
  | cons first rest ih =>
    intro previous ordered
    have previousBound : ∀ value ∈ first :: rest, le previous value :=
      (List.pairwise_cons.mp ordered).1
    have tailOrdered : (first :: rest).Pairwise le :=
      (List.pairwise_cons.mp ordered).2
    by_cases same : first = previous
    · subst first
      simpa [following] using ih previous tailOrdered
    · have tailStrict : (first :: following first rest).Pairwise
          (fun a b => le a b ∧ a ≠ b) := ih first tailOrdered
      have headBound : ∀ value ∈ first :: following first rest,
          le previous value ∧ previous ≠ value := by
        intro value present
        have original : value ∈ first :: rest :=
          (following_members rest value first).mp present
        have different : previous ≠ value := by
          intro equal
          rcases List.mem_cons.mp original with atFirst | later
          · exact same (atFirst.symm.trans equal.symm)
          · have backward : le first previous := by
              rw [equal]
              exact (List.pairwise_cons.mp tailOrdered).1 value later
            exact same (antisymmetric first previous backward
              (previousBound first List.mem_cons_self))
        exact ⟨previousBound value original, different⟩
      simpa only [following, if_neg same] using
        List.pairwise_cons.mpr ⟨headBound, tailStrict⟩

/-- An admitted adjacent pass yields the same set in strict, unique order.
The named order premises concern the supplied whole-row relation. -/
theorem ordered_normalization_exact [DecidableEq α] (le : α → α → Prop)
    (transitive : Transitive le) (antisymmetric : Antisymmetric le)
    (rows : List α) (ordered : Adjacent le rows) :
    (∀ value, value ∈ coalesce rows ↔ value ∈ rows) ∧
      (coalesce rows).Pairwise (fun a b => le a b ∧ a ≠ b) ∧
      (coalesce rows).Nodup := by
  have strict : (coalesce rows).Pairwise (fun a b => le a b ∧ a ≠ b) := by
    cases rows with
    | nil => exact List.Pairwise.nil
    | cons first rest =>
      exact following_strict le antisymmetric rest first
        (adjacent_pairwise le transitive (first :: rest) ordered)
  have unique : (coalesce rows).Nodup := strict.imp (fun pair => pair.2)
  exact ⟨coalesce_members rows, strict, unique⟩

end Zetesis.OrderedNormalization
