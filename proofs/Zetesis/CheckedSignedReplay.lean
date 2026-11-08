import Zetesis.SignedOriginal

/-!
# Checking a sparse witness row on a mixed graph

A candidate region supplies signed original nodes on one immutable graph.
The executable checker propagates their consequences at a fixed witness. The checker reads the old candidate to authenticate the row, and never evaluates a new candidate. Only a definite true result at
every asserted frozen root can exclude a candidate containing the fixed
witness and another atom. -/
namespace Zetesis.Refinement.CheckedSignedReplay
open Ferraris TightEvaluation MixedFormulaGraph
variable {α : Type}

/-- An unestablished reading remains distinct from established falsehood. -/
abbrev PartialBit := Option Bool

/-- A known bit may be used in place of the exact reading. -/
def Refines (partialBit : PartialBit) (actual : Bool) : Prop := ∀ b, partialBit = some b → actual = b

/-- The decisive true branch, or the remaining branch when the first is false. -/
def disjoin : PartialBit → PartialBit → PartialBit
  | some true, _ | _, some true => some true
  | some false, x | x, some false => x
  | none, none => none

/-- Conjunction uses disjunction on the two complementary signs. -/
def conjoin (a b : PartialBit) : PartialBit := (disjoin (a.map (!·)) (b.map (!·))).map (!·)

/-- A known true branch proves a disjunction, and two known false branches refute it. -/
theorem or_sound {a b : PartialBit} {x y : Bool} (left : Refines a x) (right : Refines b y) :
    Refines (disjoin a b) (x || y) := by
  cases a with
  | none => cases b with
    | none => simp [Refines, disjoin]
    | some v => cases v <;> simp_all [Refines, disjoin]
  | some v => cases v <;> cases b with
    | none => simp_all [Refines, disjoin]
    | some w => cases w <;> simp_all [Refines, disjoin]

/-- Reversing a known Boolean preserves agreement with the reversed exact bit. -/
theorem neg_sound {a : PartialBit} {x : Bool} (sound : Refines a x) : Refines (a.map (!·)) (!x) := by
  cases a <;> simp_all [Refines]

/-- The dual of partial disjunction preserves exact conjunction whenever it returns a bit. -/
theorem and_sound {a b : PartialBit} {x y : Bool} (left : Refines a x) (right : Refines b y) :
    Refines (conjoin a b) (x && y) := by
  simpa [conjoin] using neg_sound (or_sound (neg_sound left) (neg_sound right))

/-- Reduce an ordered Boolean group, retaining decisive true and false readings. -/
def group (kind : OperandArena.Connective) (bits : Nat → PartialBit) : List Nat → PartialBit
  | [] => some (kind == .conj)
  | i :: rest => match kind with
    | .conj => conjoin (bits i) (group .conj bits rest)
    | .disj => disjoin (bits i) (group .disj bits rest)

/-- A definite finite group result agrees with reducing the actual signed child occurrences. -/
theorem group_sound (kind : OperandArena.Connective) (xs : List Nat)
    (bits : Nat → PartialBit) (values : Nat → Bool)
    (agrees : ∀ i ∈ xs, Refines (bits i) (values i)) :
    Refines (group kind bits xs) (OperandArena.reduce kind (xs.map values)) := by
  induction xs with
  | nil => cases kind <;> simp [group, OperandArena.reduce, Refines]
  | cons i tail next =>
    have head := agrees i List.mem_cons_self
    have suffix := next (fun j h => agrees j (List.mem_cons_of_mem _ h))
    cases kind with
    | conj => simpa [group, OperandArena.reduce] using and_sound head suffix
    | disj => simpa [group, OperandArena.reduce] using or_sound head suffix

