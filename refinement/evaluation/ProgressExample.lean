import Progress

open Aeneas Aeneas.Std Result ControlFlow
open ZetesisExtract

namespace EvaluationProgressExample

/-- A concrete nonempty formula slice with no atom reads. -/
def nodes : Slice theory.Node := Slice.from [.False] (by scalar_tac)

/-- The empty interpretation of a zero-atom theory whose only formula is false.
Its formula slice is nonempty, although its packed atom storage is empty. -/
def candidate : theory.Interpretation := {
  theory := { owner := 0, value := { atoms := 0#usize, nodes := { slice := nodes }, roots := alloc.vec.Vec.new Usize } }
  words := alloc.vec.Vec.new U64
}

/-- The actual enumerated-slice representation starts at the first node. -/
def cursor : Evaluation.Cursor := { iter := { slice := nodes, i := 0 }, count := 0#usize }

/-- This invocation observes no cancellation and has no deadline. -/
def control : zetesis_cpu.cancellation.Cancellation := {
  cancelled := { owner := 1, value := { nextRead := false } }
  deadline := none
}

/-- Exactly one work unit is available to this node evaluation. -/
def limits : oracle.Limits := { max_work := 1#u64, max_subsets := 0#u64 }

/-- No prior work or subset has been charged. -/
def statistics : oracle.Statistics := { work := 0#u64, subsets := 0#u64 }

/-- The concrete generated body really has a nonempty successful transition:
it appends false, advances both indices to one and charges one work unit. This
constructs every structural/control premise rather than assuming a body result.
It proves node evaluation, not answer-set membership. -/
theorem false_node_advances :
    ∃ nextCursor : Evaluation.Cursor, ∃ output : alloc.vec.Vec Bool,
      ∃ nextStatistics : oracle.Statistics,
      oracle.evaluate_loop.body candidate none cursor (alloc.vec.Vec.new Bool)
        limits control statistics =
        ok (.cont (nextCursor, output, limits, control, nextStatistics)) ∧
      output.val = [false] ∧ nextCursor.iter.i = 1 ∧
      nextCursor.count.val = 1 ∧ nextStatistics.work.val = 1 := by
  have stored : Membership.Represented candidate := by
    intro atom inside
    simp [candidate] at inside
  obtain ⟨nextCount, nextWork, output, countAdvanced, workAdvanced, stepped,
      exactPrefix, _, _⟩ := EvaluationProgress.present_step
    candidate none cursor (alloc.vec.Vec.new Bool) limits control statistics
    (by simp [cursor]) (by simp [cursor]) (by simp [cursor, nodes]) stored
    (by simp [cursor, nodes, Evaluation.ChildrenPresent])
    (by intro mask member; cases member) rfl (by decide)
  have exactOutput : output.val = [false] := by
    simpa [cursor, nodes, Evaluation.masked, Evaluation.value] using exactPrefix
  have nextCountValue : nextCount.val = 1 := by
    simpa [cursor] using countAdvanced
  have nextWorkValue : nextWork.val = 1 := by
    simpa [statistics] using workAdvanced
  exact ⟨{ iter := { cursor.iter with i := cursor.iter.i + 1 }, count := nextCount },
    output, { statistics with work := nextWork }, stepped,
    exactOutput, rfl, nextCountValue, nextWorkValue⟩

end EvaluationProgressExample
