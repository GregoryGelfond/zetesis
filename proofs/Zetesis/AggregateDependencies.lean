import Zetesis.AggregateAssignment
import Zetesis.BindingScopes
import Zetesis.Ferraris

/-!
# Aggregate candidate families indexed by completed predecessors

A dependent aggregate selects a finite candidate family for each completed
outer row. This is a dependent sum of carriers, not an independent Cartesian
product. Rows retain their predecessor association, including equal child
values obtained from different parents. Candidate coverage composes when the
actual outer row is covered and its own child family covers the actual result.

The sum specialization uses the existing complete-full-tuple coverage theorem.
An emitted clause retains both its predecessor equalities/activation and the
dependent aggregate equality. Original and arbitrary frozen M/J laws describe
that same clause family, without replacing a proposal by aggregate truth.

The reuse layer gives a finite algorithm over one retained parent frame, one
successful carrier and its cursor position. It reads the declared inputs again
at restart, rewinds on equal projections and otherwise rebuilds. A fixed-support
builder consumes only the complete projected input row. The traversal preserves
ordered parent/child rows, typed failures and the original clause sequence.

The separate support-reuse layer combines completed possible-head images and
idempotent defined/first-zero evidence. Under complete traversal and projection
sufficiency, coalescing consecutive equal projections preserves that observation.
It does not preserve formula-witness occurrences or apply to unfinished results.

The coverage laws assume complete carriers and total value/tuple functions.
The reuse laws do not prove that a source compiler finds every dependency or
that a successful builder supplies a complete carrier. Membership coverage still
needs the premises of `covered_extension` or `covered_sum`; the clause theorem
identifies successful builds with the given family. The traversal compares whole
completed row lists or typed failures, not published prefixes before interruption.
Rust free-input extraction, source safety, topological scheduling, local joins,
machine arithmetic, possible-support completion, cancellation and resource
accounting remain unproved bridges.
Cyclic source bindings have no completeness premise supplied by this module.
No Rust cursor, cache or WGSL refinement is claimed.
-/

namespace Zetesis.AggregateDependencies
open Ferraris
universe u v w
variable {V : Type u} {W : Type v} {A : Type w}

def rows (carrier : List V) (family : V → List W) : List (V × W) :=
  carrier.flatMap (fun parent => (family parent).map (fun child => (parent, child)))

/-- A child belongs to its own predecessor's carrier, not a union of families. -/
theorem row_membership (carrier : List V) (family : V → List W)
    (parent : V) (child : W) :
    (parent, child) ∈ rows carrier family ↔ parent ∈ carrier ∧ child ∈ family parent := by
  simp only [rows, List.mem_flatMap, List.mem_map]
  constructor
  · rintro ⟨outer, member, value, inside, same⟩
    cases same
    exact ⟨member, inside⟩
  · rintro ⟨member, inside⟩
    exact ⟨parent, member, child, inside, rfl⟩

/-- Completed coverage composes for a dependent next value. -/
theorem covered_extension (carrier : List V) (family : V → List W)
    (parent : V) (actual : W) (covered : parent ∈ carrier)
    (child_covered : actual ∈ family parent) :
    (parent, actual) ∈ rows carrier family :=
  (row_membership carrier family parent actual).mpr ⟨covered, child_covered⟩

/-- Full-tuple coverage for the actual predecessor covers its signed sum.
    Tuple eligibility may be correlated; no independence premise is required. -/
theorem covered_sum (carrier : List V)
    (tuples : V → List AggregateAssignment.FullTuple)
    (elements : V → List AggregateAssignment.Element)
    (weight : AggregateAssignment.FullTuple → Int) (parent : V)
    (covered : parent ∈ carrier)
    (tuple_coverage : ∀ element, element ∈ elements parent → element.eligible = true →
      element.tuple ∈ tuples parent) :
    (parent, AggregateAssignment.actualSum weight (elements parent)) ∈ rows carrier
      (fun outer => AggregateAssignment.subsetSums
        ((AggregateAssignment.unique (tuples outer)).map weight)) := by
  have child_covered := AggregateAssignment.actual_tuple_sum_is_candidate
    weight (tuples parent) (elements parent) tuple_coverage
  exact covered_extension carrier _ parent _ covered child_covered

