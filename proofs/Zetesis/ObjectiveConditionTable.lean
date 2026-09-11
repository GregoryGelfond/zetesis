import Zetesis.ObjectiveConditions

/-!
# Backward-indexed conditions on original models

A finite node table shares earlier Boolean subqueries by zero-based index. Its
interpreter visits every node in order and returns the last value, or true for
an empty table. Unfolding the same references yields the existing closed Query
syntax. The prefix invariant says that each stored Boolean is the original-model
evaluation of the corresponding unfolded query. Backward-reference admission
ensures every lookup succeeds; malformed tables return no completed result.

The laws connect these two mathematical representations and original Ferraris
satisfaction. They do not verify Rust indexing, atom conversion, allocations,
work/cancellation accounting, source bindings, objective eligibility or priority
presence. The interpretation must be the supplied original model: a displayed
projection is interchangeable only when atomwise identity is established.
-/

namespace Zetesis.ObjectiveConditionTable

universe u
variable {A : Type u}
open ObjectiveConditions (Query)

/-- Operand indices name nodes strictly earlier in the table. -/
inductive Node (A : Type u) where
  | boolean : Bool → Node A
  | atom : A → Node A
  | neg : Nat → Node A
  | conj : Nat → Nat → Node A
  | disj : Nat → Nat → Node A

/-- Every operand of this node is inside the already completed prefix. -/
def Backward (length : Nat) : Node A → Prop
  | .boolean _ | .atom _ => True
  | .neg operand => operand < length
  | .conj left right | .disj left right => left < length ∧ right < length

/-- Admission checks references before appending each node's result. -/
def AdmittedFrom : Nat → List (Node A) → Prop
  | _, [] => True
  | length, node :: rest => Backward length node ∧ AdmittedFrom (length + 1) rest

/-- A standalone table starts with no preceding nodes. -/
def Admitted (nodes : List (Node A)) : Prop := AdmittedFrom 0 nodes

/-- Unfold one node using exactly its earlier queries. -/
def unfoldNode (previous : List (Query A)) : Node A → Option (Query A)
  | .boolean value => some (.boolean value)
  | .atom atom => some (.atom atom)
  | .neg operand => do return .neg (← previous[operand]?)
  | .conj left right => do return .conj (← previous[left]?) (← previous[right]?)
  | .disj left right => do return .disj (← previous[left]?) (← previous[right]?)

/-- Evaluate one node using exactly its earlier Boolean results. -/
def evaluateNode (model : A → Bool) (previous : List Bool) : Node A → Option Bool
  | .boolean value => some value
  | .atom atom => some (model atom)
  | .neg operand => do return !(← previous[operand]?)
  | .conj left right => do return (← previous[left]?) && (← previous[right]?)
  | .disj left right => do return (← previous[left]?) || (← previous[right]?)

/-- Unfold the remaining finite table, preserving zero-based prefix indices. -/
def unfoldRows (previous : List (Query A)) : List (Node A) → Option (List (Query A))
  | [] => some previous
  | node :: rest => do
    let query ← unfoldNode previous node
    unfoldRows (previous ++ [query]) rest

/-- Visit every remaining node, appending one Boolean for each successful step. -/
def evaluateRows (model : A → Bool) (previous : List Bool) : List (Node A) → Option (List Bool)
  | [] => some previous
  | node :: rest => do
    let value ← evaluateNode model previous node
    evaluateRows model (previous ++ [value]) rest

/-- The final unfolded query is true when the table is empty. -/
def unfold (nodes : List (Node A)) : Option (Query A) := do
  let queries ← unfoldRows [] nodes
  return queries.getLast?.getD (.boolean true)

/-- The final result is true when the table is empty, as in a conjunction with
no conditions. A failed operand lookup supplies no result. -/
def evaluate (model : A → Bool) (nodes : List (Node A)) : Option Bool := do
  let values ← evaluateRows model [] nodes
  return values.getLast?.getD true

/-- One table step preserves the prefix correspondence, including failure.
Each operand lookup commutes with mapping original query truth; then the chosen
Boolean constructor has the same meaning on both sides. -/
theorem node_correspondence (model : A → Bool) (previous : List (Query A))
    (node : Node A) :
    evaluateNode model (previous.map (ObjectiveConditions.evaluate model)) node =
      (unfoldNode previous node).map (ObjectiveConditions.evaluate model) := by
  cases node with
  | boolean value => rfl
  | atom atom => rfl
  | neg operand =>
    simp only [evaluateNode, unfoldNode, List.getElem?_map]
    cases previous[operand]? <;> rfl
  | conj left right =>
    simp only [evaluateNode, unfoldNode, List.getElem?_map]
    cases previous[left]? <;> cases previous[right]? <;> rfl
  | disj left right =>
    simp only [evaluateNode, unfoldNode, List.getElem?_map]
    cases previous[left]? <;> cases previous[right]? <;> rfl

/-- The complete stored prefix corresponds pointwise to unfolded query truth.
Induct on the unvisited suffix. The next node agrees by node_correspondence;
append that corresponding pair and use the same invariant for the shorter suffix.
No admission hypothesis is needed for this equality of partial results. -/
theorem rows_correspondence (model : A → Bool) (nodes : List (Node A))
    (previous : List (Query A)) :
    evaluateRows model (previous.map (ObjectiveConditions.evaluate model)) nodes =
      (unfoldRows previous nodes).map (List.map (ObjectiveConditions.evaluate model)) := by
  induction nodes generalizing previous with
  | nil => rfl
  | cons node rest induction =>
    simp only [evaluateRows, unfoldRows, node_correspondence]
    cases next : unfoldNode previous node with
    | none => simp
    | some query =>
      simpa using induction (previous ++ [query])

