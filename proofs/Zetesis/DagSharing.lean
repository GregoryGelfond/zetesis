import Zetesis.FerrarisMask

/-!
# Exact structural interning of formula DAGs

This executable abstract transform scans an admitted topological node table,
remaps earlier child indices, and reuses an exactly equal existing node or
appends one new node. It never performs a Boolean or classical rewrite.

The representation theorem establishes exact unfolded formula equality, then
preserves arbitrary original and frozen-reduct interpretations and stability.
The Rust hash table, memory limits, source cache keys, binding independence and
fixed support universe are separate obligations, not hypotheses silently
provided by this theorem. The current development uses unbounded mathematical
lists and linear structural lookup as an executable specification.
-/

namespace Zetesis.DagSharing

universe u
variable {α : Type u}
open Ferraris

/-- Child references name earlier nodes in an admitted table. -/
inductive Node (α : Type u) where
  | atom : α → Node α
  | bot : Node α
  | conj : Nat → Nat → Node α
  | disj : Nat → Nat → Node α
  | imp : Nat → Nat → Node α
deriving DecidableEq

/-- Admission of a node against its already available prefix. -/
def ValidNode (length : Nat) : Node α → Prop
  | .atom _ | .bot => True
  | .conj a b | .disj a b | .imp a b => a < length ∧ b < length

/-- A finite acyclic table admitted in construction order. -/
inductive WellFormed : List (Node α) → Prop
  | nil : WellFormed []
  | snoc {table : List (Node α)} {node : Node α} :
      WellFormed table → ValidNode table.length node → WellFormed (table ++ [node])

/-- Decode one layer using previously unfolded child formulas. Defaults make
    this function total; the preservation theorem requires admitted references. -/
def decode (children : List (Formula α)) : Node α → Formula α
  | .atom atom => .atom atom
  | .bot => .bot
  | .conj a b => .conj (children.getD a .bot) (children.getD b .bot)
  | .disj a b => .disj (children.getD a .bot) (children.getD b .bot)
  | .imp a b => .imp (children.getD a .bot) (children.getD b .bot)

/-- Unfold the DAG in one topological fold. Sharing is a representation choice. -/
def meanings (table : List (Node α)) : List (Formula α) :=
  table.foldl (fun children node => children ++ [decode children node]) []

theorem getD_append_left {β : Type u} (left right : List β)
    (index : Nat) (fallback : β) (inside : index < left.length) :
    (left ++ right).getD index fallback = left.getD index fallback := by
  simp only [List.getD_eq_getElem?_getD, List.getElem?_append_left inside]

theorem getD_snoc_end {β : Type u} (left : List β) (last fallback : β) :
    (left ++ [last]).getD left.length fallback = last := by
  simp [List.getD_eq_getElem?_getD]

theorem fold_length (table : List (Node α)) (children : List (Formula α)) :
    (table.foldl (fun cs node => cs ++ [decode cs node]) children).length =
      children.length + table.length := by
  induction table generalizing children with
  | nil => simp
  | cons node rest ih => simp only [List.foldl_cons, ih, List.length_append,
      List.length_cons, List.length_nil]; omega

theorem meanings_length (table : List (Node α)) :
    (meanings table).length = table.length := by
  simpa [meanings] using fold_length table []

theorem meanings_snoc (table : List (Node α)) (node : Node α) :
    meanings (table ++ [node]) = meanings table ++ [decode (meanings table) node] := by
  simp [meanings, List.foldl_append]

theorem decode_append (children suffix : List (Formula α)) (node : Node α)
    (valid : ValidNode children.length node) :
    decode (children ++ suffix) node = decode children node := by
  cases node <;> simp only [ValidNode] at valid
  · rfl
  · rfl
  all_goals
    simp only [decode]
    rw [getD_append_left _ _ _ _ valid.1, getD_append_left _ _ _ _ valid.2]

theorem well_formed_reference {table : List (Node α)} (wf : WellFormed table)
    (index : Nat) (inside : index < table.length) :
    ValidNode index (table.getD index .bot) := by
  induction wf with
  | nil => simp at inside
  | @snoc table node wf valid ih =>
    by_cases old : index < table.length
    · rw [getD_append_left _ _ _ _ old]
      exact ih old
    · have atEnd : index = table.length := by
        simp only [List.length_append, List.length_singleton] at inside
        omega
      subst index
      simpa [getD_snoc_end] using valid

theorem valid_mono {node : Node α} {small large : Nat}
    (valid : ValidNode small node) (le : small ≤ large) : ValidNode large node := by
  cases node <;> simp only [ValidNode] at *
  all_goals omega

