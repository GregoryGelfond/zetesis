import Zetesis.DeltaRounds
import Zetesis.TableBindings

/-!
# Ordered grounding tasks and independent producer deltas

A row-task partition splits one immutable ordered row list into contiguous blocks.
Finishing tasks in any order is compatible with the theorem only when their
results are restored to task order before publication. Equal-valued occurrences,
empty tasks and empty output lists remain represented; this is list equality,
not equality after deduplication. The emitter is a total mathematical function;
typed source failures, cancellation and resource interruption require a separate
ordered-result/refusal protocol.

Independent producer merging is a different, set-level statement. Its premises
specify every read on which a producer's delta depends, bound every possible
write, and exclude both cross read/write intersections. The proof does not
establish those premises from a source graph, count, queue or positive predicate
scan. In particular, shared head writes can coalesce at the set level without
preserving origin multiplicity: ordered publication still has to do that work.

The completion laws distinguish queued/in-flight work from finished work. They
do not establish memory visibility, scheduler liveness, exact receipts, round
termination or reduct membership. No Rust, threaded or GPU implementation is
asserted correct by this file.
-/

namespace Zetesis.ParallelGrounding

universe u v
variable {Row : Type u} {Output : Type v} {Atom : Type u}

/-- Ordered contiguous blocks with an explicit final remainder. Zero widths and
widths beyond the remaining length are valid; neither loses row occurrences. -/
def rowTasks : List Nat → List Row → List (List Row)
  | [], rows => [rows]
  | width :: widths, rows => rows.take width :: rowTasks widths (rows.drop width)

/-- Concatenating the contiguous blocks reconstructs the source row list exactly.
Each step uses the take/drop partition; induction handles the remaining widths.
No distinctness assumption is imposed on row values. -/
theorem row_tasks_reconstruct (widths : List Nat) (rows : List Row) :
    (rowTasks widths rows).flatten = rows := by
  induction widths generalizing rows with
  | nil => simp [rowTasks]
  | cons width widths rest =>
    calc
      (rowTasks (width :: widths) rows).flatten =
          rows.take width ++ (rowTasks widths (rows.drop width)).flatten := by
            simp only [rowTasks, List.flatten_cons]
      _ = rows.take width ++ rows.drop width := by rw [rest]
      _ = rows := List.take_append_drop width rows

/-- Mapping every input occurrence to its complete output list and publishing
blocks in their original order gives precisely the sequential flatMap result.
An occurrence with zero outputs contributes the empty list, without authorizing
omission of its task's completion or performed-work receipt. -/
theorem ordered_task_outputs_exact (widths : List Nat) (rows : List Row)
    (emit : Row → List Output) :
    (rowTasks widths rows).flatMap (fun task => task.flatMap emit) = rows.flatMap emit := by
  induction widths generalizing rows with
  | nil => simp [rowTasks]
  | cons width widths rest =>
    calc
      (rowTasks (width :: widths) rows).flatMap (fun task => task.flatMap emit) =
          (rows.take width).flatMap emit ++
            (rowTasks widths (rows.drop width)).flatMap (fun task => task.flatMap emit) := by
              simp only [rowTasks, List.flatMap_cons]
      _ = (rows.take width).flatMap emit ++ (rows.drop width).flatMap emit := by rw [rest]
      _ = (rows.take width ++ rows.drop width).flatMap emit := List.flatMap_append.symm
      _ = rows.flatMap emit := by rw [List.take_append_drop]

/-- Exact task-local implementations may replace the mathematical emitter only
on the tasks actually selected. The coordinator still consumes task results in
partition order. This premise concerns completed successful task results. -/
theorem completed_task_results_exact (widths : List Nat) (rows : List Row)
    (emit : Row → List Output) (run : List Row → List Output)
    (completed : ∀ task ∈ rowTasks widths rows, run task = task.flatMap emit) :
    ((rowTasks widths rows).map run).flatten = rows.flatMap emit := by
  have sameTasks : (rowTasks widths rows).map run =
      (rowTasks widths rows).map (fun task => task.flatMap emit) :=
    List.map_congr_left completed
  rw [sameTasks]
  exact ordered_task_outputs_exact widths rows emit