/-- A false original parent needs no children. A true numeric parent
requires a reading of *every occurrence*. Unknown parent masks withhold
positive implications and numeric reads unless the inner reading already
forces falsity. -/
def checkLayer (tested : α → Bool) (arena : List Nat) (bits : Nat → PartialBit)
    (mask : PartialBit) : Layer α → PartialBit
  | .ordinary (.atom a) => some (tested a)
  | .ordinary .bot => some false
  | .ordinary (.group kind span) => group kind bits (OperandArena.contents arena span)
  | .ordinary (.imp a b) => conjoin mask (disjoin ((bits a).map (!·)) (bits b))
  | .numeric row test =>
    let active := (List.finRange row.size).all (fun i => (bits (row.condition i)).isSome)
    let inner := if active then some (NumericWitnessReplay.accepts row test
      (fun i => (bits (row.condition i)).getD false)) else none
    conjoin mask inner

noncomputable def frozenValue (outer tested : α → Bool) (f : Formula α) : Bool :=
  formulaValue tested (Reduct (interpretation outer) f)

/-- Every determined layer sign is the actual reduct bit. Conjunction and
disjunction admit decisive children. An atom reads the witness directly
because every candidate under consideration contains that witness. -/
theorem check_layer_sound (outer tested : α → Bool) (below : Sub (interpretation tested) (interpretation outer))
    (arena : List Nat) (earlier : List (Formula α)) (layer : Layer α)
    (valid : fits arena earlier.length layer = true) (bits : Nat → PartialBit) (mask : PartialBit)
    (children : ∀ i < earlier.length, Refines (bits i) (frozenValue outer tested (earlier.getD i .bot)))
    (original : Refines mask (formulaValue outer (MixedFormulaGraph.meaning arena earlier layer))) :
    Refines (checkLayer tested arena bits mask layer) (frozenValue outer tested (MixedFormulaGraph.meaning arena earlier layer)) := by
  classical
  cases layer with
  | ordinary node =>
    have admitted := MixedFormulaGraph.ordinary_fits valid
    cases node with
    | bot => simp [Refines, checkLayer, MixedFormulaGraph.meaning, OperandTable.decode, frozenValue, Reduct, formulaValue]
    | atom a =>
      have sub : tested a = true → outer a = true := below a
      cases old : outer a <;> cases test : tested a <;>
        simp_all [Refines, checkLayer, MixedFormulaGraph.meaning, OperandTable.decode, frozenValue, Reduct, formulaValue, interpretation]
    | group kind span =>
      let v := fun i => frozenValue outer tested (earlier.getD i .bot)
      have eqn := OperandArena.reduct_exact kind earlier outer tested arena span admitted
      rw [OperandArena.evaluate_map kind earlier .bot _ arena span admitted] at eqn
      change Refines (group kind bits (OperandArena.contents arena span))
        (formulaValue tested (Reduct (interpretation outer) (OperandArena.meaning kind earlier arena span)))
      rw [← Option.some.inj eqn]
      simpa [v, frozenValue, List.map_map, Function.comp_def] using
        group_sound kind (OperandArena.contents arena span) bits v (fun i h => children i (admitted.2 i h))
    | imp a b =>
      let f := earlier.getD a .bot
      let g := earlier.getD b .bot
      have proof := and_sound original (or_sound (neg_sound (children a admitted.1)) (children b admitted.2))
      have eqn : (formulaValue outer (.imp f g) && (!(frozenValue outer tested f) || frozenValue outer tested g))
          = frozenValue outer tested (.imp f g) := by
        apply Bool.eq_iff_iff.mpr
        simp only [Bool.and_eq_true, Bool.or_eq_true, Bool.not_eq_true', Bool.eq_false_iff, frozenValue,
          formula_value_true, RuleFactorization.reduct_imp, Classical.or_iff_not_imp_left, Classical.not_not]
      exact eqn ▸ proof
  | numeric row test =>
    let child := fun i => earlier.getD i (.bot : Formula α)
    let complete := (List.finRange row.size).all (fun i => (bits (row.condition i)).isSome)
    let guard := NumericWitnessReplay.accepts row test (fun i => frozenValue outer tested (child (row.condition i)))
    have inner : Refines (if complete then some (NumericWitnessReplay.accepts row test
        (fun i => (bits (row.condition i)).getD false)) else none) guard := by
      by_cases full : complete = true
      · have maskEq : (fun i => (bits (row.condition i)).getD false) =
            (fun i => frozenValue outer tested (child (row.condition i))) := by
          funext i
          obtain ⟨b, read⟩ := Option.isSome_iff_exists.mp (List.all_eq_true.mp full i (List.mem_finRange i))
          have bound := of_decide_eq_true (List.all_eq_true.mp valid i (List.mem_finRange i))
          simpa [read, child] using (children (row.condition i) bound b read).symm
        simp [Refines, full, maskEq, guard]
      · simp [Refines, full]
    have actual : (formulaValue outer (NumericWitnessReplay.meaning row test child) && guard)
        = frozenValue outer tested (NumericWitnessReplay.meaning row test child) := by
      apply Bool.eq_iff_iff.mpr
      rw [Bool.and_eq_true, frozenValue, formula_value_true, formula_value_true, NumericWitnessReplay.frozen_read]
      apply and_congr_right
      intro _
      apply iff_of_eq
      apply congrArg (· = true)
      apply congrArg (NumericWitnessReplay.accepts row test)
      funext i
      apply Bool.eq_iff_iff.mpr
      simp [frozenValue, AggregateReduct.active, formula_value_true]
    exact actual ▸ and_sound original inner

