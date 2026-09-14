import Zetesis.TableBindings

/-!
# Necessary domains preserve complete continuations

A domain at each positive source argument covers that argument's value in every
complete positive binding. Intersecting the domains of all occurrences of one
variable therefore remains necessary for that variable. Unknown contributes the
universal predicate; an empty finite meet excludes complete continuations.

A guard may reject a locally matching row with no complete continuation. The
local row-family equality premise of `TableBindings.join_family_preserved` is
therefore too strong for this consumer. The second law below preserves the
ordered flattened list of complete continuations directly. A continuation may
carry the whole source-occurrence/row-ID trace and the final binding, so order and
multiplicity are retained, not just a set of output values.

These laws require actual argument coverage and unchanged complete continuations
as explicit premises. They do not prove `zetesis_domain` soundness, normalized
source/IR correspondence, dictionary identity, Rust matching, accounting, authored
error preservation or stopped-prefix equivalence. They do not justify narrowing a
dependency projection or treating Unknown/Stopped as a finite empty domain.
-/

namespace Zetesis.DomainBindings

variable {Column Variable Value Row Result : Type}

/-- The intersection of every argument domain naming one source variable. -/
def Meet (scope : Column → Variable) (domains : Column → Value → Prop)
    (variable : Variable) (value : Value) : Prop :=
  ∀ column, scope column = variable → domains column value

/-- Complete argument coverage implies membership in each variable's meet.

For an occurrence naming the variable, substitute that equality into its
argument coverage. This applies to repeated occurrences without assuming their
predicate names or source positions differ. -/
theorem complete_binding_survives (scope : Column → Variable)
    (domains : Column → Value → Prop) (binding : Variable → Value)
    (covered : ∀ column, domains column (binding (scope column))) :
    ∀ variable, Meet scope domains variable (binding variable) := by
  intro variable column same
  simpa only [same] using covered column

/-- Removing rows without complete continuations preserves the exact ordered
completion list, including repeated row occurrences and repeated results.

The coverage premise only requires selection of rows that have a completed
result. Induct on the original row list. An accepted row retains its complete
list. A rejected row must have an empty list, otherwise its first result would
contradict coverage. The induction hypothesis supplies the unchanged tail.
This concerns successful completion, not resource/error-producing prefixes. -/
theorem guarded_continuations_exact (rows : List Row) (selected : Row → Bool)
    (finish : Row → List Result)
    (covered : ∀ row ∈ rows, ∀ result ∈ finish row, selected row = true) :
    (rows.filter selected).flatMap finish = rows.flatMap finish := by
  induction rows with
  | nil => rfl
  | cons row rest induction =>
    have tailCovered : ∀ row ∈ rest, ∀ result ∈ finish row, selected row = true := by
      intro row member result completed
      exact covered row (List.mem_cons_of_mem _ member) result completed
    have tailSame := induction tailCovered
    by_cases accepted : selected row = true
    · simpa [accepted] using congrArg (List.append (finish row)) tailSame
    · have empty : finish row = [] := by
        cases completion : finish row with
        | nil => rfl
        | cons result remaining =>
          exact False.elim (accepted (covered row (by simp) result (by simp [completion])))
      simpa [accepted, empty] using tailSame

end Zetesis.DomainBindings