/-- Every admitted stored node has the meaning obtained by decoding its exact
    syntax against the completed table; later entries cannot change a child. -/
theorem stored_meaning {table : List (Node α)} (wf : WellFormed table)
    (index : Nat) (inside : index < table.length) :
    decode (meanings table) (table.getD index .bot) =
      (meanings table).getD index .bot := by
  induction wf with
  | nil => simp at inside
  | @snoc table node wf valid ih =>
    by_cases old : index < table.length
    · rw [getD_append_left _ _ _ _ old, meanings_snoc]
      rw [decode_append]
      · rw [getD_append_left _ _ _ _ (by simpa [meanings_length] using old)]
        exact ih old
      · rw [meanings_length]
        exact valid_mono (well_formed_reference wf index old) (Nat.le_of_lt old)
    · have atEnd : index = table.length := by
        simp only [List.length_append, List.length_singleton] at inside
        omega
      subst index
      rw [getD_snoc_end, meanings_snoc, decode_append]
      · rw [← meanings_length table, getD_snoc_end]
      · simpa [meanings_length] using valid

/-- Exact syntactic lookup, retaining the first equal node. -/
def findNode [DecidableEq α] (node : Node α) : List (Node α) → Option Nat
  | [] => none
  | head :: tail => if node = head then some 0 else (findNode node tail).map Nat.succ

theorem findNode_sound [DecidableEq α] (node : Node α)
    (table : List (Node α)) (index : Nat) (found : findNode node table = some index) :
    index < table.length ∧ table.getD index .bot = node := by
  induction table generalizing index with
  | nil => simp [findNode] at found
  | cons head tail ih =>
    by_cases same : node = head
    · simp [findNode, same] at found
      subst index
      simp [same]
    · simp only [findNode, same, ↓reduceIte] at found
      cases child : findNode node tail with
      | none => simp [child] at found
      | some previous =>
        simp only [child, Option.map_some, Option.some.injEq] at found
        subst index
        obtain ⟨bound, lookup⟩ := ih previous child
        exact ⟨by simpa using Nat.succ_lt_succ bound, by simpa using lookup⟩

/-- Intern an already reindexed node. Reuse is exact structural equality. -/
def intern [DecidableEq α] (table : List (Node α)) (node : Node α) :
    List (Node α) × Nat :=
  match findNode node table with
  | some index => (table, index)
  | none => (table ++ [node], table.length)

/-- Interning preserves every existing entry and its unfolded meaning, creates
    a valid result reference, and gives the new node exactly its layer meaning. -/
theorem intern_correct [DecidableEq α] (table : List (Node α)) (node : Node α)
    (wf : WellFormed table) (valid : ValidNode table.length node) :
    let result := intern table node
    WellFormed result.1 ∧ result.2 < result.1.length ∧
      (meanings result.1).getD result.2 .bot = decode (meanings table) node ∧
      ∀ index, index < table.length →
        index < result.1.length ∧
        (meanings result.1).getD index .bot = (meanings table).getD index .bot := by
  cases found : findNode node table with
  | some index =>
    obtain ⟨inside, lookup⟩ := findNode_sound node table index found
    simp only [intern, found]
    refine ⟨wf, inside, ?_, ?_⟩
    · rw [← lookup]
      exact (stored_meaning wf index inside).symm
    · intro i h
      exact ⟨h, trivial⟩
  | none =>
    simp only [intern, found]
    refine ⟨WellFormed.snoc wf valid, ?_, ?_, ?_⟩
    · simp
    · rw [meanings_snoc]
      rw [← meanings_length table, getD_snoc_end]
    · intro index inside
      refine ⟨by simp; omega, ?_⟩
      rw [meanings_snoc, getD_append_left _ _ _ _ (by simpa [meanings_length] using inside)]


/-- Substitute the new references for all already processed source children. -/
def reindex (aliases : List Nat) : Node α → Node α
  | .atom atom => .atom atom
  | .bot => .bot
  | .conj a b => .conj (aliases.getD a 0) (aliases.getD b 0)
  | .disj a b => .disj (aliases.getD a 0) (aliases.getD b 0)
  | .imp a b => .imp (aliases.getD a 0) (aliases.getD b 0)

/-- The interned table and one result index for every processed source node. -/
structure State (α : Type u) where
  table : List (Node α)
  aliases : List Nat

/-- One executable scan step: reindex children, then exact-node intern. -/
def step [DecidableEq α] (state : State α) (node : Node α) : State α :=
  let result := intern state.table (reindex state.aliases node)
  ⟨result.1, state.aliases ++ [result.2]⟩