/-- Each computed bit is unknown or justified by predecessor readings and
the signed mask at the current original ordinal. -/
def verify (tested : α → Bool) (cols : Columns) (marks : Nat → PartialBit) (bits : Array PartialBit) :
    List (Node α) → Option (Array PartialBit)
  | [] => some bits
  | raw :: rest => do
    let layer ← resolve cols raw
    guard (fits cols.operands bits.size layer)
    let next := checkLayer tested cols.operands (fun i => bits[i]?.join) (marks bits.size) layer
    verify tested cols marks (bits.push next) rest

/-- An indexed partial column agrees with the semantic reduct where defined. -/
def SoundColumn (outer tested : α → Bool) (fs : List (Formula α)) (bits : Array PartialBit) : Prop :=
  bits.size = fs.length ∧
    ∀ i < fs.length, Refines bits[i]?.join (frozenValue outer tested (fs.getD i .bot))

/-- Appending one justified bit preserves the previously interpreted prefix and adds exactly its new layer. -/
theorem extend_column {outer tested : α → Bool} {fs : List (Formula α)} {bits : Array PartialBit}
    (earlier : SoundColumn outer tested fs bits) (F : Formula α) (next : PartialBit)
    (last : Refines next (frozenValue outer tested F)) :
    SoundColumn outer tested (fs ++ [F]) (bits.push next) := by
  refine ⟨by simp [earlier.1], ?_⟩
  intro i bound
  by_cases previous : i < fs.length
  · have different : i ≠ bits.size := by rw [earlier.1]; omega
    simpa [Array.getElem?_push, different, List.getD_eq_getElem?_getD, List.getElem?_append_left previous]
      using earlier.2 i previous
  · have index : i = fs.length := by simp at bound; omega
    have pos : i = bits.size := index.trans earlier.1.symm
    rw [show (bits.push next)[i]? = some next by rw [pos, Array.getElem?_push_size]]
    simpa [index, List.getD_eq_getElem?_getD] using last

