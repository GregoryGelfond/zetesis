import Std

/-!
# Dependency-respecting evaluation schedules

A node evaluator reads only its declared dependencies. A reference valuation
satisfies every local evaluation equation. Starting from agreement on an
established set of nodes, a finite schedule may assign a node once all its
dependencies are established. Each assignment preserves that agreement and
establishes its own result. Coverage then gives exactly the reference values at
the observed roots, in their original order and with repeated roots retained.

This is the usual invariant proof for evaluation of an acyclic dependency graph.
It applies to original formula truth, before reduct masking. It does not assume
the conclusion that the executed schedule has the reference values: agreement
is derived from dependency readiness and the evaluator's local read contract.

The schedule here is sequential and finite. Grouping independent nodes into
levels, constructing a complete packed schedule, mapping original IDs, machine
word bounds, concurrent writes, barriers and device visibility remain Rust/WGSL
correspondence obligations. No shader or answer-set verification is claimed.
-/

namespace Zetesis.DependencySchedule

universe u
variable {α : Type u}

/-- Truth stored at each original node identity. Unestablished values are
arbitrary; a ready evaluator cannot depend on them. -/
abbrev Valuation (α : Type u) := α → Bool

/-- Two valuations agree on every listed node; list order is immaterial here. -/
def Agrees (nodes : List α) (left right : Valuation α) : Prop :=
  ∀ node, node ∈ nodes → left node = right node

/-- The local evaluator is insensitive to values outside its dependencies. -/
def ReadsOnly (dependencies : α → List α)
    (evaluate : α → Valuation α → Bool) : Prop :=
  ∀ node left right, Agrees (dependencies node) left right →
    evaluate node left = evaluate node right

/-- Every reference value satisfies its node's local evaluation equation. -/
def Reference (evaluate : α → Valuation α → Bool) (reference : Valuation α) : Prop :=
  ∀ node, evaluate node reference = reference node

/-- Every read of the next assignment names an already established node.
Repeated assignments are permitted: each must independently satisfy readiness. -/
def Admissible (dependencies : α → List α) (established : List α) : List α → Prop
  | [] => True
  | node :: rest =>
      (∀ child, child ∈ dependencies node → child ∈ established) ∧
      Admissible dependencies (node :: established) rest

/-- Evaluate from the current valuation, then replace only the target node. -/
def assign [DecidableEq α] (evaluate : α → Valuation α → Bool)
    (current : Valuation α) (node : α) : Valuation α :=
  fun index => if index = node then evaluate node current else current index

/-- Execute every finite scheduled assignment, preserving its order. -/
def execute [DecidableEq α] (evaluate : α → Valuation α → Bool)
    (current : Valuation α) (schedule : List α) : Valuation α :=
  schedule.foldl (assign evaluate) current

/-- A ready local evaluation has the reference value: its dependencies agree,
and its reference evaluation satisfies the same local equation. -/
theorem ready_evaluation (dependencies : α → List α)
    (evaluate : α → Valuation α → Bool) (reads : ReadsOnly dependencies evaluate)
    (reference current : Valuation α) (correct : Reference evaluate reference)
    (established : List α) (agreement : Agrees established current reference)
    (node : α) (ready : ∀ child, child ∈ dependencies node → child ∈ established) :
    evaluate node current = reference node := by
  have dependency_agreement : Agrees (dependencies node) current reference := by
    intro child present
    exact agreement child (ready child present)
  calc
    evaluate node current = evaluate node reference :=
      reads node current reference dependency_agreement
    _ = reference node := correct node

/-- Assigning a ready node establishes its value without losing any previously
established value. The target case uses local evaluation; all other writes are
excluded by the assignment's definition. -/
theorem assignment_preserves_agreement [DecidableEq α]
    (dependencies : α → List α) (evaluate : α → Valuation α → Bool)
    (reads : ReadsOnly dependencies evaluate) (reference current : Valuation α)
    (correct : Reference evaluate reference) (established : List α)
    (agreement : Agrees established current reference) (node : α)
    (ready : ∀ child, child ∈ dependencies node → child ∈ established) :
    Agrees (node :: established) (assign evaluate current node) reference := by
  have value_correct : evaluate node current = reference node :=
    ready_evaluation dependencies evaluate reads reference current correct
      established agreement node ready
  intro index present
  by_cases target : index = node
  · subst index
    simpa [assign] using value_correct
  · have old : index ∈ established := by
      simpa only [List.mem_cons, target, false_or] using present
    simpa [assign, target] using agreement index old

/-- A complete admissible schedule establishes the reference values of all
scheduled nodes and preserves the initial agreement. Induction maintains that
invariant after each local assignment; it does not inspect unestablished values.
-/
theorem execution_preserves_agreement [DecidableEq α]
    (dependencies : α → List α) (evaluate : α → Valuation α → Bool)
    (reads : ReadsOnly dependencies evaluate) (reference : Valuation α)
    (correct : Reference evaluate reference) (schedule established : List α)
    (current : Valuation α) (admitted : Admissible dependencies established schedule)
    (agreement : Agrees established current reference) :
    Agrees (schedule ++ established) (execute evaluate current schedule) reference := by
  induction schedule generalizing established current with
  | nil => simpa [execute] using agreement
  | cons node rest tail =>
    have next_agreement :
        Agrees (node :: established) (assign evaluate current node) reference :=
      assignment_preserves_agreement dependencies evaluate reads reference current
        correct established agreement node admitted.1
    have completed :
        Agrees (rest ++ node :: established)
          (execute evaluate (assign evaluate current node) rest) reference :=
      tail (node :: established) (assign evaluate current node) admitted.2 next_agreement
    intro index present
    have covered : index ∈ rest ++ node :: established := by
      simp only [List.cons_append, List.mem_cons, List.mem_append] at present
      rcases present with target | remaining | initial
      · exact List.mem_append_right rest (List.mem_cons.mpr (Or.inl target))
      · exact List.mem_append_left _ remaining
      · exact List.mem_append_right rest (List.mem_cons.mpr (Or.inr initial))
    exact completed index covered

/-- Covered roots observe the reference valuation in exactly their given order.
Agreement follows from execution, and mapping preserves every root occurrence.
-/
theorem observed_roots_equal [DecidableEq α]
    (dependencies : α → List α) (evaluate : α → Valuation α → Bool)
    (reads : ReadsOnly dependencies evaluate) (reference : Valuation α)
    (correct : Reference evaluate reference) (schedule established roots : List α)
    (current : Valuation α) (admitted : Admissible dependencies established schedule)
    (agreement : Agrees established current reference)
    (coverage : ∀ root, root ∈ roots → root ∈ schedule ++ established) :
    roots.map (execute evaluate current schedule) = roots.map reference := by
  have completed :
      Agrees (schedule ++ established) (execute evaluate current schedule) reference :=
    execution_preserves_agreement dependencies evaluate reads reference correct
      schedule established current admitted agreement
  apply List.map_congr_left
  intro root present
  exact completed root (coverage root present)

end Zetesis.DependencySchedule
