import Std.Tactic

/-!
# Source contributions in shared storage

Different atoms may store their producers and source locations in one arena.
A finite chain determines one ordered sequence. Changing entries outside that
chain leaves the sequence unchanged. For producer records, selecting one atom
from an append log preserves its occurrence order, including duplicates.

These laws describe the observations a shared metadata owner must preserve.
They do not prove Rust link insertion, atom ownership, sorted origin insertion,
iterator counts, allocation or resource accounting. The implementation must
establish that its private links form the finite chains described here, that
producer chains equal the corresponding log subsequences, and that each origin
chain is sorted and duplicate-free. Final support folding and owned evidence
materialization then consume those same sequences.
-/

namespace Zetesis.SourceContributions

universe u v
variable {Value : Type u} {Atom : Type v}

/-- One stored value and the optional position of its successor. -/
structure Link (Value : Type u) where
  value : Value
  next : Option Nat

/-- A finite traversal records both positions and values. Its inductive witness
excludes a traversal that cycles forever; it does not validate a Rust counter. -/
inductive Chain (arena : Nat → Option (Link Value)) :
    Option Nat → List (Nat × Value) → Prop where
  | empty : Chain arena none []
  | step {position : Nat} {entry : Link Value} {rest : List (Nat × Value)}
      (present : arena position = some entry)
      (tail : Chain arena entry.next rest) :
      Chain arena (some position) ((position, entry.value) :: rest)

/-- A fixed arena and starting position determine at most one finite sequence.
At each step, the common lookup fixes both the value and successor; induction
then identifies the remaining sequence. -/
theorem chain_unique (arena : Nat → Option (Link Value))
    {start : Option Nat} {left right : List (Nat × Value)}
    (first : Chain arena start left) (second : Chain arena start right) :
    left = right := by
  induction first generalizing right with
  | empty =>
    cases second
    rfl
  | @step position entry rest present tail induction =>
    cases second with
    | @step _ other otherRest otherPresent otherTail =>
      have same_entry : entry = other := Option.some.inj (present.symm.trans otherPresent)
      cases same_entry
      have same_tail : rest = otherRest := induction otherTail
      exact congrArg (List.cons (position, entry.value)) same_tail

/-- Updating storage outside a chain preserves every position and value it
observes. Retain the first lookup, then apply the same argument to the tail. -/
theorem chain_preserved (before after : Nat → Option (Link Value))
    {start : Option Nat} {rows : List (Nat × Value)}
    (chain : Chain before start rows)
    (unchanged : ∀ position value, (position, value) ∈ rows →
      after position = before position) :
    Chain after start rows := by
  induction chain with
  | empty => exact .empty
  | @step position entry rest present tail induction =>
    have first : after position = some entry :=
      (unchanged position entry.value (by simp)).trans present
    have remaining : Chain after entry.next rest := by
      apply induction
      intro other value member
      exact unchanged other value (by simp [member])
    exact .step first remaining

/-- Read one atom's contributions in the order in which they were recorded. -/
def contributions [DecidableEq Atom] (atom : Atom)
    (records : List (Atom × Value)) : List Value :=
  (records.filter (fun record => decide (record.1 = atom))).map Prod.snd

/-- Selecting one atom commutes with concatenating record batches. Filtering
and value projection both preserve the order of the two batches. -/
theorem contributions_append [DecidableEq Atom] (atom : Atom)
    (before added : List (Atom × Value)) :
    contributions atom (before ++ added) =
      contributions atom before ++ contributions atom added := by
  simp [contributions, List.filter_append, List.map_append]

/-- Recording a producer for an atom appends precisely that producer to its
sequence, even when the same producer already occurs. -/
theorem record_same [DecidableEq Atom] (atom : Atom)
    (records : List (Atom × Value)) (value : Value) :
    contributions atom (records ++ [(atom, value)]) =
      contributions atom records ++ [value] := by
  rw [contributions_append]
  simp [contributions]

/-- Recording a contribution for another atom leaves this atom's sequence
unchanged. The two atoms may still share the same physical arena. -/
theorem record_other [DecidableEq Atom] (atom other : Atom)
    (records : List (Atom × Value)) (value : Value) (distinct : other ≠ atom) :
    contributions atom (records ++ [(other, value)]) = contributions atom records := by
  rw [contributions_append]
  simp [contributions, distinct]

end Zetesis.SourceContributions
