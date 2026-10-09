import Zetesis.KeyedConstraints

/-!
# Constraints specialized by fixed witnesses

A finite family of witnesses can contain several witnesses for the same
interface key. If every retained witness condition is true, a constraint body
fires for some witness exactly when its residual body fires for a projected key.
The residual body depends only on that key: eliminated witness coordinates
cannot occur in it. Projection coverage is exact in both directions; neither
list needs to be duplicate-free.

For a source rewrite, establish the fixed-condition premise on every answer set
of the unchanged rest of the program, and instantiate `bodies_fire_together`.
`KeyedConstraints.asked_constraints_preserve` then supplies equality of answer
sets for the two finite constraint families. No new semantic atom is needed.

The finite witness family must cover the original fixed block. This module does
not establish that coverage from source producers, recognize variable scopes,
validate typed substitution, or preserve compiler diagnostics and resource stops.
It is a semantic library law, not a Rust refinement or a performance claim.
-/

namespace Zetesis.FixedWitnessConstraints

open Ferraris

universe u v w
variable {A : Type u} {W : Type v} {K : Type w}

/-- A constraint with a true fixed condition fires for some witness exactly when
its residual body fires for a projected key. The complete key projection keeps
whole keys together and may repeat them; it cannot omit or invent a key.

Forward, project the firing witness and keep its residual truth. Backward,
coverage supplies a witness for the firing key, and its fixed condition is true.
An empty witness family has no projected keys; a nonempty family may project to
one empty tuple. These cases need no separate nonemptiness assumption. -/
theorem bodies_fire_together (M : Atoms A) (witnesses : List W) (keys : List K)
    (key : W → K) (condition : W → Formula A) (body : K → Formula A)
    (fixed : ∀ witness ∈ witnesses, Satisfies M (condition witness))
    (coverage : ∀ candidate, candidate ∈ keys ↔
      ∃ witness ∈ witnesses, key witness = candidate) :
    (∃ witness ∈ witnesses,
      Satisfies M (.conj (condition witness) (body (key witness)))) ↔
      ∃ candidate ∈ keys, Satisfies M (body candidate) := by
  constructor
  · rintro ⟨witness, present, _, residual⟩
    have projected : key witness ∈ keys :=
      (coverage (key witness)).mpr ⟨witness, present, rfl⟩
    exact ⟨key witness, projected, residual⟩
  · rintro ⟨candidate, present, residual⟩
    obtain ⟨witness, inWitnesses, sameKey⟩ := (coverage candidate).mp present
    have fixedHolds : Satisfies M (condition witness) := fixed witness inWitnesses
    have residualHolds : Satisfies M (body (key witness)) := sameKey.symm ▸ residual
    exact ⟨witness, inWitnesses, fixedHolds, residualHolds⟩

end Zetesis.FixedWitnessConstraints
