import AdmittedData
import ArcAllocation
import RuntimeOwnership

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Construction of an admitted shared theory

The actual generated `Theory::new` first admits its inputs and then invokes the
supplied `Arc::new` operation. `completed_phases` recovers both successful calls
from a successful constructor return; it does not assume that admission passed.
The explicit library contract in `returned_value` then establishes what the
returned theory stores, and `returned_structure` supplies the existing evaluator
and root-scan premises.

The generated binding carries an audited allocation-provider parameter. The
operation may fail or diverge, and no theorem assumes it returns. Its value law
and any heap transition are trusted library contracts for this invocation. A
single pure provider does not model repeated fresh allocations. No allocator,
reference-count, destruction, changing-control or complete membership-wrapper
claim is made here.
-/
namespace TheoryConstruction

variable [allocation : ArcAllocation]

/-- Admission refusal is returned before the supplied allocation operation.
The equation is independent of that operation's behavior. -/
theorem refused (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (reason : theory.AdmissionError)
    (admitted : theory.admit atoms nodes roots limits = ok (.Err reason)) :
    theory.Theory.new atoms nodes roots limits = ok (.Err reason) := by
  simp [theory.Theory.new, admitted, core.result.Result.Insts.CoreOpsTry.branch,
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual]

/-- After admission, the wrapper maps only the returned allocation to source
success. Backend failure or divergence is retained by this exact bind equation;
passing admission is not a promise of constructor completion. -/
theorem admitted_allocation (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits) (data : theory.Data)
    (admitted : theory.admit atoms nodes roots limits = ok (.Ok data)) :
    theory.Theory.new atoms nodes roots limits =
      (do let program ← alloc.sync.Arc.new data
          ok (core.result.Result.Ok program)) := by
  simp [theory.Theory.new, admitted, core.result.Result.Insts.CoreOpsTry.branch]

/-- A returned theory comes from actual successful admission and allocation of
exactly the supplied data. No inner-call equation or value-preservation premise
is assumed.

Proof: admission always returns its finite validator verdict. A refusal cannot
produce a returned theory. In the admitted case, allocation failure or divergence
also cannot produce that return; an allocation return determines the theory. -/
theorem completed_phases (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (program : theory.Theory)
    (completed : theory.Theory.new atoms nodes roots limits = ok (.Ok program)) :
    theory.admit atoms nodes roots limits = ok (.Ok { atoms, nodes, roots }) ∧
      alloc.sync.Arc.new ({ atoms, nodes, roots } : theory.Data) = ok program := by
  cases validated : Zetesis.Refinement.TheoryAdmission.validate Usize.max
      (AdmittedData.limitsOf limits) atoms.val
      (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) with
  | error reason =>
      have rejected : theory.admit atoms nodes roots limits =
          ok (.Err (AdmissionValidation.refusal reason)) := by
        rw [AdmittedData.admit_exact, validated]
      have stopped := refused atoms nodes roots limits
        (AdmissionValidation.refusal reason) rejected
      rw [stopped] at completed
      simp at completed
  | ok accepted =>
      cases accepted
      have admitted : theory.admit atoms nodes roots limits =
          ok (.Ok { atoms, nodes, roots }) := by
        rw [AdmittedData.admit_exact, validated]
      have fromAllocation := admitted_allocation atoms nodes roots limits
        { atoms, nodes, roots } admitted
      rw [fromAllocation] at completed
      have allocated : alloc.sync.Arc.new ({ atoms, nodes, roots } : theory.Data) =
          ok program := by
        cases outcome : alloc.sync.Arc.new ({ atoms, nodes, roots } : theory.Data) with
        | ret view =>
            have same : view = program := by
              simpa only [outcome, bind_tc_ok, Result.ok.injEq,
                core.result.Result.Ok.injEq] using completed
            simp only [same]
        | vis effect continuation =>
            simp only [outcome, bind_tc_vis, vis_not_ok] at completed
        | div =>
            simp only [outcome, bind_tc_div, div_not_ok] at completed
      exact ⟨admitted, allocated⟩

/-- A typed constructor refusal is exactly an admission refusal. Allocation is
not replaced by a typed admission error: its failure and divergence remain in
the outer backend result. -/
theorem refused_iff (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (reason : theory.AdmissionError) :
    theory.Theory.new atoms nodes roots limits = ok (.Err reason) ↔
      theory.admit atoms nodes roots limits = ok (.Err reason) := by
  constructor
  · intro completed
    cases validated : Zetesis.Refinement.TheoryAdmission.validate Usize.max
        (AdmittedData.limitsOf limits) atoms.val
        (nodes.val.map EvaluationSemantics.node) (roots.val.map UScalar.val) with
    | error authored =>
        have rejected : theory.admit atoms nodes roots limits =
            ok (.Err (AdmissionValidation.refusal authored)) := by
          rw [AdmittedData.admit_exact, validated]
        have stopped := refused atoms nodes roots limits
          (AdmissionValidation.refusal authored) rejected
        have same : AdmissionValidation.refusal authored = reason := by
          simpa only [Result.ok.injEq, core.result.Result.Err.injEq] using
            stopped.symm.trans completed
        simpa only [same] using rejected
    | ok accepted =>
        cases accepted
        have admitted : theory.admit atoms nodes roots limits =
            ok (.Ok { atoms, nodes, roots }) := by
          rw [AdmittedData.admit_exact, validated]
        rw [admitted_allocation atoms nodes roots limits { atoms, nodes, roots } admitted]
          at completed
        cases outcome : alloc.sync.Arc.new ({ atoms, nodes, roots } : theory.Data) with
        | ret view => simp only [outcome, bind_tc_ok, Result.ok.injEq, reduceCtorEq] at completed
        | vis effect continuation =>
            simp only [outcome, bind_tc_vis, vis_not_ok] at completed
        | div => simp only [outcome, bind_tc_div, div_not_ok] at completed
  · exact refused atoms nodes roots limits reason

/-- Under the library's explicit successful-value contract, a returned theory
stores the supplied atom count and vectors, retaining order and multiplicity.
Admission and allocation are derived from the actual constructor return. -/
theorem returned_value (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (completed : theory.Theory.new atoms nodes roots limits = ok (.Ok program)) :
    program.value = { atoms, nodes, roots } := by
  have allocated : alloc.sync.Arc.new ({ atoms, nodes, roots } : theory.Data) = ok program :=
    (completed_phases atoms nodes roots limits program completed).2
  exact preservesValue { atoms, nodes, roots } program allocated

/-- The same returned value satisfies the existing evaluator's ordered-node and
bounded-root premises. These are consequences of actual construction and the
library value contract, rather than separately assumed admission invariants. -/
theorem returned_structure (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (completed : theory.Theory.new atoms nodes roots limits = ok (.Ok program)) :
    EvaluationSpecification.Ordered program.value.nodes.val ∧
      ∀ root ∈ program.value.roots.val, root.val < program.value.nodes.val.length := by
  have admitted : theory.admit atoms nodes roots limits = ok (.Ok { atoms, nodes, roots }) :=
    (completed_phases atoms nodes roots limits program completed).1
  have stored : program.value = { atoms, nodes, roots } :=
    returned_value atoms nodes roots limits program preservesValue completed
  exact AdmittedData.admitted_program_structure atoms nodes roots limits
    { atoms, nodes, roots } admitted program stored

/-- An explicitly supplied fresh heap transition makes the returned theory
consistent and retains every previously consistent view under a different owner.
Freshness is a premise about this invocation's returned owner, not a theorem
about repeated calls to one pure allocation provider.

Proof: the value law identifies the new heap entry. An old consistent view
cannot use an owner absent from the prior heap; the other-entry premise then
preserves that view. -/
theorem returned_heap (atoms : Usize) (nodes : alloc.vec.Vec theory.Node)
    (roots : alloc.vec.Vec Usize) (limits : theory.AdmissionLimits)
    (program : theory.Theory)
    (preservesValue : ∀ (data : theory.Data) (view : theory.Theory),
      alloc.sync.Arc.new data = ok view → view.value = data)
    (completed : theory.Theory.new atoms nodes roots limits = ok (.Ok program))
    (before after : RuntimeOwnership.Heap theory.Data)
    (fresh : before program.owner = none)
    (stored : after program.owner = some ({ atoms, nodes, roots } : theory.Data))
    (otherEntries : ∀ owner, owner ≠ program.owner → after owner = before owner) :
    RuntimeOwnership.Consistent after program ∧
      ∀ previous : theory.Theory, RuntimeOwnership.Consistent before previous →
        previous.owner ≠ program.owner ∧ RuntimeOwnership.Consistent after previous := by
  have value : program.value = { atoms, nodes, roots } :=
    returned_value atoms nodes roots limits program preservesValue completed
  have newConsistent : RuntimeOwnership.Consistent after program := by
    rw [RuntimeOwnership.Consistent, value]
    exact stored
  have priorRetained : ∀ previous : theory.Theory,
      RuntimeOwnership.Consistent before previous →
        previous.owner ≠ program.owner ∧ RuntimeOwnership.Consistent after previous := by
    intro previous priorConsistent
    have distinct : previous.owner ≠ program.owner := by
      intro same
      have impossible : (none : Option theory.Data) = some previous.value := by
        calc
          none = before program.owner := fresh.symm
          _ = before previous.owner := congrArg before same.symm
          _ = some previous.value := priorConsistent
      cases impossible
    have retained : RuntimeOwnership.Consistent after previous := by
      change after previous.owner = some previous.value
      rw [otherEntries previous.owner distinct]
      exact priorConsistent
    exact ⟨distinct, retained⟩
  exact ⟨newConsistent, priorRetained⟩

end TheoryConstruction