/-- Splitting an empty-output task does not let output count stand for task
count: the partition retains exactly one task per width plus the remainder. -/
theorem task_count_includes_empty (widths : List Nat) (rows : List Row) :
    (rowTasks widths rows).length = widths.length + 1 := by
  induction widths generalizing rows with
  | nil => rfl
  | cons width widths rest => simp [rowTasks, rest, Nat.add_assoc]

/-- Empty blocks and rows yielding no output preserve output multiplicity/order. -/
theorem empty_output_partition_fixture :
    rowTasks [2, 0, 1] [0, 1, 2, 3] = [[0, 1], [], [2], [3]] ∧
    (rowTasks [2, 0, 1] [0, 1, 2, 3]).flatMap
      (fun task => task.flatMap (fun n => if n % 2 = 0 then [n, n] else [])) =
        [0, 0, 2, 2] := by decide

/-- Completion-order publication is not an ordered partition implementation. -/
theorem completion_order_is_not_source_order :
    ([[0, 1], [2, 3]] : List (List Nat)).flatten ≠ [[2, 3], [0, 1]].flatten := by decide

/-- All state information affecting a delta must be included in this footprint.
This is an explicit extensional premise, not a claim about graph discovery. -/
def ReadsOnly (reads : Atoms Atom) (delta : Atoms Atom → Atoms Atom) : Prop :=
  ∀ before after, EqOn reads before after → delta before = delta after

/-- Every emitted atom belongs to the producer's authenticated write footprint. -/
def WritesOnly (writes : Atoms Atom) (delta : Atoms Atom → Atoms Atom) : Prop :=
  ∀ snapshot, Sub (delta snapshot) writes

/-- Publishing another producer's bounded writes does not affect a delta when
its full read footprint is disjoint from those writes. The atom-level argument
first establishes agreement on reads; the extensional premise then applies. -/
theorem delta_stable_under_disjoint_publication
    (reads writes snapshot : Atoms Atom) (delta other : Atoms Atom → Atoms Atom)
    (depends : ReadsOnly reads delta) (bounded : WritesOnly writes other)
    (separate : ∀ atom, reads atom → ¬ writes atom) :
    delta (Union snapshot (other snapshot)) = delta snapshot := by
  have readsUnchanged : EqOn reads (Union snapshot (other snapshot)) snapshot := by
    intro atom read
    constructor
    · rintro (old | new)
      · exact old
      · exact False.elim (separate atom read (bounded snapshot atom new))
    · exact Or.inl
  exact depends _ _ readsUnchanged

/-- Two independently computed deltas have the same final consequence set as
either serial publication order. Both read/write exclusions are required for
both orders; no write/write exclusion is needed for predicate-set union.
This does not equate source-provenance lists or allocation/work traces. -/
theorem independent_deltas_merge
    (leftReads leftWrites rightReads rightWrites snapshot : Atoms Atom)
    (left right : Atoms Atom → Atoms Atom)
    (leftDepends : ReadsOnly leftReads left) (rightDepends : ReadsOnly rightReads right)
    (leftBounded : WritesOnly leftWrites left) (rightBounded : WritesOnly rightWrites right)
    (leftUnaffected : ∀ atom, leftReads atom → ¬ rightWrites atom)
    (rightUnaffected : ∀ atom, rightReads atom → ¬ leftWrites atom) :
    Union (Union snapshot (left snapshot)) (right (Union snapshot (left snapshot))) =
        Union snapshot (Union (left snapshot) (right snapshot)) ∧
    Union (Union snapshot (right snapshot)) (left (Union snapshot (right snapshot))) =
        Union snapshot (Union (left snapshot) (right snapshot)) := by
  have sameRight : right (Union snapshot (left snapshot)) = right snapshot :=
    delta_stable_under_disjoint_publication rightReads leftWrites snapshot right left
      rightDepends leftBounded rightUnaffected
  have sameLeft : left (Union snapshot (right snapshot)) = left snapshot :=
    delta_stable_under_disjoint_publication leftReads rightWrites snapshot left right
      leftDepends rightBounded leftUnaffected
  constructor
  · rw [sameRight]
    apply atoms_ext
    intro atom
    simp only [Union, or_assoc]
  · rw [sameLeft]
    apply atoms_ext
    intro atom
    simp only [Union, or_left_comm, or_comm]

