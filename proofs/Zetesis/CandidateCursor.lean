import Zetesis.Ferraris

/-!
# Retained finite candidate traversal

This executable forest cursor expands one decision node or consumes one leaf
per fuel step. It retains the unexplored forest across yields. Appended blocks
compare only semantic projections, so distinct auxiliary leaves may represent
the same candidate. Blocking follows a completed external check, regardless of
its acceptance result; interruptions do not create completion evidence.

The tree is an abstract finite representation of the original classical query.
Its connection to a CNF, Rust watches/trails, chronological backtracking, source
grounding, memory ceilings and charged machine work is not proved here.
-/

namespace Zetesis.CandidateCursor

universe u v
variable {α : Type u} {β : Type v}

/-- A finite, ordered classical search tree. Leaves may share a projection. -/
inductive Tree (α : Type u) where
  | leaf : α → Tree α
  | fork : Tree α → Tree α → Tree α
deriving DecidableEq

def treeLeaves : Tree α → List α
  | .leaf value => [value]
  | .fork left right => treeLeaves left ++ treeLeaves right

def leaves (forest : List (Tree α)) : List α := forest.flatMap treeLeaves

def treeWork : Tree α → Nat
  | .leaf _ => 1
  | .fork left right => 1 + treeWork left + treeWork right

def work (forest : List (Tree α)) : Nat := (forest.map treeWork).sum

/-- Remaining leaves that disagree with every recorded semantic candidate. -/
def eligible [DecidableEq β] (project : α → β) (blocked : List β)
    (forest : List (Tree α)) : List α :=
  (leaves forest).filter (fun value => decide (project value ∉ blocked))

inductive Outcome (α : Type u) where
  | found : α → List (Tree α) → Outcome α
  | exhausted : Outcome α
  | interrupted : List (Tree α) → Outcome α
deriving DecidableEq

/-- Zero fuel always interrupts, even if the forest happens to be empty. -/
def next [DecidableEq β] (project : α → β) (blocked : List β) :
    Nat → List (Tree α) → Outcome α
  | 0, forest => .interrupted forest
  | _ + 1, [] => .exhausted
  | fuel + 1, .leaf value :: rest =>
      if project value ∈ blocked then next project blocked fuel rest
      else .found value rest
  | fuel + 1, .fork left right :: rest =>
      next project blocked fuel (left :: right :: rest)

/-- Exact remaining-list accounting, including multiplicity and order. -/
def Preserves [DecidableEq β] (project : α → β) (blocked : List β)
    (before : List (Tree α)) : Outcome α → Prop
  | .found value rest => eligible project blocked before =
      value :: eligible project blocked rest
  | .exhausted => eligible project blocked before = []
  | .interrupted rest => eligible project blocked before =
      eligible project blocked rest

theorem next_preserves [DecidableEq β] (project : α → β) (blocked : List β)
    (fuel : Nat) (forest : List (Tree α)) :
    Preserves project blocked forest (next project blocked fuel forest) := by
  induction fuel generalizing forest with
  | zero => rfl
  | succ fuel ih =>
    cases forest with
    | nil => rfl
    | cons tree rest =>
      cases tree with
      | leaf value =>
        by_cases h : project value ∈ blocked
        · simpa [next, h, Preserves, eligible, leaves, treeLeaves] using ih rest
        · simp [next, h, Preserves, eligible, leaves, treeLeaves]
      | fork left right =>
        simpa [next, Preserves, eligible, leaves, treeLeaves,
          List.append_assoc] using ih (left :: right :: rest)

/-- A mathematical node count is sufficient for one finite cursor call.
    This is distinct from the Rust primitive-work accounting or cancellation. -/
theorem enough_fuel_no_interruption [DecidableEq β] (project : α → β)
    (blocked : List β) (fuel : Nat) (forest rest : List (Tree α))
    (enough : work forest < fuel) :
    next project blocked fuel forest ≠ .interrupted rest := by
  induction fuel generalizing forest with
  | zero => omega
  | succ fuel ih =>
    cases forest with
    | nil => simp [next]
    | cons tree tail =>
      cases tree with
      | leaf value =>
        by_cases h : project value ∈ blocked
        · simp only [next, h, ↓reduceIte]
          apply ih tail
          simp only [work, List.map_cons, List.sum_cons, treeWork] at enough ⊢
          omega
        · simp [next, h]
      | fork left right =>
        simp only [next]
        apply ih (left :: right :: tail)
        simp only [work, List.map_cons, List.sum_cons, treeWork] at enough ⊢
        omega

theorem mem_eligible [DecidableEq β] (project : α → β) (blocked : List β)
    (forest : List (Tree α)) (value : α) :
    value ∈ eligible project blocked forest ↔
      value ∈ leaves forest ∧ project value ∉ blocked := by
  simp [eligible]

