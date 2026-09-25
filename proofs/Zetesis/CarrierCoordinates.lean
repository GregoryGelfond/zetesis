import Zetesis.ModelSelections

/-!
# Atoms read through frozen vocabulary coordinates

A coordinate names a signature and an ordered list of domain positions. The
two position types range over admitted positions in one fixed vocabulary;
decoding is total on those types. A well-formed carrier tuple additionally has
the signature's arity. The identity and order laws hold for every list, so also
for that arity-restricted subset, without enumerating a Cartesian carrier.

Unique signatures and domain values make decoding injective. Numeric position
order agrees with atom storage order only when both component decoders preserve
and reflect their supplied orders. These assumptions concern the complete
signed signature and typed value, not a spelling, hash or ASP term order.

Sparse selection needs only its finite list of coordinates. No finite whole
carrier, total cardinality or representable ordinal is assumed. `GatePositions`
separately relates valid digits to mixed-radix positions; `Search` and
`RegionBounds` separately justify coverage and narrowing. These laws do not
establish those semantic obligations from coordinate identity alone.

Rust must validate positions, arity and program applicability, establish the
component uniqueness/order premises, and retain the frozen vocabulary. Shared
vocabulary does not establish a shared atom-row scope. Allocation, machine
arithmetic, immutable publication and executable refinement remain unproved here.
-/

namespace Zetesis.CarrierCoordinates

universe u v w x

variable {SignatureSlot : Type u} {DomainSlot : Type v}
variable {Signature : Type w} {Value : Type x}

/-- Read the complete signature and each ordered argument from one vocabulary.
This mathematical pair does not require a materialized atom row or dense rank. -/
def decode (signature : SignatureSlot → Signature) (domain : DomainSlot → Value)
    (coordinate : SignatureSlot × List DomainSlot) : Signature × List Value :=
  (signature coordinate.1, coordinate.2.map domain)

/-- Unique signatures and domain values give each decoded atom one coordinate.
Project equal atoms to their signatures and argument lists, then use injectivity
at each position. No carrier cardinality or enumeration premise is needed. -/
theorem decode_injective
    (signature : SignatureSlot → Signature) (domain : DomainSlot → Value)
    (unique_signatures : Function.Injective signature)
    (unique_values : Function.Injective domain) :
    Function.Injective (decode signature domain) := by
  intro left right same_atom
  have same_signature : signature left.1 = signature right.1 :=
    congrArg Prod.fst same_atom
  have same_arguments : left.2.map domain = right.2.map domain :=
    congrArg Prod.snd same_atom
  have same_positions : left.2 = right.2 :=
    (List.map_inj_right (fun _ _ equal => unique_values equal)).mp same_arguments
  exact Prod.ext (unique_signatures same_signature) same_positions

