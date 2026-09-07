import Zetesis.QueryCompaction

/-!
# Exact projection indexing and failed-literal candidate pruning

An executable Boolean trie replaces a linear conjunction of exact semantic
blocking clauses. Whole finite Boolean keys are retained; auxiliary assignment
bits are outside the supplied projection. A classical failed-branch certificate
permits forcing its opposite, without changing the satisfying candidate region.

These are denotational transform contracts, not a refinement of Rust's flat
trie arena, unit propagation, watches, trial undo, allocation or work ceilings.
The failed-literal law requires an actual branch refutation; interrupted probes
cannot supply one. Original Ferraris roots and reducts remain unchanged.
-/

namespace Zetesis.IndexedCandidates

inductive Trie where
  | empty
  | fork : Bool → Trie → Trie → Trie
  deriving DecidableEq

/-- Exact complete-key lookup; terminal bits distinguish differing lengths. -/
def lookup : List Bool → Trie → Bool
  | _, .empty => false
  | [], .fork terminal _ _ => terminal
  | false :: rest, .fork _ left _ => lookup rest left
  | true :: rest, .fork _ _ right => lookup rest right

/-- Insert one complete projection, preserving all existing keys. -/
def insert : List Bool → Trie → Trie
  | [], .empty => .fork true .empty .empty
  | [], .fork _ left right => .fork true left right
  | false :: rest, .empty => .fork false (insert rest .empty) .empty
  | true :: rest, .empty => .fork false .empty (insert rest .empty)
  | false :: rest, .fork terminal left right => .fork terminal (insert rest left) right
  | true :: rest, .fork terminal left right => .fork terminal left (insert rest right)

theorem lookup_empty (key : List Bool) : lookup key .empty = false := by
  cases key <;> rfl

theorem lookup_insert (stored tested : List Bool) (trie : Trie) :
    lookup tested (insert stored trie) = (decide (tested = stored) || lookup tested trie) := by
  induction stored generalizing tested trie with
  | nil =>
    cases tested with
    | nil => cases trie <;> simp [insert, lookup]
    | cons head tail => cases head <;> cases trie <;> simp [insert, lookup]
  | cons head tail ih =>
    cases tested with
    | nil => cases head <;> cases trie <;> simp [insert, lookup]
    | cons next rest =>
      cases head <;> cases next <;> cases trie <;>
        simp [insert, lookup, ih, lookup_empty]

/-- Build an index from the existing exact blocked projections. -/
def index (keys : List (List Bool)) : Trie := keys.foldr insert .empty

theorem lookup_index (keys : List (List Bool)) (tested : List Bool) :
    lookup tested (index keys) = decide (tested ∈ keys) := by
  induction keys with
  | nil => simp [index, lookup_empty]
  | cons key keys ih =>
    change lookup tested (insert key (index keys)) = _
    simp [lookup_insert, ih]

/-- A disjunction of complemented equality bits, including the empty clause.
    Mismatched lengths are unequal keys; the runtime admits a fixed width. -/
def block : List Bool → List Bool → Bool
  | [], [] => false
  | a :: left, b :: right => (a != b) || block left right
  | _, _ => true

theorem block_exact (stored tested : List Bool) :
    block stored tested = !(decide (tested = stored)) := by
  induction stored generalizing tested with
  | nil => cases tested <;> simp [block]
  | cons head tail ih =>
    cases tested with
    | nil => simp [block]
    | cons next rest => cases head <;> cases next <;> simp [block, ih]

/-- Linear reference checking all exact blocking clauses. -/
def allowed (keys : List (List Bool)) (tested : List Bool) : Bool :=
  keys.all (fun stored => block stored tested)

theorem index_equals_all_blocks (keys : List (List Bool)) (tested : List Bool) :
    (!(lookup tested (index keys))) = allowed keys tested := by
  rw [lookup_index]
  induction keys with
  | nil => simp [allowed]
  | cons key keys ih =>
    change (!(decide (tested ∈ key :: keys))) = (block key tested && allowed keys tested)
    rw [block_exact, ← ih]
    simp

/-- Auxiliary extensions with the same semantic key receive the same decision. -/
theorem identical_projection_same_filter {β : Type} (project : β → List Bool)
    (keys : List (List Bool)) (first second : β) (same : project first = project second) :
    (!(lookup (project first) (index keys))) = (!(lookup (project second) (index keys))) := by
  rw [same]

/-- Every recorded key is rejected, irrespective of duplicate insertions. -/
theorem recorded_projection_rejected (keys : List (List Bool)) (tested : List Bool)
    (recorded : tested ∈ keys) : lookup tested (index keys) = true := by
  simp [lookup_index, recorded]

/-- Indexing rejects no previously unrecorded semantic key. -/
theorem unseen_projection_allowed (keys : List (List Bool)) (tested : List Bool)
    (unseen : tested ∉ keys) : lookup tested (index keys) = false := by
  simp [lookup_index, unseen]

universe u
variable {α : Type u}

/-- Refutation of a trial branch makes its opposite true in every candidate.
    The query predicate can include all already established root assignments. -/
theorem failed_literal_forced (valid : (α → Bool) → Prop) (atom : α) (trial : Bool)
    (refuted : ∀ valuation, valuation atom = trial → ¬ valid valuation)
    (valuation : α → Bool) (model : valid valuation) : valuation atom = !trial := by
  have different : valuation atom ≠ trial := fun same => refuted valuation same model
  cases trial <;> cases truth : valuation atom <;> simp_all

/-- Conjoining the certified opposite literal preserves the entire query region. -/
theorem failed_literal_query_exact (valid : (α → Bool) → Prop) (atom : α) (trial : Bool)
    (refuted : ∀ valuation, valuation atom = trial → ¬ valid valuation)
    (valuation : α → Bool) :
    (valid valuation ∧ valuation atom = !trial) ↔ valid valuation := by
  exact ⟨And.left, fun model => ⟨model, failed_literal_forced valid atom trial refuted valuation model⟩⟩

/-- A probe may refine even a frozen countermodel query, if that exact query's
    branch is refuted. This law neither recomputes M's mask nor changes roots. -/
theorem frozen_countermodel_probe_exact [DecidableEq α] (candidate : α → Bool)
    (theory : Ferraris.Theory α) (atom : α) (trial : Bool)
    (refuted : ∀ tested, tested atom = trial →
      ¬ (Ferraris.ProperSub (QueryCompaction.asAtoms tested) (QueryCompaction.asAtoms candidate) ∧
        QueryCompaction.QueryModels candidate tested theory)) :
    (∃ tested, (Ferraris.ProperSub (QueryCompaction.asAtoms tested) (QueryCompaction.asAtoms candidate) ∧
        QueryCompaction.QueryModels candidate tested theory) ∧ tested atom = !trial) ↔
    ∃ J, Ferraris.ProperSub J (QueryCompaction.asAtoms candidate) ∧
      Ferraris.Models J (Ferraris.ReductTheory (QueryCompaction.asAtoms candidate) theory) := by
  rw [← QueryCompaction.compacted_countermodels_iff]
  constructor
  · rintro ⟨tested, model, _⟩
    exact ⟨tested, model⟩
  · rintro ⟨tested, model⟩
    exact ⟨tested, model, failed_literal_forced _ atom trial refuted tested model⟩

end Zetesis.IndexedCandidates