theorem found_sound [DecidableEq β] (project : α → β) (blocked : List β)
    (fuel : Nat) (forest rest : List (Tree α)) (value : α)
    (found : next project blocked fuel forest = .found value rest) :
    value ∈ leaves forest ∧ project value ∉ blocked := by
  have exact := next_preserves project blocked fuel forest
  rw [found] at exact
  apply (mem_eligible project blocked forest value).mp
  simp only [Preserves] at exact
  rw [exact]
  exact List.mem_cons_self

theorem exhausted_all_blocked [DecidableEq β] (project : α → β)
    (blocked : List β) (fuel : Nat) (forest : List (Tree α))
    (done : next project blocked fuel forest = .exhausted)
    (value : α) (present : value ∈ leaves forest) : project value ∈ blocked := by
  have exact := next_preserves project blocked fuel forest
  rw [done] at exact
  simp only [Preserves] at exact
  by_cases fresh : project value ∈ blocked
  · exact fresh
  · exfalso
    have member := (mem_eligible project blocked forest value).mpr ⟨present, fresh⟩
    rw [exact] at member
    exact List.not_mem_nil member

theorem interruption_retains_unblocked [DecidableEq β] (project : α → β)
    (blocked : List β) (fuel : Nat) (forest rest : List (Tree α))
    (stop : next project blocked fuel forest = .interrupted rest) :
    eligible project blocked forest = eligible project blocked rest := by
  have exact := next_preserves project blocked fuel forest
  simpa [stop, Preserves] using exact

/-- Recording a projection removes every auxiliary extension of that candidate. -/
theorem record_excludes_projection [DecidableEq β] (project : α → β)
    (blocked : List β) (forest : List (Tree α)) (recorded value : α)
    (same : project value = project recorded) :
    value ∉ eligible project (project recorded :: blocked) forest := by
  simp [mem_eligible, same]

/-- A completed trace contains actual cursor calls and exact projection blocks.
    It cannot finish at an interrupted call. External check evidence is separate;
    an application must stop before recording a candidate whose check is incomplete.
    No semantic acceptance predicate is used to prune this abstract traversal. -/
inductive Complete [DecidableEq β] (project : α → β) :
    List β → List (Tree α) → List α → Prop
  | done {blocked : List β} {forest : List (Tree α)} {fuel : Nat} :
      next project blocked fuel forest = .exhausted → Complete project blocked forest []
  | yield {blocked : List β} {forest rest : List (Tree α)}
      {value : α} {values : List α} {fuel : Nat} :
      next project blocked fuel forest = .found value rest →
      Complete project (project value :: blocked) rest values →
      Complete project blocked forest (value :: values)

theorem mem_projected_eligible [DecidableEq β] (project : α → β)
    (blocked : List β) (forest : List (Tree α)) (candidate : β) :
    candidate ∈ (eligible project blocked forest).map project ↔
      candidate ∈ (leaves forest).map project ∧ candidate ∉ blocked := by
  constructor
  · intro h
    obtain ⟨value, member, same⟩ := List.mem_map.mp h
    obtain ⟨leaf, fresh⟩ := (mem_eligible project blocked forest value).mp member
    exact ⟨List.mem_map.mpr ⟨value, leaf, same⟩, same ▸ fresh⟩
  · intro ⟨h, fresh⟩
    obtain ⟨value, member, same⟩ := List.mem_map.mp h
    exact List.mem_map.mpr ⟨value,
      (mem_eligible project blocked forest value).mpr ⟨member, same ▸ fresh⟩, same⟩

theorem found_projection_partition [DecidableEq β] (project : α → β)
    (blocked : List β) (fuel : Nat) (forest rest : List (Tree α)) (value : α)
    (found : next project blocked fuel forest = .found value rest) (candidate : β) :
    candidate ∈ (eligible project blocked forest).map project ↔
      candidate = project value ∨
        candidate ∈ (eligible project (project value :: blocked) rest).map project := by
  have exact := next_preserves project blocked fuel forest
  rw [found] at exact
  simp only [Preserves] at exact
  rw [exact, List.map_cons, List.mem_cons]
  simp only [mem_projected_eligible, List.mem_cons, not_or]
  constructor
  · intro h
    rcases h with same | ⟨member, fresh⟩
    · exact Or.inl same
    · by_cases same : candidate = project value
      · exact Or.inl same
      · exact Or.inr ⟨member, same, fresh⟩
  · intro h
    rcases h with same | ⟨member, _, fresh⟩
    · exact Or.inl same
    · exact Or.inr ⟨member, fresh⟩

theorem record_filters_remaining [DecidableEq β] (project : α → β)
    (blocked : List β) (forest : List (Tree α)) (candidate : β) :
    eligible project (candidate :: blocked) forest =
      (eligible project blocked forest).filter (fun value => decide (project value ≠ candidate)) := by
  simp only [eligible, List.filter_filter]
  apply List.filter_congr
  intro value _
  simp

/-- Every finite forest has a completed trace when each call receives enough
    mathematical fuel. Finite existence supplies no machine resource guarantee. -/