/-- An injective domain decoder transports lexicographic tuple order exactly
when it transports the supplied element relation exactly. Induction follows
the first differing element; injectivity preserves equal prefixes. -/
theorem tuple_before_iff (domain : DomainSlot → Value)
    (unique_values : Function.Injective domain)
    (position_before : DomainSlot → DomainSlot → Prop)
    (value_before : Value → Value → Prop)
    (ordered_values : ∀ left right,
      value_before (domain left) (domain right) ↔ position_before left right)
    (left right : List DomainSlot) :
    List.Lex value_before (left.map domain) (right.map domain) ↔
      List.Lex position_before left right := by
  induction left generalizing right with
  | nil =>
    cases right <;>
      simp only [List.map_nil, List.map_cons, List.not_lex_nil, List.nil_lex_cons']
  | cons first rest tail =>
    cases right with
    | nil => simp only [List.map_nil, List.not_lex_nil]
    | cons next remaining =>
      have same_head : domain first = domain next ↔ first = next :=
        ⟨fun equal => unique_values equal, congrArg domain⟩
      simp only [List.map_cons, List.cons_lex_cons_iff, ordered_values,
        same_head, tail]

/-- Compare the signature first and compare argument tuples only on an equal
signature. The relations are parameters; this does not select an ASP order. -/
def Before (signature_before : SignatureSlot → SignatureSlot → Prop)
    (value_before : DomainSlot → DomainSlot → Prop)
    (left right : SignatureSlot × List DomainSlot) : Prop :=
  signature_before left.1 right.1 ∨
    left.1 = right.1 ∧ List.Lex value_before left.2 right.2

/-- Coordinate order is exactly decoded atom order under unique, ordered
signatures and a unique, ordered domain. The same vocabulary supplies both
decodings. Signature order is transported first; equal signatures then use
`tuple_before_iff`. Merely allocating increasing IDs does not supply the premises.
-/
theorem decode_before_iff
    (signature : SignatureSlot → Signature) (domain : DomainSlot → Value)
    (unique_signatures : Function.Injective signature)
    (unique_values : Function.Injective domain)
    (signature_position_before : SignatureSlot → SignatureSlot → Prop)
    (domain_position_before : DomainSlot → DomainSlot → Prop)
    (signature_before : Signature → Signature → Prop)
    (value_before : Value → Value → Prop)
    (ordered_signatures : ∀ left right,
      signature_before (signature left) (signature right) ↔
        signature_position_before left right)
    (ordered_values : ∀ left right,
      value_before (domain left) (domain right) ↔ domain_position_before left right)
    (left right : SignatureSlot × List DomainSlot) :
    Before signature_before value_before
        (decode signature domain left) (decode signature domain right) ↔
      Before signature_position_before domain_position_before left right := by
  have same_signature : signature left.1 = signature right.1 ↔ left.1 = right.1 :=
    ⟨fun equal => unique_signatures equal, congrArg signature⟩
  have same_tuple_order :
      List.Lex value_before (left.2.map domain) (right.2.map domain) ↔
        List.Lex domain_position_before left.2 right.2 :=
    tuple_before_iff domain unique_values domain_position_before value_before
      ordered_values left.2 right.2
  simp only [Before, decode, ordered_signatures, same_signature, same_tuple_order]

/-- A decoded atom belongs to a sparse interpretation exactly when its unique
coordinate was selected. Only the selected list is finite; the coordinate types
may be infinite. Thus the law has no full-carrier cardinality or ordinal premise.
The forward direction uses decoding injectivity; the reverse retains the given
selected coordinate as the membership witness. -/
theorem sparse_membership_iff
    (signature : SignatureSlot → Signature) (domain : DomainSlot → Value)
    (unique_signatures : Function.Injective signature)
    (unique_values : Function.Injective domain)
    (selected : List (SignatureSlot × List DomainSlot))
    (tested : SignatureSlot × List DomainSlot) :
    ModelSelections.denotes (fun coordinate => some (decode signature domain coordinate))
        selected (decode signature domain tested) ↔ tested ∈ selected := by
  constructor
  · rintro ⟨coordinate, present, same_atom⟩
    have same_coordinate : coordinate = tested :=
      decode_injective signature domain unique_signatures unique_values
        (Option.some.inj same_atom)
    exact same_coordinate ▸ present
  · intro present
    exact ⟨tested, present, rfl⟩

/-- Two atom-row scopes can assign raw position zero to different unary atoms
while sharing exactly the same signature and value decoders. Their selections
then differ. Shared vocabulary justifies value identity, not local atom-row ID
identity: the left interpretation contains the false-valued row and the right
does not. -/
theorem shared_vocabulary_does_not_identify_atoms :
    ModelSelections.denotes
        (fun _ : Nat => some (decode (id : Unit → Unit) (id : Bool → Bool)
          ((), [false]))) [0] ≠
      ModelSelections.denotes
        (fun _ : Nat => some (decode (id : Unit → Unit) (id : Bool → Bool)
          ((), [true]))) [0] := by
  intro same_interpretation
  have selected : ModelSelections.denotes
      (fun _ : Nat => some (decode (id : Unit → Unit) (id : Bool → Bool)
        ((), [false]))) [0] ((), [false]) := by
    exact ⟨0, by simp, rfl⟩
  have absent : ¬ ModelSelections.denotes
      (fun _ : Nat => some (decode (id : Unit → Unit) (id : Bool → Bool)
        ((), [true]))) [0] ((), [false]) := by
    rintro ⟨_position, _present, same_atom⟩
    simp [decode] at same_atom
  exact absent (same_interpretation ▸ selected)

end Zetesis.CarrierCoordinates
