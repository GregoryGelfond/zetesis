import Zetesis.Core

/-!
# Upper domains for source argument producers

A domain is a predicate on values. `Full` denotes Unknown: it admits every value
and contributes no restriction to an intersection. A producer describes one
output argument, a local value bound and the mandatory positive arguments that
must admit the same value. Its proposal intersects those bounds. Alternative
producers for an output contribute their union, since any one can supply it.
A constant head may use a singleton local bound with no inputs even when its
source body has no witness; this deliberately overapproximates source values.
An unsupported head may use `Full` with no inputs. An unknown input alongside a
finite input contributes the universal predicate, leaving the finite restriction.

Transfer is monotone under pointwise domain inclusion. Every finite abstract
argument-value derivation belongs to any assignment closed under the transfer.
Induction is on the derivation, so the producer dependency graph may be cyclic;
a cycle without a finite derivation supplies no value by itself. These are
possible-value derivations, not source facts or answer-set membership proofs.

The definitions use only predicate sets. They do not prove that normalized
source-to-producer extraction covers every actual argument value, or that Rust
finite sets and Unknown implement these predicates and transfers.
Those correspondences, completed fixed-point computation, finite-width
arithmetic, work limits and error preservation remain separate obligations.
This upstream coverage contract can supply a premise to `DomainBindings`; it
does not by itself establish that consumer's complete-binding coverage.
-/

namespace Zetesis.DomainProducers

universe u v
variable {Slot : Type u} {Value : Type v}

/-- One alternative way to supply a possible value at an output argument.
The local bound covers constants or other externally justified restrictions;
`Full` records that no local restriction is known. Inputs are mandatory positive
occurrences of the same value, so their domains are intersected. Repeated inputs
are allowed, and no acyclic ordering is assumed. -/
structure Producer (Slot : Type u) (Value : Type v) where
  output : Slot
  inputs : List Slot
  bound : Atoms Value

/-- One producer's local bound intersected with all its mandatory input domains.
An empty input list adds no restriction; an input with domain `Full` does not
restrict the other inputs. -/
def Proposal (producer : Producer Slot Value) (domains : Slot → Atoms Value) : Atoms Value :=
  fun value => producer.bound value ∧ ∀ input ∈ producer.inputs, domains input value

/-- Union of proposals from every alternative producer of an output argument.
An output with no producer has no proposed value. -/
def transfer (producers : List (Producer Slot Value))
    (domains : Slot → Atoms Value) : Slot → Atoms Value :=
  fun output value => ∃ producer ∈ producers,
    producer.output = output ∧ Proposal producer domains value

/-- Enlarging every input domain can only enlarge the transferred domains.
Retain the same alternative producer and local bound; each mandatory input
membership passes through its pointwise inclusion. -/
theorem transfer_monotone (producers : List (Producer Slot Value))
    (smaller larger : Slot → Atoms Value)
    (included : ∀ slot, Sub (smaller slot) (larger slot)) :
    ∀ slot, Sub (transfer producers smaller slot) (transfer producers larger slot) := by
  intro slot value proposed
  obtain ⟨producer, member, output, allowed, inputs⟩ := proposed
  have largerInputs : ∀ input ∈ producer.inputs, larger input value := by
    intro input occurs
    exact included input value (inputs input occurs)
  exact ⟨producer, member, output, allowed, largerInputs⟩

/-- A closed upper-bound assignment already contains every value its producers
can propose from that assignment. Closure is a premise, not a claim that any
particular iteration schedule has completed. -/
def Closed (producers : List (Producer Slot Value)) (domains : Slot → Atoms Value) : Prop :=
  ∀ slot, Sub (transfer producers domains slot) (domains slot)

/-- Finite abstract argument-value derivations. One producer supplies the output
when its local bound holds and every mandatory input has a derivation for the
same value. Zero-input producers seed derivations from their local bound.
This grammar permits recursive dependencies without assuming their termination. -/
inductive Derives (producers : List (Producer Slot Value)) : Slot → Value → Prop where
  | produce (producer : Producer Slot Value) (member : producer ∈ producers)
      (value : Value) (allowed : producer.bound value)
      (inputs : ∀ input ∈ producer.inputs, Derives producers input value) :
      Derives producers producer.output value

/-- Every finite abstract derivation is covered by any closed upper-bound
assignment, including derivations through recursive producer dependencies.

Induct on the derivation. The child induction hypotheses place all mandatory
inputs in the assignment. Together with the unchanged local bound they give a
proposal from this producer; closure then covers its output. No acyclic rank,
source-body witness or assertion of answer-set truth is required. -/
theorem derivation_covered (producers : List (Producer Slot Value))
    (domains : Slot → Atoms Value) (closed : Closed producers domains)
    {slot : Slot} {value : Value} (derived : Derives producers slot value) :
    domains slot value := by
  induction derived with
  | produce producer member value allowed inputs induction =>
    have coveredInputs : ∀ input ∈ producer.inputs, domains input value := by
      intro input occurs
      exact induction input occurs
    have proposed : transfer producers domains producer.output value :=
      ⟨producer, member, rfl, allowed, coveredInputs⟩
    exact closed producer.output value proposed

end Zetesis.DomainProducers