/-- A further dependent family preserves the complete two-value prefix. -/
theorem covered_continuation {X : Type w} (carrier : List V)
    (family : V → List W) (continuation : V × W → List X)
    (parent : V) (child : W) (actual : X)
    (covered : parent ∈ carrier) (child_covered : child ∈ family parent)
    (next_covered : actual ∈ continuation (parent, child)) :
    ((parent, child), actual) ∈ rows (rows carrier family) continuation := by
  have prefix_covered : (parent, child) ∈ rows carrier family :=
    covered_extension carrier family parent child covered child_covered
  exact covered_extension (rows carrier family) continuation _ actual prefix_covered next_covered

/-- Later predecessor rows contribute their own complete families even when
    earlier rows contain equal parent values or equal child values. -/
theorem appended_predecessors (before after : List V) (family : V → List W) :
    rows (before ++ after) family = rows before family ++ rows after family := by
  simp [rows, List.flatMap_append]

def clause (previous equality head : Formula A) : Formula A :=
  .imp (.conj previous equality) head

def clauses (carrier : List V) (family : V → List W)
    (previous : V → Formula A) (equality head : V → W → Formula A) : Theory A :=
  (rows carrier family).map
    (fun row => clause (previous row.1) (equality row.1 row.2) (head row.1 row.2))

/-- Each original row is conditional on both its prior activation/equalities
    and its own aggregate equality. Membership alone proves neither condition. -/
theorem original_rows (M : Atoms A) (carrier : List V) (family : V → List W)
    (previous : V → Formula A) (equality head : V → W → Formula A) :
    Models M (clauses carrier family previous equality head) ↔
      ∀ parent ∈ carrier, ∀ child ∈ family parent,
        Satisfies M (previous parent) ∧ Satisfies M (equality parent child) →
          Satisfies M (head parent child) := by
  constructor
  · intro model parent member child inside
    have row := covered_extension carrier family parent child member inside
    have satisfied := model _ (List.mem_map.mpr ⟨(parent, child), row, rfl⟩)
    exact satisfied
  · intro all formula member
    obtain ⟨⟨parent, child⟩, row, rfl⟩ := List.mem_map.mp member
    have association := (row_membership carrier family parent child).mp row
    exact all parent association.1 child association.2

/-- Reduct formation preserves the exact complete clause associated with each
    dependent row. The frozen interpretation is arbitrary, including J ⊄ M. -/
theorem frozen_rows (M J : Atoms A) (carrier : List V) (family : V → List W)
    (previous : V → Formula A) (equality head : V → W → Formula A) :
    Models J (ReductTheory M (clauses carrier family previous equality head)) ↔
      ∀ parent ∈ carrier, ∀ child ∈ family parent,
        Satisfies J (Reduct M
          (clause (previous parent) (equality parent child) (head parent child))) := by
  constructor
  · intro model parent member child inside
    have row := covered_extension carrier family parent child member inside
    have original : clause (previous parent) (equality parent child) (head parent child) ∈
        clauses carrier family previous equality head :=
      List.mem_map.mpr ⟨(parent, child), row, rfl⟩
    exact model _ (List.mem_map.mpr ⟨_, original, rfl⟩)
  · intro all formula member
    obtain ⟨original, original_member, rfl⟩ := List.mem_map.mp member
    obtain ⟨⟨parent, child⟩, row, rfl⟩ := List.mem_map.mp original_member
    have association := (row_membership carrier family parent child).mp row
    exact all parent association.1 child association.2

namespace Reuse

universe s d e
variable {Support : Type s} {Data : Type d} {Error : Type e}

/-- The preceding relational frame is retained, without a separate stored key.
Only one successful value list and a position in that list belong to the cursor. -/
structure Cursor (Data : Type d) (W : Type v) where
  parent : StructuralBindings.Binding Data
  values : List W
  position : Nat

/-- Missing inputs and a failed family build are separate incomplete outcomes. -/
inductive Failure (Error : Type e) where
  | missingInput
  | build (error : Error)
  deriving DecidableEq

/-- The builder reads one fixed support and the complete projected input row.
Its type excludes access to unrelated parent slots. Source dependency extraction
and support completeness remain obligations of the caller. -/
abbrev Build (Support : Type s) (Data : Type d) (W : Type v) (Error : Type e) :=
  Support → List Data → Except Error (List W)

private def buildValues (support : Support) (build : Build Support Data W Error)
    (inputs : List Data) : Except (Failure Error) (List W) :=
  match build support inputs with
  | .error error => .error (.build error)
  | .ok values => .ok values

/-- Rebuild directly; absent input values never become a successful empty list. -/
def compute (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) (parent : StructuralBindings.Binding Data) :
    Except (Failure Error) (List W) :=
  match BindingScopes.readAll parent inputs with
  | none => .error .missingInput
  | some values => buildValues support build values

