import Zetesis.ParallelGrounding

/-!
# Replaying a fixed zero-or-one prefix around ordered row tasks

The Rust premise is stronger than selected-probe cardinality alone: each skipped
prefix pattern is fully bound and its actual probe offers exactly one row. The
ordinary matcher can therefore either reject the prefix or produce the same
unchanged binding once. It still runs in every worker; these laws do not remove
matching, scans, charges, cancellation or complete-family obligations.

These laws reuse the ordered task law. Concrete descriptor/source ownership,
probe-order correspondence and matcher totality remain Rust obligations.
Standalone checks of the declaration bodies do not qualify a later module
layout; the maintained proof capture must cover the integrated source.
-/

namespace Zetesis.ParallelGrounding.SingletonPrefix

universe u v w
variable {Binding : Type u} {Row : Type v} {Output : Type w}

/-- Replaying one unchanged prefix binding around each block preserves exact
ordered outputs, including repeated occurrences and empty suffix results. -/
theorem singleton_prefix_outputs (initial : Binding) (widths : List Nat)
    (rows : List Row) (emit : Binding → Row → List Output) :
    (rowTasks widths rows).flatMap
      (fun task => [initial].flatMap (fun binding => task.flatMap (emit binding))) =
      [initial].flatMap (fun binding => rows.flatMap (emit binding)) := by
  simp only [List.flatMap_cons, List.flatMap_nil, List.append_nil]
  exact ordered_task_outputs_exact widths rows (emit initial)

/-- A failed ordinary singleton-prefix match returns no complete binding in
any block; completion of those empty results is still required operationally. -/
theorem rejected_prefix_outputs (widths : List Nat) (rows : List Row)
    (emit : Binding → Row → List Output) :
    (rowTasks widths rows).flatMap
      (fun task => ([] : List Binding).flatMap (fun binding => task.flatMap (emit binding))) =
      [] := by
  simp

/-- No task-order interchange is justified for a prefix with multiple bindings.
The concrete adapter deliberately stops before an unbound generator. -/
theorem multiple_prefixes_do_not_commute_with_blocks :
    ([[1], [2]] : List (List Nat)).flatMap
      (fun task => [10, 20].flatMap (fun initial => task.map (fun row => initial + row))) ≠
      [10, 20].flatMap (fun initial => [1, 2].map (fun row => initial + row)) := by
  decide

end Zetesis.ParallelGrounding.SingletonPrefix