/-- State invariant has independent syntactic bounds and exact representation. -/
structure Represents (source : List (Node α)) (state : State α) : Prop where
  wellFormed : WellFormed state.table
  mapLength : state.aliases.length = source.length
  inBounds : ∀ index, index < source.length →
    state.aliases.getD index 0 < state.table.length
  exactMeaning : ∀ index, index < source.length →
    (meanings state.table).getD (state.aliases.getD index 0) .bot =
      (meanings source).getD index .bot

theorem reindex_valid {source : List (Node α)} {state : State α}
    (correct : Represents source state) (node : Node α)
    (valid : ValidNode source.length node) :
    ValidNode state.table.length (reindex state.aliases node) := by
  cases node <;> simp only [ValidNode] at valid
  · trivial
  · trivial
  all_goals exact ⟨correct.inBounds _ valid.1, correct.inBounds _ valid.2⟩

theorem reindex_meaning {source : List (Node α)} {state : State α}
    (correct : Represents source state) (node : Node α)
    (valid : ValidNode source.length node) :
    decode (meanings state.table) (reindex state.aliases node) = decode (meanings source) node := by
  cases node <;> simp only [ValidNode] at valid
  · rfl
  · rfl
  all_goals
    simp only [reindex, decode]
    rw [correct.exactMeaning _ valid.1, correct.exactMeaning _ valid.2]

/-- The invariant is established by the transform itself at every scan step. -/
theorem step_correct [DecidableEq α] {source : List (Node α)} {state : State α}
    (correct : Represents source state) (node : Node α)
    (valid : ValidNode source.length node) :
    Represents (source ++ [node]) (step state node) := by
  let result := intern state.table (reindex state.aliases node)
  have interned := intern_correct state.table (reindex state.aliases node)
    correct.wellFormed (reindex_valid correct node valid)
  change WellFormed result.1 ∧ result.2 < result.1.length ∧
    (meanings result.1).getD result.2 .bot =
      decode (meanings state.table) (reindex state.aliases node) ∧
    (∀ index, index < state.table.length → index < result.1.length ∧
      (meanings result.1).getD index .bot = (meanings state.table).getD index .bot)
    at interned
  change Represents (source ++ [node]) ⟨result.1, state.aliases ++ [result.2]⟩
  have newAlias : (state.aliases ++ [result.2]).getD source.length 0 = result.2 := by
    rw [← correct.mapLength]
    exact getD_snoc_end _ _ _
  constructor
  · exact interned.1
  · simp [correct.mapLength]
  · intro index inside
    by_cases old : index < source.length
    · rw [getD_append_left _ _ _ _ (by rw [correct.mapLength]; exact old)]
      exact (interned.2.2.2 _ (correct.inBounds index old)).1
    · have atEnd : index = source.length := by
        simp only [List.length_append, List.length_singleton] at inside
        omega
      subst index
      rw [newAlias]
      exact interned.2.1
  · intro index inside
    by_cases old : index < source.length
    · rw [getD_append_left _ _ _ _ (by rw [correct.mapLength]; exact old)]
      rw [(interned.2.2.2 _ (correct.inBounds index old)).2, correct.exactMeaning index old]
      rw [meanings_snoc, getD_append_left _ _ _ _ (by simpa [meanings_length] using old)]
    · have atEnd : index = source.length := by
        simp only [List.length_append, List.length_singleton] at inside
        omega
      subst index
      rw [newAlias, interned.2.2.1, reindex_meaning correct node valid, meanings_snoc]
      rw [← meanings_length source, getD_snoc_end]

/-- Scan an independent admitted source DAG into an existing admitted table.
    Existing nodes may be reused; source IDs always refer to the source prefix. -/
def shareInto [DecidableEq α] (initial source : List (Node α)) : State α :=
  source.foldl step ⟨initial, []⟩

/-- Full executable sharing constructs the representation invariant from only
    syntactic admission; it does not assume semantic equivalence as an input. -/
theorem share_into_correct [DecidableEq α] (initial source : List (Node α))
    (initialValid : WellFormed initial) (sourceValid : WellFormed source) :
    Represents source (shareInto initial source) := by
  induction sourceValid with
  | nil =>
    constructor
    · exact initialValid
    · rfl
    · intro index h
      simp at h
    · intro index h
      simp at h
  | @snoc source node wf valid ih =>
    simpa [shareInto, List.foldl_append] using step_correct ih node valid

/-- Root assertions use original semantic atoms only. Unasserted table entries
    do not add formulas to the theory. -/
def assertions (table : List (Node α)) (roots : List Nat) : Theory α :=
  roots.map (fun index => (meanings table).getD index .bot)

/-- Reindex every source root through the complete alias map. -/
def mappedRoots (state : State α) (roots : List Nat) : List Nat :=
  roots.map (fun index => state.aliases.getD index 0)