private def fresh (parent : StructuralBindings.Binding Data) (values : List W) :
    Cursor Data W := ⟨parent, values, 0⟩

/-- Read both projections at restart. A hit reuses only the successful list and
resets its position; a miss publishes a fresh cursor only after the build succeeds.
The support parameter stays fixed throughout a traversal. -/
def restart [DecidableEq Data] (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) (parent : StructuralBindings.Binding Data)
    (retained : Option (Cursor Data W)) : Except (Failure Error) (Cursor Data W) :=
  match BindingScopes.readAll parent inputs with
  | none => .error .missingInput
  | some values =>
    match retained with
    | none => (buildValues support build values).map (fresh parent)
    | some cursor =>
      if BindingScopes.readAll cursor.parent inputs = some values then
        .ok (fresh parent cursor.values)
      else (buildValues support build values).map (fresh parent)

/-- A retained list was successfully built for its retained parent under this
same support and builder. Cursor position does not establish carrier validity. -/
def Valid (support : Support) (inputs : List Nat) (build : Build Support Data W Error) :
    Option (Cursor Data W) → Prop
  | none => True
  | some cursor => compute support inputs build cursor.parent = .ok cursor.values

/-- Agreement on the declared input slots preserves the complete ordered build
or the same typed failure. This follows from checked projection, not a separate
assumption that two opaque family functions happen to agree. -/
theorem compute_agrees (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) (left right : StructuralBindings.Binding Data)
    (agreement : ∀ slot ∈ inputs, left slot = right slot) :
    compute support inputs build left = compute support inputs build right := by
  have sameReads : BindingScopes.readAll left inputs = BindingScopes.readAll right inputs :=
    BindingScopes.readAll_agrees left right inputs agreement
  simp only [compute, sameReads]

/-- Restart equals direct rebuilding with a zero cursor, including both failure
kinds. In the reuse case, the retained success and equal complete reads determine
the same build; in every other case the algorithm executes that build directly. -/
theorem restart_exact [DecidableEq Data] (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) (parent : StructuralBindings.Binding Data)
    (retained : Option (Cursor Data W)) (valid : Valid support inputs build retained) :
    restart support inputs build parent retained =
      (compute support inputs build parent).map (fresh parent) := by
  cases reads : BindingScopes.readAll parent inputs with
  | none => simp [restart, compute, reads, Except.map]
  | some values =>
    cases retained with
    | none => simp [restart, compute, reads]
    | some cursor =>
      by_cases sameReads : BindingScopes.readAll cursor.parent inputs = some values
      · have retainedBuild : buildValues support build values = .ok cursor.values := by
          simpa only [Valid, compute, sameReads] using valid
        simp [restart, compute, reads, sameReads, retainedBuild, Except.map]
      · simp [restart, compute, reads, sameReads]

/-- The unread suffix is a denotation of the one retained list, not another
stored carrier. A successful step consumes its first occurrence, including repeats. -/
def remaining (cursor : Cursor Data W) : List W := cursor.values.drop cursor.position

private def advance (cursor : Cursor Data W) : Cursor Data W :=
  { cursor with position := cursor.position + 1 }

def next (cursor : Cursor Data W) : Option (W × Cursor Data W) :=
  match remaining cursor with
  | [] => none
  | value :: _ => some (value, advance cursor)

/-- One successful cursor step partitions the exact ordered remaining values. -/
theorem next_preserves (cursor after : Cursor Data W) (value : W)
    (yielded : next cursor = some (value, after)) :
    remaining cursor = value :: remaining after := by
  cases rest : remaining cursor with
  | nil => simp [next, rest] at yielded
  | cons first tail =>
    have identity : first = value ∧ advance cursor = after := by
      simpa only [next, rest, Option.some.injEq, Prod.mk.injEq] using yielded
    obtain ⟨rfl, rfl⟩ := identity
    have nextSuffix : remaining (advance cursor) = tail := by
      change cursor.values.drop (cursor.position + 1) = tail
      rw [List.drop_add_one_eq_tail_drop]
      change (remaining cursor).tail = tail
      rw [rest]
      rfl
    rw [nextSuffix]

/-- A finite prefix of the cursor's actual next operations. Its fuel counts
mathematical output steps, not primitive work, allocation or cancellation polls. -/
def drain : Nat → Cursor Data W → List W
  | 0, _ => []
  | fuel + 1, cursor =>
    match next cursor with
    | none => []
    | some (value, after) => value :: drain fuel after

