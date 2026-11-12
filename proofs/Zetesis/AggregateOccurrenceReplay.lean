import Zetesis.AggregateDependencies
import Zetesis.AxisFolds

/-!
# Ordered observations of one completed aggregate-family walk

A producer and a fresh consumer may have different full binding frames. Their
steps must nevertheless expose the same ordered tuple/required observations
under the same source and completed support. A relation between the states,
preserved at each observable step, supplies this correspondence. Equality of
tuple sets or a permutation does not supply it.

The transition below describes a walk's observations; it is not a new runtime
search primitive. `simulate` preserves a completed observation trace or its
semantic failure. It does not preserve a resource-interrupted execution prefix.
`replay_exact` then relates retained coordinates to the actual fresh rows. It
keeps every duplicate occurrence and pairs it with that row, not a previous
parent's whole binding. Only after this alignment may the named Reduction fold
consume the observations. No commutativity or permutation law is used.

The compiler's complete input footprint, local-plan compatibility, fresh join
step relation, coordinate-owner authentication, physical storage and charges
remain Rust correspondence obligations. These statements do not establish them.
-/
namespace Zetesis.AggregateOccurrenceReplay

universe s t o e c r v k d p

/-- An observation includes its original element and requiredness. Distinct
occurrences may have equal tuples and must still remain in the ordered trace. -/
structure Occurrence (Tuple : Type o) where
  element : Nat
  tuple : Tuple
  required : Bool
  deriving DecidableEq

/-- One observable step of a producer or a fresh consumer's existing walk. -/
inductive Step (State : Type s) (Observed : Type o) (Error : Type e) where
  | done
  | failure (error : Error)
  | emit (observed : Observed) (after : State)

/-- Compatible observable steps agree on completion, semantic failure, or the
next occurrence and the relation needed for the remainder of the walk. -/
def Matches {Left : Type s} {Right : Type t} {Observed : Type o} {Error : Type e}
    (related : Left → Right → Prop) : Step Left Observed Error → Step Right Observed Error → Prop
  | .done, .done => True
  | .failure first, .failure second => first = second
  | .emit first left, .emit second right => first = second ∧ related left right
  | _, _ => False

/-- A finite completed trace; an earlier emitted prefix does not turn a later
semantic failure into successful completion. -/
inductive Trace {State : Type s} {Observed : Type o} {Error : Type e}
    (step : State → Step State Observed Error) : State → Except Error (List Observed) → Prop where
  | done {state} (observed : step state = .done) : Trace step state (.ok [])
  | failure {state error} (observed : step state = .failure error) :
      Trace step state (.error error)
  | emit {state after occurrence result} (observed : step state = .emit occurrence after)
      (tail : Trace step after result) :
      Trace step state (result.map (fun rest => occurrence :: rest))

/-- A stepwise simulation preserves the exact ordered trace, not just its set.

The induction follows the producer's completed trace. Completion and failure
must match immediately. At an emission, the same observation is prepended to
the induction result for the related successor states. -/
theorem simulate {Left : Type s} {Right : Type t} {Observed : Type o} {Error : Type e}
    (leftStep : Left → Step Left Observed Error)
    (rightStep : Right → Step Right Observed Error) (related : Left → Right → Prop)
    (steps : ∀ left right, related left right → Matches related (leftStep left) (rightStep right))
    {left : Left} {right : Right} {result : Except Error (List Observed)}
    (trace : Trace leftStep left result) (initial : related left right) :
    Trace rightStep right result := by
  induction trace generalizing right with
  | @done state observed =>
    have compatible := steps state right initial
    cases current : rightStep right with
    | done => exact Trace.done current
    | failure error => simp [Matches, observed, current] at compatible
    | emit occurrence after => simp [Matches, observed, current] at compatible
  | @failure state error observed =>
    have compatible := steps state right initial
    cases current : rightStep right with
    | done => simp [Matches, observed, current] at compatible
    | failure other =>
      have same : error = other := by simpa [Matches, observed, current] using compatible
      cases same
      exact Trace.failure current
    | emit occurrence after => simp [Matches, observed, current] at compatible
  | @emit state after occurrence result observed tail inductionHypothesis =>
    have compatible := steps state right initial
    cases current : rightStep right with
    | done => simp [Matches, observed, current] at compatible
    | failure error => simp [Matches, observed, current] at compatible
    | emit other successor =>
      have paired : occurrence = other ∧ related after successor := by
        simpa [Matches, observed, current] using compatible
      have remainder : Trace rightStep successor result := inductionHypothesis paired.2
      have completed := Trace.emit current remainder
      simpa only [paired.1] using completed