theorem finite_completion_exists [DecidableEq β] (project : α → β)
    (blocked : List β) (forest : List (Tree α)) :
    ∃ values, Complete project blocked forest values := by
  generalize size : (eligible project blocked forest).length = count
  induction count using Nat.strongRecOn generalizing blocked forest with
  | ind count ih =>
    cases result : next project blocked (work forest + 1) forest with
    | exhausted => exact ⟨[], Complete.done result⟩
    | interrupted rest =>
      exact False.elim (enough_fuel_no_interruption project blocked
        (work forest + 1) forest rest (by omega) result)
    | found value rest =>
      have exact := next_preserves project blocked (work forest + 1) forest
      rw [result] at exact
      simp only [Preserves] at exact
      have shorter : (eligible project (project value :: blocked) rest).length < count := by
        rw [record_filters_remaining]
        have le := List.length_filter_le (fun item => decide (project item ≠ project value))
          (eligible project blocked rest)
        have lengths := congrArg List.length exact
        simp only [List.length_cons] at lengths
        omega
      obtain ⟨values, completed⟩ := ih
        (eligible project (project value :: blocked) rest).length shorter
        (project value :: blocked) rest rfl
      exact ⟨value :: values, Complete.yield result completed⟩

/-- Completed projected output is exact and duplicate free even when the tree's
    leaves have repeated projections or the same total assignment appears twice. -/
theorem complete_exact [DecidableEq β] (project : α → β)
    {blocked : List β} {forest : List (Tree α)} {values : List α}
    (completed : Complete project blocked forest values) :
    (values.map project).Nodup ∧
      ∀ candidate, candidate ∈ values.map project ↔
        candidate ∈ (eligible project blocked forest).map project := by
  induction completed with
  | @done blocked forest fuel done =>
    have exact := next_preserves project blocked fuel forest
    rw [done] at exact
    simp only [Preserves] at exact
    simp [exact]
  | @yield blocked forest rest value values fuel found completed ih =>
    constructor
    · simp only [List.map_cons, List.nodup_cons]
      refine ⟨?_, ih.1⟩
      intro duplicate
      have h := (ih.2 (project value)).mp duplicate
      rw [mem_projected_eligible] at h
      exact h.2 (List.mem_cons_self)
    · intro candidate
      rw [found_projection_partition project blocked fuel forest rest value found candidate]
      simp only [List.map_cons, List.mem_cons, ih.2 candidate]

/-- A complete cursor trace covers exactly the original unblocked projections. -/
theorem completed_coverage [DecidableEq β] (project : α → β)
    {forest : List (Tree α)} {values : List α}
    (completed : Complete project [] forest values) (candidate : β) :
    candidate ∈ values.map project ↔ candidate ∈ (leaves forest).map project := by
  rw [(complete_exact project completed).2 candidate, mem_projected_eligible]
  simp

/-- Acceptance is an independent filter applied to every completed candidate.
    The cursor does not manufacture a semantic verdict from skipped leaves. -/
theorem accepted_outputs_exact [DecidableEq β] (project : α → β)
    (accepted : β → Bool) {forest : List (Tree α)} {values : List α}
    (completed : Complete project [] forest values) (candidate : β) :
    candidate ∈ (values.map project).filter accepted ↔
      candidate ∈ (leaves forest).map project ∧ accepted candidate = true := by
  simp only [List.mem_filter, completed_coverage project completed candidate]

open Classical in
/-- Instantiate the independent filter with the original formula reduct.
    This is a denotational assurance boundary, not a Rust oracle refinement. -/
theorem stable_outputs_exact [DecidableEq β] {γ : Type u}
    (project : α → β) (interpret : β → Atoms γ) (theory : Ferraris.Theory γ)
    {forest : List (Tree α)} {values : List α}
    (completed : Complete project [] forest values) (candidate : β) :
    let accepted := fun value => decide (Ferraris.Stable (interpret value) theory)
    candidate ∈ (values.map project).filter accepted ↔
      candidate ∈ (leaves forest).map project ∧
        Ferraris.Stable (interpret candidate) theory := by
  classical
  simpa using accepted_outputs_exact project
    (fun value => decide (Ferraris.Stable (interpret value) theory)) completed candidate

def repeatedAuxiliaryTree : List (Tree (Bool × Bool)) :=
  [.fork (.fork (.leaf (false, false)) (.leaf (false, true)))
    (.fork (.leaf (true, false)) (.leaf (true, true)))]

/-- Both Boolean candidates appear once despite their two auxiliary extensions. -/
theorem repeated_auxiliary_complete :
    Complete Prod.fst [] repeatedAuxiliaryTree [(false, false), (true, false)] := by
  exact Complete.yield (fuel := 3)
    (rest := [.leaf (false, true), .fork (.leaf (true, false)) (.leaf (true, true))])
    (by decide) (Complete.yield (fuel := 3) (rest := [.leaf (true, true)])
      (by decide) (Complete.done (fuel := 2) (by decide)))

/-- Resource exhaustion is observably different from exhausting the tree. -/
theorem zero_fuel_keeps_open_tree :
    next Prod.fst ([] : List Bool) 0 repeatedAuxiliaryTree =
      .interrupted repeatedAuxiliaryTree := rfl

end Zetesis.CandidateCursor