/-- Sufficient finite fuel returns precisely the unread suffix. The induction
consumes one occurrence and decreases the remaining length by one. -/
theorem drain_exact (fuel : Nat) (cursor : Cursor Data W)
    (enough : (remaining cursor).length ≤ fuel) :
    drain fuel cursor = remaining cursor := by
  induction fuel generalizing cursor with
  | zero =>
    have empty : remaining cursor = [] := List.length_eq_zero_iff.mp (Nat.eq_zero_of_le_zero enough)
    simp [drain, empty]
  | succ fuel inductionHypothesis =>
    cases rest : remaining cursor with
    | nil => simp [drain, next, rest]
    | cons value tail =>
      have nextSuffix : remaining (advance cursor) = tail := by
        have partition : remaining cursor = value :: remaining (advance cursor) :=
          next_preserves cursor (advance cursor) value (by simp [next, rest])
        exact (List.cons.inj (rest.symm.trans partition)).2.symm
      have tailBound : (remaining (advance cursor)).length ≤ fuel := by
        rw [nextSuffix]
        simp only [rest, List.length_cons] at enough
        omega
      have tailExact : drain fuel (advance cursor) = remaining (advance cursor) :=
        inductionHypothesis (advance cursor) tailBound
      simp only [drain, next, rest, tailExact, nextSuffix]

private def finish (cursor : Cursor Data W) : Cursor Data W :=
  { cursor with position := cursor.values.length }

/-- Reuse across the supplied finite parent sequence. Every child is paired with
the current parent, even when a different parent's carrier supplied the hit.
Only the single exhausted frame is retained for the next restart. -/
def reuseRows [DecidableEq Data] (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) :
    List (StructuralBindings.Binding Data) → Option (Cursor Data W) →
      Except (Failure Error) (List (StructuralBindings.Binding Data × W))
  | [], _ => .ok []
  | parent :: rest, retained =>
    match restart support inputs build parent retained with
    | .error error => .error error
    | .ok cursor =>
      match reuseRows support inputs build rest (some (finish cursor)) with
      | .error error => .error error
      | .ok rows => .ok ((drain cursor.values.length cursor).map (fun value => (parent, value)) ++ rows)

/-- The comparison traversal rebuilds for every parent and retains no family. -/
def directRows (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) :
    List (StructuralBindings.Binding Data) →
      Except (Failure Error) (List (StructuralBindings.Binding Data × W))
  | [] => .ok []
  | parent :: rest =>
    match compute support inputs build parent with
    | .error error => .error error
    | .ok values =>
      match directRows support inputs build rest with
      | .error error => .error error
      | .ok rows => .ok (values.map (fun value => (parent, value)) ++ rows)

/-- The reuse traversal has exactly direct rebuilding's ordered output or typed
failure. Each restart is exact; its fresh position permits full draining, and its
successful carrier establishes the retained invariant for the next parent. -/
theorem reuse_rows_exact [DecidableEq Data] (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) (parents : List (StructuralBindings.Binding Data))
    (retained : Option (Cursor Data W)) (valid : Valid support inputs build retained) :
    reuseRows support inputs build parents retained = directRows support inputs build parents := by
  induction parents generalizing retained with
  | nil => rfl
  | cons parent rest inductionHypothesis =>
    have restarted : restart support inputs build parent retained =
        (compute support inputs build parent).map (fresh parent) :=
      restart_exact support inputs build parent retained valid
    cases built : compute support inputs build parent with
    | error error => simp [reuseRows, directRows, restarted, built, Except.map]
    | ok values =>
      have retainedValid : Valid support inputs build (some (finish (fresh parent values))) := by
        simpa only [Valid, finish, fresh] using built
      have tailExact : reuseRows support inputs build rest (some (finish (fresh parent values))) =
          directRows support inputs build rest :=
        inductionHypothesis _ retainedValid
      have drained : drain values.length (fresh parent values) = values := by
        have complete : drain values.length (fresh parent values) =
            remaining (fresh parent values) :=
          drain_exact values.length (fresh parent values) (by simp [remaining, fresh])
        simpa [remaining, fresh] using complete
      have resetValues : (fresh parent values).values = values := rfl
      simp only [reuseRows, directRows, restarted, built, Except.map, tailExact, resetValues, drained]

