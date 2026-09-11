import Zetesis.WorldViews

/-!
# Observations of completed original answer families

A term query uses a supplied finite binding family, model-relative truth and a
completed finite value family for each binding. Its output is a set of enabled
values: duplicate derivations of the same whole value do not create new terms.
Binding coverage and expression-value coverage are separate premises. Neither
premise follows from a successful parse or an unfinished evaluation.

A displayed record retains its original interpretation. Decorating a list of
answers preserves its order and multiplicity, even when distinct models have
equal displays. Erasing original identity before deduplicating is a different
operation. These laws do not establish answer-set membership or coverage; the
world-view corollary retains that explicit premise from `WorldViews`.

`ProjectedConditionals` supplies the complete condition-row, source-alternative
and anonymous-witness quantifier order. `FiniteValues.failed_step` describes a
failed scalar computation without publishing a partial row. The laws below
apply only after complete evaluation, not to an error prefix. Concrete source
safety, arithmetic, tuple construction, atom matching, ownership accounting,
resource/cancellation completion and Rust correspondence remain unproved.
-/

namespace Zetesis.Observations
universe u v w
variable {B : Type u} {V : Type v} {M : Type u} {D : Type v} {A : Type w}

/-- Distinct term membership from complete values of enabled rows. `enabled`
    tests the supplied interpretation, not the possible-support relation. -/
def Emits (rows : List B) (enabled : B → Prop) (values : B → List V) (value : V) : Prop :=
  ∃ row ∈ rows, enabled row ∧ value ∈ values row

/-- Complete binding coverage and exact value expansion establish precisely the
    enabled logical values. Soundness excludes spurious bindings and values;
    completeness retains every intended enabled value. Both use the same row,
    so values cannot cross between unrelated source alternatives. -/
theorem completed_terms_exact (rows : List B) (valid enabled : B → Prop)
    (values : B → List V) (denotes : B → V → Prop)
    (bindings : ∀ row, row ∈ rows ↔ valid row)
    (expansion : ∀ row, valid row → ∀ value, value ∈ values row ↔ denotes row value)
    (value : V) :
    Emits rows enabled values value ↔ ∃ row, valid row ∧ enabled row ∧ denotes row value := by
  have sound : Emits rows enabled values value →
      ∃ row, valid row ∧ enabled row ∧ denotes row value := by
    rintro ⟨row, present, active, generated⟩
    have admitted : valid row := (bindings row).mp present
    have meaning : denotes row value := (expansion row admitted value).mp generated
    exact ⟨row, admitted, active, meaning⟩
  have complete : (∃ row, valid row ∧ enabled row ∧ denotes row value) →
      Emits rows enabled values value := by
    rintro ⟨row, admitted, active, meaning⟩
    have present : row ∈ rows := (bindings row).mpr admitted
    have generated : value ∈ values row := (expansion row admitted value).mpr meaning
    exact ⟨row, present, active, generated⟩
  exact ⟨sound, complete⟩

/-- Reordering or repeating complete rows preserves term membership. Equality
    of only a tuple's measure is insufficient: this premise retains whole rows
    and their associated enabled value families. -/
theorem same_rows_preserve_terms (first second : List B)
    (enabled : B → Prop) (values : B → List V)
    (same : ∀ row, row ∈ first ↔ row ∈ second) (value : V) :
    Emits first enabled values value ↔ Emits second enabled values value := by
  constructor
  · rintro ⟨row, present, active, generated⟩
    exact ⟨row, (same row).mp present, active, generated⟩
  · rintro ⟨row, present, active, generated⟩
    exact ⟨row, (same row).mpr present, active, generated⟩

/-- Distinct complete tuple keys can carry equal measures. Deduplicating those
    measures before reducing the complete key set changes its size. -/
theorem equal_measures_do_not_identify_tuple_keys :
    let keys : List (Nat × Nat) := [(1, 0), (1, 1)]
    keys.eraseDups.length = 2 ∧ (keys.map Prod.fst).eraseDups.length = 1 := by
  decide

/-- A completed display augments each original model instead of replacing it. -/
def decorate (answers : List M) (display : M → D) : List (M × D) :=
  answers.map (fun model => (model, display model))

/-- Removing only the added display recovers the exact original list, including
    order and duplicate occurrences. The display need not be injective. -/
theorem original_family_projection (answers : List M) (display : M → D) :
    (decorate answers display).map Prod.fst = answers := by
  simp [decorate, List.map_map, Function.comp_def]

/-- An already complete world view remains complete when every displayed record
    retains its original model. The premise supplies stability and coverage;
    observation supplies neither. -/
theorem decorated_world_view (theory : Ferraris.Theory A)
    (answers : List (Atoms A)) (display : Atoms A → D)
    (original : WorldViews.Represents theory answers) :
    WorldViews.Represents theory ((decorate answers display).map Prod.fst) := by
  rw [original_family_projection]
  exact original

/-- Equal displays cannot merge distinct models while records retain their
    original-model component. This applies to any display policy or selection. -/
theorem different_models_remain_different_records (first second : M)
    (display : M → D) (different : first ≠ second) :
    (first, display first) ≠ (second, display second) := by
  intro same_record
  have same_model : first = second := congrArg Prod.fst same_record
  exact different same_model

/-- Erasing hidden identity before display deduplication can lose one of two
    original records. This representation counterexample does not assert that
    these numbers encode answer sets of a particular source program. -/
theorem display_deduplication_can_shrink_a_family :
    let answers : List Nat := [0, 1]
    let display : Nat → Nat := fun _ => 0
    (decorate answers display).length = 2 ∧
      ((decorate answers display).map Prod.snd).eraseDups.length = 1 := by
  decide

end Zetesis.Observations