/-- At one fixed snapshot, merging two covered producer deltas preserves the
full synchronous headed step when both old head histories were published.
This invokes the existing DeltaRounds law separately for each producer. It does
not establish readiness, serial in-place equivalence, constraint latches or
termination; independent_deltas_merge supplies the separate serial-order law. -/
theorem covered_delta_merge_exact {LeftBinding RightBinding : Type v}
    (left : Lifted.Template Atom LeftBinding) (right : Lifted.Template Atom RightBinding)
    (seed old current : Atoms Atom)
    (leftSelected : Lifted.Bindings LeftBinding) (rightSelected : Lifted.Bindings RightBinding)
    (leftHistory : Sub (Lifted.DirectConsequence left seed old) current)
    (rightHistory : Sub (Lifted.DirectConsequence right seed old) current)
    (leftCovered : DeltaRounds.DeltaCoverage left seed old current leftSelected)
    (rightCovered : DeltaRounds.DeltaCoverage right seed old current rightSelected) :
    Union current (Union (Lifted.MaterializedTransform left seed current leftSelected)
      (Lifted.MaterializedTransform right seed current rightSelected)) =
    Union current (Union (Lifted.DirectConsequence left seed current)
      (Lifted.DirectConsequence right seed current)) := by
  have leftExact := DeltaRounds.delta_step_exact left seed old current leftSelected
    leftHistory leftCovered
  have rightExact := DeltaRounds.delta_step_exact right seed old current rightSelected
    rightHistory rightCovered
  have regroup : ∀ first second : Atoms Atom,
      Union current (Union first second) = Union (Union current first) (Union current second) := by
    intro first second
    apply atoms_ext
    intro atom
    constructor
    · rintro (retained | fromFirst | fromSecond)
      · exact Or.inl (Or.inl retained)
      · exact Or.inl (Or.inr fromFirst)
      · exact Or.inr (Or.inr fromSecond)
    · rintro ((retained | fromFirst) | (retained | fromSecond))
      · exact Or.inl retained
      · exact Or.inr (Or.inl fromFirst)
      · exact Or.inl retained
      · exact Or.inr (Or.inr fromSecond)
  calc
    _ = Union (Union current (Lifted.MaterializedTransform left seed current leftSelected))
        (Union current (Lifted.MaterializedTransform right seed current rightSelected)) := regroup _ _
    _ = Union (Union current (Lifted.DirectConsequence left seed current))
        (Union current (Lifted.DirectConsequence right seed current)) := by rw [leftExact, rightExact]
    _ = _ := (regroup _ _).symm

/-- This mathematical phase distinguishes enrollment from completed publication.
`completed` must be refined to successful publication by the concrete scheduler. -/
inductive TaskPhase where
  | waiting
  | queued
  | running
  | completed
  deriving DecidableEq

/-- All prerequisite publications, not merely queue insertions, must be complete. -/
def Ready (prerequisites : Nat → List Nat) (phase : Nat → TaskPhase) (task : Nat) : Prop :=
  ∀ dependency ∈ prerequisites task, phase dependency = .completed

/-- Any unfinished prerequisite blocks readiness, even when it is already queued
or running, and even when it will ultimately publish an empty delta. -/
theorem unfinished_dependency_blocks (prerequisites : Nat → List Nat)
    (phase : Nat → TaskPhase) (task dependency : Nat)
    (required : dependency ∈ prerequisites task) (unfinished : phase dependency ≠ .completed) :
    ¬ Ready prerequisites phase task := by
  intro ready
  exact unfinished (ready dependency required)

/-- Changing one task to Running cannot establish a dependent's readiness.
Starting work is not publication; every dependency of an already ready task
must have another identity and therefore retain its previous phase. -/
theorem starting_cannot_create_readiness (prerequisites : Nat → List Nat)
    (phase : Nat → TaskPhase) (started task : Nat)
    (ready : Ready prerequisites (fun i => if i = started then .running else phase i) task) :
    Ready prerequisites phase task := by
  intro dependency required
  have finished := ready dependency required
  by_cases same : dependency = started
  · simp [same] at finished
  · simpa [same] using finished

end Zetesis.ParallelGrounding