/-- Successful direct builds instantiate the original dependent row family,
with parent order and child occurrences retained exactly. -/
theorem direct_rows_complete (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) (parents : List (StructuralBindings.Binding Data))
    (family : StructuralBindings.Binding Data → List W)
    (complete : ∀ parent ∈ parents, compute support inputs build parent = .ok (family parent)) :
    directRows support inputs build parents = .ok (rows parents family) := by
  induction parents with
  | nil => rfl
  | cons parent rest inductionHypothesis =>
    have first : compute support inputs build parent = .ok (family parent) :=
      complete parent (by simp)
    have tailComplete : ∀ parent ∈ rest, compute support inputs build parent = .ok (family parent) := by
      intro other present
      exact complete other (by simp [present])
    have tailExact : directRows support inputs build rest = .ok (rows rest family) :=
      inductionHypothesis tailComplete
    simp [directRows, first, tailExact, rows]

/-- Reuse emits the identical original clause sequence. In particular, prior
activation and aggregate equality still use the current parent and child.
Equality of theories preserves both original and arbitrary frozen M/J truth;
no proposal is interpreted as an established aggregate result. -/
theorem clauses_exact [DecidableEq Data] (support : Support) (inputs : List Nat)
    (build : Build Support Data W Error) (parents : List (StructuralBindings.Binding Data))
    (family : StructuralBindings.Binding Data → List W)
    (complete : ∀ parent ∈ parents, compute support inputs build parent = .ok (family parent))
    (previous : StructuralBindings.Binding Data → Formula A)
    (equality head : StructuralBindings.Binding Data → W → Formula A) :
    (reuseRows support inputs build parents none).map (fun pairs => pairs.map
      (fun pair => clause (previous pair.1) (equality pair.1 pair.2) (head pair.1 pair.2))) =
      .ok (clauses parents family previous equality head) := by
  rw [reuse_rows_exact support inputs build parents none True.intro,
    direct_rows_complete support inputs build parents family complete]
  rfl

-- Rewinding restores occurrences that an exhausted or partly consumed cursor
-- would omit; a changed declared input rebuilds, and absent input is an error.
example : remaining (⟨fun _ => some (1 : Nat), [1, 1], 1⟩ : Cursor Nat Nat) = [1] := rfl
example : (restart () [0] (fun _ values => (Except.ok values : Except Unit (List Nat)))
    (fun _ => some 2) (some ⟨fun _ => some 1, [1], 1⟩)).map Cursor.values = .ok [2] := rfl
example : (restart () [0] (fun _ values => (Except.ok values : Except Unit (List Nat)))
    (fun _ => some 1) (some ⟨fun _ => some 1, [1], 1⟩)).map remaining = .ok [1] := rfl
example : restart () [0] (fun _ values => (Except.ok values : Except Unit (List Nat)))
    (fun _ => none) none = .error .missingInput := rfl
example : restart () [0] (fun _ _ => (Except.error () : Except Unit (List Nat)))
    (fun _ => some (1 : Nat)) none = .error (.build ()) := rfl

end Reuse

namespace SupportReuse

universe p k e
variable {Parent : Type p} {Key : Type k} {Error : Type e}

/-- The observation of a completed possible-head continuation. Heads form a set;
`defined` records whether any complete substitution has defined arithmetic, and
`zero` retains the first unexcluded zero-divisor diagnostic, if one occurred.
Immediate fatal failures and unfinished traversals supply no observation.
Completion means finishing this continuation, not finalizing the enclosing
source family's arithmetic: a zero without a defined witness remains evidence. -/
structure Observation (Atom : Type w) (Error : Type e) where
  heads : Atoms Atom
  defined : Bool
  zero : Option Error

/-- Combine completed observations in source traversal order. Repeated heads
and defined witnesses are idempotent; an earlier diagnostic has priority. -/
def merge (first later : Observation A Error) : Observation A Error :=
  ⟨Union first.heads later.heads, first.defined || later.defined,
    match first.zero with
    | some error => some error
    | none => later.zero⟩

/-- No completed parent contributes no head, defined witness or diagnostic. -/
def empty : Observation A Error := ⟨Empty, false, none⟩

/-- Repeating a completed observation cannot add support or alter its evidence,
even in front of later observations. The first zero is retained, not reordered. -/
theorem merge_repeated (first later : Observation A Error) :
    merge first (merge first later) = merge first later := by
  have repeatedHeads : Union first.heads (Union first.heads later.heads) =
      Union first.heads later.heads := by
    apply atoms_ext
    intro atom
    simp only [Union, or_self_left]
  cases first with
  | mk heads defined zero =>
    cases defined <;> cases zero <;> simpa only [merge, Bool.false_or, Bool.true_or,
      Observation.mk.injEq, and_true] using repeatedHeads