/-- Abstract observations of the existing join state. `control` denotes its
ordered row/cursor/trail position; it is not just a set of visited tuples.
`owner` denotes the full required source/support/workspace identity. -/
structure JoinState (Owner : Type k) (Plan : Type p) (Control : Type c) (Data : Type d) where
  owner : Owner
  plan : Plan
  prefixLength : Nat
  variables : Nat
  control : Control
  binding : StructuralBindings.Binding Data

/-- Same authenticated owner and immutable LocalPlan, same scope geometry and
control position, and equal typed values or absence at every semantic read.
Irrelevant full-frame slots are deliberately unconstrained. -/
structure JoinAlignment {Owner : Type k} {Plan : Type p} {Control : Type c} {Data : Type d}
    (reads : List Nat) (left right : JoinState Owner Plan Control Data) : Prop where
  owner : left.owner = right.owner
  plan : left.plan = right.plan
  prefixLength : left.prefixLength = right.prefixLength
  variables : left.variables = right.variables
  control : left.control = right.control
  values : ∀ slot ∈ reads, left.binding slot = right.binding slot

/-- The explicit state relation preserves the checked input read, including
unavailable input. It does not infer source read completeness from a plan key. -/
theorem aligned_reads {Owner : Type k} {Plan : Type p} {Control : Type c} {Data : Type d}
    (reads : List Nat) (left right : JoinState Owner Plan Control Data)
    (aligned : JoinAlignment reads left right) :
    BindingScopes.readAll left.binding reads = BindingScopes.readAll right.binding reads :=
  BindingScopes.readAll_agrees left.binding right.binding reads aligned.values

/-- An explicit place for the concrete join correspondence obligation. If each
actual producer/consumer step preserves `JoinAlignment`, the ordered trace is
equal. The `stepCorrespondence` premise is not proved here: source footprint,
scope initialization, row selection, generators and backtracking must discharge
it in the implementation review/tests or a later concrete formal model. -/
theorem join_trace_exact {Owner : Type k} {Plan : Type p} {Control : Type c} {Data : Type d}
    {Observed : Type o} {Error : Type e} (reads : List Nat)
    (producer consumer : JoinState Owner Plan Control Data →
      Step (JoinState Owner Plan Control Data) Observed Error)
    (stepCorrespondence : ∀ left right, JoinAlignment reads left right →
      Matches (JoinAlignment reads) (producer left) (consumer right))
    (left right : JoinState Owner Plan Control Data) (result : Except Error (List Observed))
    (trace : Trace producer left result) (aligned : JoinAlignment reads left right) :
    Trace consumer right result :=
  simulate producer consumer (JoinAlignment reads) stepCorrespondence trace aligned

/-- Existing scoped reads establish complete ordered build equality only when
the builder really depends on the fixed support and complete declared inputs.
The Rust step-simulation obligation is not inferred from equal bound bits. -/
theorem projected_occurrences {Support : Type p} {Data : Type d} {Observed : Type o}
    {Error : Type e} (support : Support) (inputs : List Nat)
    (build : AggregateDependencies.Reuse.Build Support Data Observed Error)
    (before after : StructuralBindings.Binding Data)
    (agree : ∀ slot ∈ inputs, before slot = after slot) :
    AggregateDependencies.Reuse.compute support inputs build before =
      AggregateDependencies.Reuse.compute support inputs build after :=
  AggregateDependencies.Reuse.compute_agrees support inputs build before after agree

/-- The reference alignment checker consumes both ordered lists completely.
Missing coordinates, a changed observation, or a changed length decline.
The returned row is always the actual fresh row supplied by the consumer. -/
def replay {Coordinate : Type c} {Row : Type r} {Observed : Type o} [DecidableEq Observed]
    (decode : Coordinate → Option Observed) (describe : Row → Observed) :
    List Coordinate → List Row → Option (List (Observed × Row))
  | [], [] => some []
  | coordinate :: coordinates, row :: rows =>
    match decode coordinate with
    | none => none
    | some stored =>
      if stored = describe row then
        (replay decode describe coordinates rows).map (fun rest => (stored, row) :: rest)
      else none
  | _, _ => none

