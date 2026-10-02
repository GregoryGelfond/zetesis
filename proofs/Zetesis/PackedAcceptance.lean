import Zetesis.PackedClosure

/-!
# Completed packed acceptance for normalized rules

The packed closure is followed by two finite decisions: no selected constraint
has a true positive body, and closure and frozen seed agree on the entire supplied
gate carrier. Carrier-only agreement requires seed admission into that carrier;
it cannot discover an outside seed atom. That premise is explicit below.

These executable Lean operations refine the existing normalized `Accept`
predicate. They are not extracted Rust and do not prove source admission,
canonical graph construction, resource outcomes, ownership or enumeration.
-/

namespace Zetesis.Refinement.PackedAcceptance

open FiniteClosure PackedClosure

/-- Select enabled constraints, then stop at the first true positive body.
    A false result is constraint violation, rather than incomplete checking. -/
def constraints {size : Nat} (rules : List (Rule (Fin size)))
    (frozen words : List (BitVec 32)) : Bool :=
  (rules.filter (fun rule => enabled rule frozen && rule.head.isNone)).all
    (fun rule => !allPresent words rule.positive)

/-- A packed body test reads exactly the represented semantic interpretation. -/
theorem body_exact {size : Nat} (rule : Rule (Fin size)) (words : List (BitVec 32)) :
    allPresent words rule.positive = true ↔ Semantics.Body rule.denote (denotes words) := by
  simp [allPresent, List.all_eq_true, Semantics.Body, Rule.denote, denotes]

/-- Frozen constraint selection and its final packed tests are exactly the
    existing normalized constraint condition. No list representation of the
    resulting closure needs to be supplied by the caller. -/
theorem constraints_exact {size : Nat} (rules : List (Rule (Fin size)))
    (seed : List (Fin size)) (words : List (BitVec 32)) :
    constraints rules (Packing.encode seed) words = true ↔
      Semantics.ConstraintsOK (program rules) (atoms seed) (denotes words) := by
  have frozen := Packing.encode_represents seed
  constructor
  · intro checked source member no_head _ gate positive
    obtain ⟨rule, in_rules, same⟩ := List.mem_map.mp member
    subst source
    have head : rule.head = none := no_head
    have gate_true := (gates_exact rule seed).mpr gate
    have selected : rule ∈ rules.filter
        (fun item => enabled item (Packing.encode seed) && item.head.isNone) := by
      simp [in_rules, head, enabled_refines rule _ seed frozen, gate_true]
    have rejected := List.all_eq_true.mp checked rule selected
    have body_true := (body_exact rule words).mpr positive
    simp [body_true] at rejected
  · intro valid
    apply List.all_eq_true.mpr
    intro rule member
    obtain ⟨in_rules, selected⟩ := List.mem_filter.mp member
    have enabled_head : enabled rule (Packing.encode seed) = true ∧ rule.head = none := by
      simpa only [Bool.and_eq_true, Option.isNone_iff_eq_none] using selected
    have gate := (gates_exact rule seed).mp
      ((enabled_refines rule _ seed frozen) ▸ enabled_head.1)
    have absent : ¬ allPresent words rule.positive = true := by
      intro present
      exact valid rule.denote (List.mem_map.mpr ⟨rule, in_rules, rfl⟩) enabled_head.2
        True.intro gate ((body_exact rule words).mp present)
    cases truth : allPresent words rule.positive <;> simp_all

/-- Compare both packed truth values only at supplied carrier coordinates. -/
def agrees {size : Nat} (carrier : List (Fin size))
    (frozen words : List (BitVec 32)) : Bool :=
  carrier.all (fun atom => Packing.read frozen atom.val == Packing.read words atom.val)

/-- Carrier-only equality is full projection equality when the seed has already
    been admitted into the carrier. Without that premise, outside true seed
    atoms are unobserved by this loop. -/
theorem agrees_exact {size : Nat} (carrier seed : List (Fin size))
    (words : List (BitVec 32)) (admitted : ∀ atom ∈ seed, atom ∈ carrier) :
    agrees carrier (Packing.encode seed) words = true ↔
      Inter (denotes words) (atoms carrier) = atoms seed := by
  have read_seed := Packing.read_member _ seed (Packing.encode_represents seed)
  have pointwise : agrees carrier (Packing.encode seed) words = true ↔
      ∀ atom ∈ carrier, denotes words atom ↔ atom ∈ seed := by
    simp only [agrees, List.all_eq_true, beq_iff_eq, read_seed, denotes]
    constructor
    · intro same atom member
      rw [← same atom member]
      simp
    · intro same atom member
      have at_atom := same atom member
      cases truth : Packing.read words atom.val <;> simp_all
  rw [pointwise]
  constructor
  · intro same
    apply atoms_ext
    intro atom
    constructor
    · rintro ⟨present, inside⟩
      exact (same atom inside).mp present
    · intro present
      exact ⟨(same atom (admitted atom present)).mpr present, admitted atom present⟩
  · intro same atom inside
    have projected : (denotes words atom ∧ atom ∈ carrier) ↔ atom ∈ seed := by
      change Inter (denotes words) (atoms carrier) atom ↔ atoms seed atom
      rw [same]
    simpa only [inside, and_true] using projected

/-- Use the proved scan bound, then test the completed closure. The finite
    mathematical operation is total; a machine resource stop is not modeled. -/
def check {size : Nat} (rules : List (Rule (Fin size))) (carrier seed : List (Fin size)) : Bool :=
  let result := (close rules seed ((heads rules).length + 1)).get (by
    obtain ⟨words, completed⟩ := close_completes rules seed
    simp [completed])
  constraints rules (Packing.encode seed) result && agrees carrier (Packing.encode seed) result

/-- The computed bounded packed checker decides precisely normalized seed
    acceptance, provided seed-carrier admission was established separately. -/
theorem check_exact {size : Nat} (rules : List (Rule (Fin size)))
    (carrier seed : List (Fin size)) (admitted : ∀ atom ∈ seed, atom ∈ carrier) :
    check rules carrier seed = true ↔ Semantics.Accept (program rules) (atoms carrier) (atoms seed) := by
  unfold check
  generalize result_eq : (close rules seed ((heads rules).length + 1)).get _ = result
  have completed : close rules seed ((heads rules).length + 1) = some result := by
    rw [← result_eq]
    exact (Option.some_get _).symm
  have least := (close_exact rules seed _ result completed).1
  simp only [Bool.and_eq_true, constraints_exact, agrees_exact carrier seed result admitted,
    least, Semantics.Accept]
  exact and_comm

/-- Under complete gate coverage, successful packed checking reconstructs an
    answer set. This is membership soundness, not candidate enumeration. -/
theorem check_sound {size : Nat} (rules : List (Rule (Fin size)))
    (carrier seed : List (Fin size)) (admitted : ∀ atom ∈ seed, atom ∈ carrier)
    (covered : Semantics.GateCarrier (program rules) (atoms carrier))
    (accepted : check rules carrier seed = true) :
    Semantics.Stable (program rules) (Semantics.Gamma (program rules) (atoms seed)) :=
  Semantics.accept_sound covered ((check_exact rules carrier seed admitted).mp accepted)

end Zetesis.Refinement.PackedAcceptance