/-- Check a whole sparse witness explanation. The only assumed signs are
the explicitly supplied *original* facts; every determined *frozen* fact
is derived. In particular no premise presupposes a correct dependency cone,
a correct selector, or satisfaction of the new reduct. -/
theorem verify_sound (outer tested : α → Bool) (cols : Columns) (marks : Nat → PartialBit)
    (nodes : List (Node α)) {earlier fs : List (Formula α)} {bits : Array PartialBit}
    (below : Sub (interpretation tested) (interpretation outer))
    (decoded : decode cols earlier nodes = some fs) (before : SoundColumn outer tested earlier bits)
    (original : ∀ i F, fs[i]? = some F → Refines (marks i) (formulaValue outer F)) :
    ∃ after, verify tested cols marks bits nodes = some after ∧ SoundColumn outer tested fs after := by
  induction nodes generalizing earlier bits with
  | nil =>
    simp [decode] at decoded
    exact ⟨bits, rfl, decoded ▸ before⟩
  | cons raw rest advance =>
    cases found : resolve cols raw with
    | none => simp [decode, found] at decoded
    | some layer =>
      by_cases checked : fits cols.operands earlier.length layer = true
      · simp [decode, found, checked, guard] at decoded
        let F := MixedFormulaGraph.meaning cols.operands earlier layer
        let next : PartialBit := checkLayer tested cols.operands (fun i => bits[i]?.join) (marks bits.size) layer
        obtain ⟨suffix, eqn, _⟩ := SignedOriginal.decode_extends cols _ rest decoded
        have id : fs[bits.size]? = some F := by
          simp [before.1, eqn, F]
        have signed := original bits.size F id
        have fact := check_layer_sound outer tested below cols.operands earlier layer checked _ _
          before.2 signed
        obtain ⟨after, finished, sound⟩ := advance decoded (extend_column before F next fact)
        exact ⟨after, by simpa [verify, found, before.1, checked, guard, next] using finished, sound⟩
      · simp [decode, found, checked, guard] at decoded
        contradiction

/-- Require a definite true at every asserted root. In particular an
unestablished value, an absent index and a definite false all decline. -/
def rootsConfirmed (bits : Array PartialBit) (roots : List Nat) : Bool :=
  roots.all (fun i => bits[i]? == some (some true))

/-- Ordered successful gathering makes every asserted formula an actual
member of the supplied root list, even in the presence of duplicates. -/
theorem gather_forall (fs : List (Formula α)) (roots : List Nat) {T : List (Formula α)}
    (source : OperandArena.gather fs roots = some T) (P : Formula α → Prop)
    (each : ∀ i ∈ roots, ∃ f, fs[i]? = some f ∧ P f) : ∀ f ∈ T, P f := by
  induction roots generalizing T with
  | nil => simp [OperandArena.gather] at source; subst T; simp
  | cons i rest next =>
    obtain ⟨g, here, holds⟩ := each i List.mem_cons_self
    cases tail : OperandArena.gather fs rest with
    | none => simp [OperandArena.gather, here, tail] at source
    | some out =>
      simp [OperandArena.gather, here, tail] at source
      subst T
      intro f member
      rcases List.mem_cons.mp member with eq | more
      · exact eq ▸ holds
      · exact next tail (fun j h => each j (List.mem_cons_of_mem _ h)) f more

/-- The checked original root extraction and a successful all-roots
certificate establish that the fixed witness models this candidate's
entire frozen theory. -/
theorem confirmed_roots_model (outer tested : α → Bool) {fs T : List (Formula α)} {bits : Array PartialBit}
    (roots : List Nat) (source : OperandArena.gather fs roots = some T)
    (column : SoundColumn outer tested fs bits) (confirmed : rootsConfirmed bits roots = true) :
    Models (interpretation tested) (ReductTheory (interpretation outer) T) := by
  have allTrue : ∀ i ∈ roots, ∃ F, fs[i]? = some F ∧ Satisfies (interpretation tested) (Reduct (interpretation outer) F) := by
    intro i member
    have yes : bits[i]? = some (some true) := beq_iff_eq.mp (List.all_eq_true.mp confirmed i member)
    have bound : i < fs.length := by
      rw [← column.1]
      exact (Array.getElem?_eq_some_iff.mp yes).choose
    refine ⟨fs[i], List.getElem?_eq_getElem bound, ?_⟩
    have known := column.2 i bound true (by simp [yes])
    apply (formula_value_true tested _).mp
    simpa [frozenValue, List.getD_eq_getElem?_getD, List.getElem?_eq_getElem bound] using known
  intro reduced present
  obtain ⟨f, member, rfl⟩ := List.mem_map.mp present
  exact gather_forall fs roots source _ allTrue f member

/-- Insert actual signed node IDs. The original column and the destination
must both contain the slot; a contradictory key cannot survive the check. -/
def insertKeys (original : Array Bool) (current : Array PartialBit) :
    List SignedOriginal.Key → Option (Array PartialBit)
  | [] => some current
  | k :: rest => if SignedOriginal.hasSign original k && k.id < current.size then
      insertKeys original (current.setIfInBounds k.id (some k.sign)) rest else none