/-- Equal-length lists related at each ordered position. This local relation
supplies the nil/cons alignment used below without an external library. -/
inductive Positional {Left : Type c} {Right : Type r} (related : Left → Right → Prop) :
    List Left → List Right → Prop where
  | nil : Positional related [] []
  | cons {left right rest remaining} (head : related left right)
      (tail : Positional related rest remaining) :
      Positional related (left :: rest) (right :: remaining)

/-- Exact correspondence is positional, including duplicates and annotations.
It supplies both exhaustion and the right tuple at every fresh consumer row. -/
theorem replay_exact {Coordinate : Type c} {Row : Type r} {Observed : Type o}
    [DecidableEq Observed] (decode : Coordinate → Option Observed) (describe : Row → Observed)
    (coordinates : List Coordinate) (rows : List Row)
    (aligned : Positional (fun coordinate row => decode coordinate = some (describe row))
      coordinates rows) :
    replay decode describe coordinates rows = some (rows.map (fun row => (describe row, row))) := by
  induction aligned with
  | nil => rfl
  | cons same rest inductionHypothesis =>
    simp only [replay, same, ↓reduceIte, inductionHypothesis, Option.map_some, List.map_cons]

/-- A decoder's coordinate space is valid only in its exact authenticated
owner. The abstract equality includes every owner dimension the caller needs. -/
def ownedReplay {Owner : Type k} {Coordinate : Type c} {Row : Type r} {Observed : Type o}
    [DecidableEq Owner] [DecidableEq Observed] (expected retained : Owner)
    (decode : Coordinate → Option Observed) (describe : Row → Observed)
    (coordinates : List Coordinate) (rows : List Row) : Option (List (Observed × Row)) :=
  if retained = expected then replay decode describe coordinates rows else none

theorem foreign_declines {Owner : Type k} {Coordinate : Type c} {Row : Type r} {Observed : Type o}
    [DecidableEq Owner] [DecidableEq Observed] (expected retained : Owner)
    (decode : Coordinate → Option Observed) (describe : Row → Observed)
    (coordinates : List Coordinate) (rows : List Row) (foreign : retained ≠ expected) :
    ownedReplay expected retained decode describe coordinates rows = none := by
  simp only [ownedReplay, if_neg foreign]

/-- Ordered alignment is strong enough for any pure consumer, including one
whose result records the first source error or exact formula/provenance order.
No assumption of commutativity, idempotence or equal tuple sets is required. -/
theorem consumer_exact {Coordinate : Type c} {Row : Type r} {Observed : Type o}
    {Result : Type v} [DecidableEq Observed]
    (decode : Coordinate → Option Observed) (describe : Row → Observed)
    (coordinates : List Coordinate) (rows : List Row)
    (aligned : Positional (fun coordinate row => decode coordinate = some (describe row))
      coordinates rows) (consume : List (Observed × Row) → Result) :
    (replay decode describe coordinates rows).map consume =
      some (consume (rows.map (fun row => (describe row, row)))) := by
  rw [replay_exact decode describe coordinates rows aligned]
  rfl

/-- The named Reduction receives the identical ordered annotated rows. This
uses exact replay, never the weaker reduction law for permuting members. -/
theorem reduction_exact {Coordinate : Type c} {Row : Type r} {Observed : Type o}
    {Value : Type v} [DecidableEq Observed]
    (decode : Coordinate → Option Observed) (describe : Row → Observed)
    (coordinates : List Coordinate) (rows : List Row)
    (aligned : Positional (fun coordinate row => decode coordinate = some (describe row))
      coordinates rows) (reduction : AxisFolds.Reduction Value) (value : Observed × Row → Value) :
    (replay decode describe coordinates rows).map (reduction.fold value) =
      some (reduction.fold value (rows.map (fun row => (describe row, row)))) :=
  consumer_exact decode describe coordinates rows aligned (reduction.fold value)

-- Equal tuple sets with a reversed order do not establish coordinate alignment.
example : replay (fun value : Nat => some value) id [0, 1] [1, 0] = none := by decide
-- Neither equal tuple values nor repeats permit discarding a required annotation.
example :
    replay (fun _ : Nat => some (Occurrence.mk 0 7 false)) id [0]
      [Occurrence.mk 0 7 true] = none := by decide
example : replay (fun value : Nat => some value) id [1, 1] [1, 1] =
    some [(1, 1), (1, 1)] := by decide

end Zetesis.AggregateOccurrenceReplay
