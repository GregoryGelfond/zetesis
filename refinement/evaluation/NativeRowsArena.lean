import NativeRows
import Zetesis.OperandArena

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract
open Zetesis.Refinement

/-!
# Native span contents in the reusable operand arena

This bridge maps the actual generated getter's exact machine-index row into
OperandArena's natural-index list. Conversion preserves the whole ordered row,
including duplicate occurrences. A separate backward-reference premise supplies
the topology that a storage getter alone does not establish.

No formula-table, evaluation-loop, work or runtime-effect refinement is claimed.
The generated row getter and its imported scalar/sequence model are those used
by NativeRows; no second implementation is assumed to agree with them.
-/
namespace NativeRowsArena

/-- Machine indices retain their exact natural values; no wrapping or rebasing
is performed by this mathematical representation map. -/
def span (stored : theory.OperandSpan) : OperandArena.Span :=
  ⟨stored.start.val, stored.length.val⟩

/-- A successful generated span read is exactly the general arena's ordered
contents after scalar conversion. The list equality preserves multiplicity. -/
theorem returned_contents (arena : Slice Usize) (stored : theory.OperandSpan)
    (result : Slice Usize)
    (returned : theory.read_span arena stored = ok (.Ok result)) :
    result.val.map UScalar.val =
      OperandArena.contents (arena.val.map UScalar.val) (span stored) := by
  have exactRow : result.val =
      (arena.val.drop stored.start.val).take stored.length.val :=
    (NativeRows.read_span_contents arena stored result returned).1
  rw [exactRow]
  simp only [OperandArena.contents, Zetesis.AdjacencyRows.slice, span,
    List.map_take, List.map_drop]

/-- A returned native row with every occurrence before the current node meets
the general arena's complete range-and-topology premise. The getter supplies
range validity; the explicitly separate hypothesis supplies child validity. -/
theorem returned_admitted (arena : Slice Usize) (stored : theory.OperandSpan)
    (result : Slice Usize) (earlier : Nat)
    (returned : theory.read_span arena stored = ok (.Ok result))
    (backwards : ∀ child ∈ result.val, child.val < earlier) :
    OperandArena.Admitted (arena.val.map UScalar.val) earlier (span stored) := by
  have accepted : 3 ≤ stored.length.val ∧
      stored.start.val + stored.length.val ≤ arena.val.length ∧
      NativeRows.row arena stored = result :=
    (NativeRows.read_span_accepts_iff arena stored result).mp returned
  have contents : result.val.map UScalar.val =
      OperandArena.contents (arena.val.map UScalar.val) (span stored) :=
    returned_contents arena stored result returned
  constructor
  · simpa only [OperandArena.Fits, span, List.length_map] using accepted.2.1
  · intro child member
    rw [← contents] at member
    obtain ⟨source, sourceMember, same⟩ := List.mem_map.mp member
    rw [← same]
    exact backwards source sourceMember

/-- Under explicit backward-reference validity, the reusable executable arena
validator accepts exactly the native getter's converted row. This supplies the
checked-row premise for the existing Boolean reduction laws without claiming
that the generated child validator or evaluator has already been refined. -/
theorem returned_validation (arena : Slice Usize) (stored : theory.OperandSpan)
    (result : Slice Usize) (earlier : Nat)
    (returned : theory.read_span arena stored = ok (.Ok result))
    (backwards : ∀ child ∈ result.val, child.val < earlier) :
    OperandArena.validate (arena.val.map UScalar.val) earlier (span stored) =
      some (result.val.map UScalar.val) := by
  apply (OperandArena.validate_exact _ _ _ _).mpr
  exact ⟨returned_admitted arena stored result earlier returned backwards,
    (returned_contents arena stored result returned).symm⟩

end NativeRowsArena