/-- Every marked slot came from the previous marks or from an actual signed
input with exactly that ID and polarity. Relabeling an ID is not admissible. -/
theorem inserted_source (original : Array Bool) (keys : List SignedOriginal.Key)
    {before after : Array PartialBit} (inserted : insertKeys original before keys = some after)
    {i : Nat} {b : Bool} (found : after[i]? = some (some b)) :
    before[i]? = some (some b) ∨ ∃ k ∈ keys, k.id = i ∧ k.sign = b := by
  induction keys generalizing before with
  | nil => cases (Option.some.inj inserted); exact Or.inl found
  | cons k rest advance =>
    by_cases valid : SignedOriginal.hasSign original k && k.id < before.size
    · simp [insertKeys, valid] at inserted
      rcases advance inserted with earlier | ⟨q, member, same⟩
      · rw [Array.getElem?_setIfInBounds] at earlier
        by_cases here : k.id = i
        · have fits := of_decide_eq_true (Bool.and_eq_true_iff.mp valid).2
          exact Or.inr ⟨k, List.mem_cons_self, here, by simpa [here, here ▸ fits] using earlier⟩
        · exact Or.inl (by simpa [here] using earlier)
      · exact Or.inr ⟨q, List.mem_cons_of_mem _ member, same⟩
    · simp [insertKeys, valid] at inserted

/-- Roots have their original truth in every stable completion. Add their
authenticated positive signs to the nonroot conditions recorded in the row. -/
def withRoots (keys : List SignedOriginal.Key) (roots : List Nat) : List SignedOriginal.Key :=
  keys ++ roots.map (fun i => ⟨i,true⟩)

/-- Check the old frozen roots, reconstruct a signed mask on the same
graph, and require the sparse checker to establish all asserted frozen roots.
Strict containment of the authenticated witness uses interpretation sets. -/
def checkRow (old tested : α → Bool) (cols : Columns) (nodes : List (Node α))
    (roots : List Nat) (keys : List SignedOriginal.Key) : Option (Array PartialBit) := do
  let data ← run old tested cols ⟨#[],#[]⟩ nodes
  guard (IndexedEvaluation.roots data.reduct.toList roots == some true)
  let marks ← insertKeys data.original (Array.replicate data.original.size none) (withRoots keys roots)
  let determined ← verify tested cols (fun i => marks[i]?.join) #[] nodes
  guard (rootsConfirmed determined roots)
  return marks

/-- Every indexed original root named by the complete borrow belongs to
the semantic theory. Repeated roots need no special case. -/
theorem root_member {fs T : List (Formula α)} {roots : List Nat}
    (checked : OperandArena.gather fs roots = some T) {i : Nat} (member : i ∈ roots)
    {F : Formula α} (entry : fs[i]? = some F) : F ∈ T := by
  induction roots generalizing T with
  | nil => simp at member
  | cons j rest next =>
    cases here : fs[j]? with
    | none => simp [OperandArena.gather, here] at checked
    | some g =>
      cases tail : OperandArena.gather fs rest with
      | none => simp [OperandArena.gather, here, tail] at checked
      | some out =>
        simp [OperandArena.gather, here, tail] at checked
        subst T
        rcases List.mem_cons.mp member with same | later
        · subst j; rw [here] at entry; cases entry; exact List.mem_cons_self
        · exact List.mem_cons_of_mem _ (next tail later)