/-- The complete ordered traversal combines every parent's observation. -/
def full (observation : Parent → Observation A Error) : List Parent → Observation A Error
  | [] => empty
  | parent :: rest => merge (observation parent) (full observation rest)

/-- Keep the first completed parent of each run of equal projections. The
retained parent supplies that run's observation; no later parent replaces its
first diagnostic. Each recursive call consumes one remaining parent. -/
private def runFrom [DecidableEq Key] (projection : Parent → Key)
    (observation : Parent → Observation A Error) (previous : Parent) :
    List Parent → Observation A Error
  | [] => observation previous
  | parent :: rest =>
    if projection previous = projection parent then runFrom projection observation previous rest
    else merge (observation previous) (runFrom projection observation parent rest)

/-- A finite support-only traversal that omits consecutive repeated projections.
It observes completed results, not original formula witnesses or error prefixes. -/
def reuse [DecidableEq Key] (projection : Parent → Key)
    (observation : Parent → Observation A Error) : List Parent → Observation A Error
  | [] => empty
  | parent :: rest => runFrom projection observation parent rest

/-- Skipping repeated projections preserves exactly the union of possible heads,
the defined-witness flag and the first zero-divisor diagnostic. Every supplied
parent must have completed its entire continuation, and the projection must
determine this full observation on completed parents. Equality of just the head
image would not suffice for diagnostics. The observations belong to one fixed
producer, support and source-family context; the theorem does not construct it.

The induction keeps the first parent of an equal-projection run. A hit removes
one idempotent repeated observation; a miss combines the preceding observation
with the induction result for the next run. This law neither omits original
formula occurrences nor establishes a compiler's projection/completion premises. -/
theorem reuse_exact [DecidableEq Key] (projection : Parent → Key)
    (observation : Parent → Observation A Error) (Complete : Parent → Prop)
    (sufficient : ∀ left right, Complete left → Complete right →
      projection left = projection right → observation left = observation right)
    (parents : List Parent) (completed : ∀ parent ∈ parents, Complete parent) :
    reuse projection observation parents = full observation parents := by
  have fromExact : ∀ (rest : List Parent) (previous : Parent), Complete previous →
      (∀ parent ∈ rest, Complete parent) →
      runFrom projection observation previous rest =
        merge (observation previous) (full observation rest) := by
    intro rest
    induction rest with
    | nil =>
      intro previous _ _
      cases observed : observation previous with
      | mk heads defined zero =>
        have emptyHeads : heads = Union heads Empty := by
          apply atoms_ext
          intro atom
          simp only [Union, Empty, or_false]
        cases defined <;> cases zero <;> simpa only [runFrom, full, merge, empty,
          observed, Bool.or_false, Observation.mk.injEq, and_true] using emptyHeads
    | cons parent rest inductionHypothesis =>
      intro previous previousComplete allComplete
      have parentComplete : Complete parent := allComplete parent (by simp)
      have restComplete : ∀ other ∈ rest, Complete other := by
        intro other member
        exact allComplete other (by simp [member])
      by_cases same : projection previous = projection parent
      · have equalObservation : observation previous = observation parent :=
          sufficient previous parent previousComplete parentComplete same
        have continued : runFrom projection observation previous rest =
            merge (observation previous) (full observation rest) :=
          inductionHypothesis previous previousComplete restComplete
        rw [runFrom, if_pos same, continued, full, ← equalObservation, merge_repeated]
      · have continued : runFrom projection observation parent rest =
            merge (observation parent) (full observation rest) :=
          inductionHypothesis parent parentComplete restComplete
        rw [runFrom, if_neg same, continued, full]
  cases parents with
  | nil => rfl
  | cons parent rest =>
    have parentComplete : Complete parent := completed parent (by simp)
    have restComplete : ∀ other ∈ rest, Complete other := by
      intro other member
      exact completed other (by simp [member])
    exact fromExact rest parent parentComplete restComplete

-- Equal head images alone do not permit skipping different diagnostic evidence.
example :
    let observation : Nat → Observation Unit Nat := fun index =>
      ⟨Empty, false, if index = 0 then none else some index⟩
    (reuse (fun _ => ()) observation [0, 1]).zero ≠
      (full observation [0, 1]).zero := by decide

end SupportReuse

end Zetesis.AggregateDependencies