theorem represented_theory_equal {source : List (Node α)} {state : State α}
    (correct : Represents source state) (roots : List Nat)
    (validRoots : ∀ index, index ∈ roots → index < source.length) :
    assertions state.table (mappedRoots state roots) = assertions source roots := by
  simp only [assertions, mappedRoots, List.map_map]
  apply List.map_congr_left
  intro index member
  exact correct.exactMeaning index (validRoots index member)

/-- Exact structural interning preserves the whole unfolded asserted theory. -/
theorem sharing_theory_equal [DecidableEq α] (initial source : List (Node α))
    (initialValid : WellFormed initial) (sourceValid : WellFormed source)
    (roots : List Nat) (validRoots : ∀ index, index ∈ roots → index < source.length) :
    let shared := shareInto initial source
    assertions shared.table (mappedRoots shared roots) = assertions source roots := by
  exact represented_theory_equal (share_into_correct initial source initialValid sourceValid)
    roots validRoots

/-- No classical rewrite or fresh semantic atom is needed to preserve truth. -/
theorem sharing_models_iff [DecidableEq α] (initial source : List (Node α))
    (initialValid : WellFormed initial) (sourceValid : WellFormed source)
    (roots : List Nat) (validRoots : ∀ index, index ∈ roots → index < source.length)
    (M : Atoms α) :
    let shared := shareInto initial source
    Models M (assertions shared.table (mappedRoots shared roots)) ↔
      Models M (assertions source roots) := by
  dsimp only
  rw [sharing_theory_equal initial source initialValid sourceValid roots validRoots]

/-- Original and tested interpretations are arbitrary; J need not be a subset
    of M for representation preservation of the frozen reduct. -/
theorem sharing_reduct_models_iff [DecidableEq α] (initial source : List (Node α))
    (initialValid : WellFormed initial) (sourceValid : WellFormed source)
    (roots : List Nat) (validRoots : ∀ index, index ∈ roots → index < source.length)
    (M J : Atoms α) :
    let shared := shareInto initial source
    Models J (ReductTheory M (assertions shared.table (mappedRoots shared roots))) ↔
      Models J (ReductTheory M (assertions source roots)) := by
  dsimp only
  rw [sharing_theory_equal initial source initialValid sourceValid roots validRoots]

/-- Sharing leaves stable acceptance unchanged, including the exact set of
    proper-subset countermodels over the same original atom interpretation. -/
theorem sharing_stable_iff [DecidableEq α] (initial source : List (Node α))
    (initialValid : WellFormed initial) (sourceValid : WellFormed source)
    (roots : List Nat) (validRoots : ∀ index, index ∈ roots → index < source.length)
    (M : Atoms α) :
    let shared := shareInto initial source
    Stable M (assertions shared.table (mappedRoots shared roots)) ↔
      Stable M (assertions source roots) := by
  dsimp only
  rw [sharing_theory_equal initial source initialValid sourceValid roots validRoots]

/-- Exact lookup finds every structurally present node; interning does perform
    sharing rather than merely allowing an implementation that always appends. -/
theorem findNode_complete [DecidableEq α] (node : Node α) (table : List (Node α))
    (member : node ∈ table) : ∃ index, findNode node table = some index := by
  induction table with
  | nil => simp at member
  | cons head tail ih =>
    by_cases same : node = head
    · exact ⟨0, by simp [findNode, same]⟩
    · have inTail : node ∈ tail := by simpa [same] using member
      obtain ⟨index, found⟩ := ih inTail
      exact ⟨index + 1, by simp [findNode, same, found]⟩

/-- A repeated exact node never enlarges the table and returns its existing
    reference. Semantic equivalence alone is not the lookup criterion. -/
theorem intern_existing_reuses [DecidableEq α] (node : Node α)
    (table : List (Node α)) (member : node ∈ table) :
    (intern table node).1 = table ∧ (intern table node).2 < table.length := by
  obtain ⟨index, found⟩ := findNode_complete node table member
  simpa [intern, found] using (findNode_sound node table index found).1

/-- A repeated choice-shaped graph shares exact nodes but retains the original
    p OR not p syntax, rather than replacing it with a classical constant. -/
def repeatedChoice : List (Node Bool) :=
  [.atom false, .bot, .imp 0 1, .disj 0 2, .atom false, .imp 4 1, .disj 4 5]

theorem exact_choice_graph_sharing :
    (shareInto [] repeatedChoice).table = [.atom false, .bot, .imp 0 1, .disj 0 2] ∧
    (shareInto [] repeatedChoice).aliases = [0, 1, 2, 3, 0, 2, 3] := by
  decide

end Zetesis.DagSharing