/-- Acceptance authenticates the old proper countermodel and excludes all
stable completions of the supplied region. The only external knowledge is
the original sign at each actual recorded ID in this very theory. Definite
truth at every new frozen root follows from the executable checker. -/
theorem checked_row_excludes {cols : Columns} {nodes : List (Node α)} {fs T : List (Formula α)}
    (decoded : decode cols [] nodes = some fs) (roots : List Nat)
    (source : OperandArena.gather fs roots = some T) (old tested : α → Bool)
    (strict : ProperSub (interpretation tested) (interpretation old)) (keys : List SignedOriginal.Key)
    {marks : Array PartialBit} (accepted : checkRow old tested cols nodes roots keys = some marks)
    (r : Region α) (held : Sub (interpretation tested) r.lower) {a : α}
    (extra : r.lower a) (outside : ¬ interpretation tested a)
    (signs : ∀ k ∈ keys, ∃ F, fs[k.id]? = some F ∧ StableProducerKnowledge.Sign T r F k.sign) :
    Models (interpretation old) T ∧ ReductRegions.Countermodel (interpretation old) T (interpretation tested) ∧
      ∀ z, r.Contains z → ¬ Stable z T := by
  classical
  cases ran : run old tested cols ⟨#[],#[]⟩ nodes with
  | none => simp [checkRow, ran] at accepted
  | some data =>
    by_cases rootsOK : IndexedEvaluation.roots data.reduct.toList roots = some true
    · cases built : insertKeys data.original (Array.replicate data.original.size none) (withRoots keys roots) with
      | none => simp [checkRow, ran, rootsOK, built, guard] at accepted
      | some mask =>
        cases replay : verify tested cols (fun i => mask[i]?.join) #[] nodes with
        | none => simp [checkRow, ran, rootsOK, built, replay, guard] at accepted
        | some after =>
          by_cases definite : rootsConfirmed after roots = true
          · have oldBits := run_refines_evaluate old tested cols nodes ⟨#[],#[]⟩
            rw [ran] at oldBits
            change some data.view = evaluate old tested cols ⟨[],[]⟩ nodes at oldBits
            have originalWitness : Models (interpretation tested) (ReductTheory (interpretation old) T) := by
              apply (frozen_roots_exact old tested cols nodes roots decoded source).mp
              rw [← oldBits]
              exact rootsOK
            have oldModel : Models (interpretation old) T := fun F member =>
              TightPlans.original_of_reduct _ _ F (originalWitness _ (List.mem_map.mpr ⟨F, member, rfl⟩))
            refine ⟨oldModel, ⟨strict, originalWitness⟩, ?_⟩
            intro z inside stable
            let candidate := fun atom => decide (z atom)
            have represented : interpretation candidate = z := funext fun atom => propext (by simp [interpretation, candidate])
            have contained := sub_trans held inside.1
            have below : Sub (interpretation tested) (interpretation candidate) := represented ▸ contained
            have signed : ∀ (i : Nat) (F : Formula α), fs[i]? = some F → Refines (mask[i]?.join) (formulaValue candidate F) := by
              intro i F inTable b observed
              cases mark : mask[i]? with
              | none => simp [mark] at observed
              | some bit =>
                simp [mark] at observed
                subst bit
                rcases inserted_source _ _ built mark with empty | ⟨k, member, same, val⟩
                · simp [Array.getElem?_replicate] at empty
                · rcases List.mem_append.mp member with explicit | root
                  · obtain ⟨g, fAt, sign⟩ := signs k explicit
                    rw [same, inTable] at fAt
                    cases fAt
                    apply Bool.eq_iff_iff.mpr
                    rw [formula_value_true, represented, ← val]
                    exact sign z inside stable
                  · obtain ⟨id, present, rfl⟩ := List.mem_map.mp root
                    simp at same val
                    subst id; subst b
                    apply (formula_value_true candidate _).mpr
                    rw [represented]
                    exact stable.1 F (root_member source present inTable)
            obtain ⟨verified, result, correct⟩ := verify_sound candidate tested cols _ nodes below decoded
              (show SoundColumn candidate tested [] #[] by simp [SoundColumn]) signed
            rw [replay] at result
            cases result
            have counter := confirmed_roots_model candidate tested roots source correct definite
            rw [represented] at counter
            exact stable.2 ⟨_, ⟨contained, fun back => outside (back a (inside.1 a extra))⟩, counter⟩
          · simp [checkRow, ran, rootsOK, built, replay, definite, guard] at accepted
            contradiction
    · simp [checkRow, ran, rootsOK, guard] at accepted
      contradiction

end Zetesis.Refinement.CheckedSignedReplay