private theorem lookup_exists {V : Type u} (previous : List V) (index : Nat)
    (inside : index < previous.length) : ∃ value, previous[index]? = some value := by
  exact ⟨previous[index], (List.getElem?_eq_some_getElem_iff inside).2 trivial⟩

/-- Backward references are sufficient for one unfolding step to succeed. -/
theorem backward_node_total (previous : List (Query A)) (node : Node A)
    (backward : Backward previous.length node) :
    ∃ query, unfoldNode previous node = some query := by
  cases node with
  | boolean value => exact ⟨.boolean value, rfl⟩
  | atom atom => exact ⟨.atom atom, rfl⟩
  | neg operand =>
    obtain ⟨query, found⟩ := lookup_exists previous operand backward
    exact ⟨.neg query, by simp [unfoldNode, found]⟩
  | conj left right =>
    obtain ⟨first, leftFound⟩ := lookup_exists previous left backward.1
    obtain ⟨second, rightFound⟩ := lookup_exists previous right backward.2
    exact ⟨.conj first second, by simp [unfoldNode, leftFound, rightFound]⟩
  | disj left right =>
    obtain ⟨first, leftFound⟩ := lookup_exists previous left backward.1
    obtain ⟨second, rightFound⟩ := lookup_exists previous right backward.2
    exact ⟨.disj first second, by simp [unfoldNode, leftFound, rightFound]⟩

/-- A finite admitted suffix completes with exactly one stored query per node.
The prefix length grows by one, which supplies the next admission premise;
the number of unvisited nodes strictly decreases. -/
theorem admitted_rows_total (nodes : List (Node A)) (previous : List (Query A))
    (admitted : AdmittedFrom previous.length nodes) :
    ∃ queries, unfoldRows previous nodes = some queries ∧
      queries.length = previous.length + nodes.length := by
  induction nodes generalizing previous with
  | nil => exact ⟨previous, rfl, by simp⟩
  | cons node rest induction =>
    obtain ⟨query, next⟩ := backward_node_total previous node admitted.1
    have tailAdmitted : AdmittedFrom (previous ++ [query]).length rest := by
      simpa using admitted.2
    obtain ⟨queries, complete, length⟩ := induction (previous ++ [query]) tailAdmitted
    refine ⟨queries, ?_, ?_⟩
    · simp [unfoldRows, next, complete]
    · simpa [Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using length

/-- The final table result agrees with its unfolded closed query. This includes
the empty table's true result and matching failure for malformed references. -/
theorem unfolding_correspondence (model : A → Bool) (nodes : List (Node A)) :
    evaluate model nodes = (unfold nodes).map (ObjectiveConditions.evaluate model) := by
  have rows := rows_correspondence model nodes []
  simp only [List.map_nil] at rows
  unfold evaluate unfold
  rw [rows]
  cases complete : unfoldRows [] nodes with
  | none => simp
  | some queries =>
    change some ((queries.map (ObjectiveConditions.evaluate model)).getLast?.getD true) =
      some (ObjectiveConditions.evaluate model (queries.getLast?.getD (.boolean true)))
    rw [List.getLast?_map]
    cases queries.getLast? <;> rfl

/-- An admitted finite node table has a closed query and the same completed
Boolean result. Backward admission establishes existence; unfolding
correspondence establishes original-model truth of that result. -/
theorem admitted_correspondence (model : A → Bool) (nodes : List (Node A))
    (admitted : Admitted nodes) :
    ∃ query, unfold nodes = some query ∧
      evaluate model nodes = some (ObjectiveConditions.evaluate model query) := by
  obtain ⟨queries, complete, _⟩ := admitted_rows_total nodes [] admitted
  have unfolded : unfold nodes = some (queries.getLast?.getD (.boolean true)) := by
    simp [unfold, complete]
  exact ⟨_, unfolded, by rw [unfolding_correspondence, unfolded]; rfl⟩

/-- A successfully unfolded table tests original Ferraris satisfaction, never a
frozen reduct. The query correspondence and the existing original_truth law
supply the two steps; no stable-model or source-eligibility claim is added. -/
theorem original_truth (model : A → Bool) (nodes : List (Node A)) (query : Query A)
    (unfolded : unfold nodes = some query) :
    evaluate model nodes = some true ↔
      Ferraris.Satisfies (fun atom => model atom = true) (ObjectiveConditions.formula query) := by
  rw [unfolding_correspondence, unfolded]
  simpa using ObjectiveConditions.original_truth model query

/-- Atomwise identical original models yield identical complete or failed table
results. A projection that omits true atoms does not satisfy this premise. -/
theorem model_identity (left right : A → Bool) (same : ∀ atom, left atom = right atom)
    (nodes : List (Node A)) : evaluate left nodes = evaluate right nodes := by
  have identical : left = right := funext same
  exact congrArg (fun model => evaluate model nodes) identical

end Zetesis.ObjectiveConditionTable
