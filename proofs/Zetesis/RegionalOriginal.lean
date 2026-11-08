import Zetesis.CheckedSignedReplay

/-!
# Original signs in a candidate region

A partial assignment supplies held and cut atoms. A single forward scan
computes original signs in graph order. A guard can be determined even
when some of its conditions remain open, by evaluating the lower and upper
occurrence selections. Separately justified original signs may seed this
scan. Only established readings at the requested ordinals can authorize
the corresponding conditions of a checked reduct-witness row. The graph,
theory and enclosing region are shared throughout. This is a reference
scan over admitted columns. It leaves the Rust ownership, finite-width
arithmetic, closure schedules and resource accounting to their callers.
-/
namespace Zetesis.Refinement.RegionalOriginal

open Ferraris TightEvaluation MixedFormulaGraph CheckedSignedReplay
open NumericWitnessReplay (Row Guard)
variable {α : Type}

/-- Held, cut and open atoms denote a closed interval of interpretations. -/
def region (atoms : α → PartialBit) : Region α :=
  ⟨fun a => atoms a = some true, fun a => atoms a ≠ some false⟩

/-- A narrower selection is an ordered sublist of the larger one.
Repeated values and equal condition IDs retain their multiplicity. -/
theorem selection_sublist (row : Row) (p q : Fin row.size → Bool)
    (inclusion : ∀ i, p i = true → q i = true) :
    (RankedExtrema.selectedValues row.value p).Sublist (RankedExtrema.selectedValues row.value q) := by
  apply List.Sublist.map
  have ordered (indices : List (Fin row.size)) : (indices.filter p).Sublist (indices.filter q) := by
    induction indices with
    | nil => exact .slnil
    | cons i rest ih =>
      cases hp : p i with
      | false =>
        cases hq : q i
        · simpa [hp, hq] using ih
        · simpa [hp, hq] using ih.cons i
      | true =>
        simpa [hp, inclusion i hp] using ih.cons_cons i
  exact ordered _

/-- Adding selected occurrences increases a natural weight sum, lowers the
minimum and raises the maximum. This includes the designated empty endpoints. -/
theorem selection_bounds {xs ys : List Nat} (order : xs.Sublist ys) (greatest : Nat) :
    xs.sum ≤ ys.sum ∧
    (OrdinalExtrema.measure greatest ys).1 ≤ (OrdinalExtrema.measure greatest xs).1 ∧
    (OrdinalExtrema.measure greatest xs).2 ≤ (OrdinalExtrema.measure greatest ys).2 := by
  induction order with
  | slnil => exact ⟨Nat.le_refl _, Nat.le_refl _, Nat.le_refl _⟩
  | cons x _ ih =>
    obtain ⟨sum, lo, hi⟩ := ih
    simp only [List.sum_cons, OrdinalExtrema.measure, List.foldr_cons] at *
    omega
  | cons_cons x _ ih =>
    obtain ⟨sum, lo, hi⟩ := ih
    simp only [List.sum_cons, OrdinalExtrema.measure, List.foldr_cons] at *
    omega

/-- Each admitted primitive guard is increasing in its occurrence mask. -/
theorem guard_mono (row : Row) (test : Guard) (p q : Fin row.size → Bool)
    (inclusion : ∀ i, p i = true → q i = true) :
    NumericWitnessReplay.accepts row test p = true → NumericWitnessReplay.accepts row test q = true := by
  have order := selection_sublist row p q inclusion
  cases test with
  | threshold bound =>
    have sum := (selection_bounds order 0).1
    simp only [NumericWitnessReplay.accepts, decide_eq_true_eq]
    omega
  | «prefix» greatest cut =>
    have minimum := (selection_bounds order greatest).2.1
    simp only [NumericWitnessReplay.accepts, decide_eq_true_eq]
    omega
  | «suffix» greatest cut =>
    have maximum := (selection_bounds order greatest).2.2
    simp only [NumericWitnessReplay.accepts, decide_eq_true_eq]
    omega

/-- The lower mask keeps the definitely true occurrences. The upper mask
also keeps the unknown ones. A positive lower guard or a negative upper
guard determines the guard for every completion. -/
def numeric (row : Row) (test : Guard) (bits : Nat → PartialBit) : PartialBit :=
  if NumericWitnessReplay.accepts row test (fun i => (bits (row.condition i)).getD false)
  then some true
  else if NumericWitnessReplay.accepts row test (fun i => (bits (row.condition i)).getD true)
  then none else some false

/-- The exact mask of any compatible completion lies between those two masks. -/
theorem numeric_sound (row : Row) (test : Guard) (bits : Nat → PartialBit)
    (actual : Fin row.size → Bool) (agrees : ∀ i, Refines (bits (row.condition i)) (actual i)) :
    Refines (numeric row test bits) (NumericWitnessReplay.accepts row test actual) := by
  let lower := fun i => (bits (row.condition i)).getD false
  let upper := fun i => (bits (row.condition i)).getD true
  have below : ∀ i, lower i = true → actual i = true := by
    intro i h
    cases eq : bits (row.condition i) with
    | none => simp [lower, eq] at h
    | some b => exact agrees i true (by simpa [lower, eq] using h)
  have above : ∀ i, actual i = true → upper i = true := by
    intro i h
    cases eq : bits (row.condition i) with
    | none => simp [upper, eq]
    | some b => simpa [upper, eq, ← agrees i b eq] using h
  change Refines (if NumericWitnessReplay.accepts row test lower then some true else
    if NumericWitnessReplay.accepts row test upper then none else some false) _
  cases low : NumericWitnessReplay.accepts row test lower with
  | true =>
    have certain := guard_mono row test lower actual below low
    simp [Refines, certain]
  | false =>
    cases high : NumericWitnessReplay.accepts row test upper with
    | false =>
      have impossible := Bool.eq_false_iff.mpr (fun h => Bool.false_ne_true (high ▸ guard_mono row test actual upper above h))
      simp [Refines, impossible]
    | true => simp [Refines]

/-- Read one original node from predecessor signs without freezing its parent. -/
def readLayer (atoms : α → PartialBit) (arena : List Nat) (bits : Nat → PartialBit) : Layer α → PartialBit
  | .ordinary (.atom a) => atoms a
  | .ordinary .bot => some false
  | .ordinary (.imp a b) => disjoin ((bits a).map (!·)) (bits b)
  | .ordinary (.group kind span) => group kind bits (OperandArena.contents arena span)
  | .numeric row test => numeric row test bits

/-- The mixed graph lowering assigns the computed sign to this very layer.
Implication exchanges polarity in its antecedent. Groups preserve every
operand, and the numeric guard uses its checked row's condition ordinals. -/
theorem read_layer_sound (world : α → Bool) (atoms : α → PartialBit)
    (atomSound : ∀ a, Refines (atoms a) (world a)) (arena : List Nat)
    (earlier : List (Formula α)) (bits : Nat → PartialBit) (layer : Layer α)
    (valid : fits arena earlier.length layer = true)
    (children : ∀ i < earlier.length, Refines (bits i) (formulaValue world (earlier.getD i .bot))) :
    Refines (readLayer atoms arena bits layer) (formulaValue world (meaning arena earlier layer)) := by
  cases layer with
  | ordinary node =>
    have admitted := ordinary_fits valid
    cases node with
    | atom a => exact atomSound a
    | bot => simp [readLayer, Refines, meaning, OperandTable.decode, formulaValue]
    | imp a b => exact or_sound (neg_sound (children a admitted.1)) (children b admitted.2)
    | group kind span =>
      have exactValue := OperandArena.original_exact kind earlier world arena span admitted
      rw [OperandArena.evaluate_map kind earlier .bot _ arena span admitted] at exactValue
      rw [show formulaValue world (meaning arena earlier (.ordinary (.group kind span))) = _
        from (Option.some.inj exactValue).symm]
      simpa [readLayer, List.map_map, Function.comp_def] using
        group_sound kind (OperandArena.contents arena span) bits _ (fun i h => children i (admitted.2 i h))
  | numeric row test =>
    let actual := fun i => formulaValue world (earlier.getD (row.condition i) .bot)
    have source := (layer_exact world world arena earlier (.numeric row test) valid).1
    have mask : (fun i => (earlier.map (formulaValue world)).getD (row.condition i) false) = actual := by
      funext i
      have bound := of_decide_eq_true (List.all_eq_true.mp valid i (List.mem_finRange i))
      simp [actual, List.getD_eq_getElem?_getD, List.getElem?_map, List.getElem?_eq_getElem bound]
    change some (NumericWitnessReplay.accepts row test _) = some _ at source
    rw [mask] at source
    rw [← Option.some.inj source]
    exact numeric_sound row test bits actual (fun i =>
      children (row.condition i) (of_decide_eq_true (List.all_eq_true.mp valid i (List.mem_finRange i))))

/-- Visit the raw graph in order, validating every edge even when a producer
already settled this original node. A seed can supply a sign whose operands
alone do not decide it. -/
def scan (atoms : α → PartialBit) (seeds : Nat → PartialBit) (cols : Columns)
    (bits : Array PartialBit) : List (Node α) → Option (Array PartialBit)
  | [] => some bits
  | raw :: rest => do
    let layer ← resolve cols raw
    guard (fits cols.operands bits.size layer)
    let next := (seeds bits.size).orElse (fun _ => readLayer atoms cols.operands (fun i => bits[i]?.join) layer)
    scan atoms seeds cols (bits.push next) rest

/-- An indexed prefix has the decoded length and gives only justified
original signs in stable completions of this specific region and theory. -/
def Column (T : Theory α) (r : Region α) (fs : List (Formula α)) (bits : Array PartialBit) : Prop :=
  bits.size = fs.length ∧ ∀ world : α → Bool, r.Contains (interpretation world) →
    Stable (interpretation world) T →
    ∀ i < fs.length, Refines bits[i]?.join (formulaValue world (fs.getD i .bot))

/-- Appending a sound node sign extends the exact-index prefix invariant. -/
theorem extend {T : Theory α} {r : Region α} {fs : List (Formula α)} {bits : Array PartialBit}
    (established : Column T r fs bits) (F : Formula α) (next : PartialBit)
    (valid : ∀ world, r.Contains (interpretation world) → Stable (interpretation world) T →
      Refines next (formulaValue world F)) : Column T r (fs ++ [F]) (bits.push next) := by
  refine ⟨by simp [established.1], ?_⟩
  intro world inside stable i bound
  by_cases old : i < fs.length
  · have different : i ≠ bits.size := by rw [established.1]; omega
    simpa [Array.getElem?_push, different, List.getD_eq_getElem?_getD, List.getElem?_append_left old]
      using established.2 world inside stable i old
  · have index : i = fs.length := by simp at bound; omega
    have position : i = bits.size := index.trans established.1.symm
    rw [show (bits.push next)[i]? = some next by rw [position, Array.getElem?_push_size]]
    simpa [index, List.getD_eq_getElem?_getD] using valid world inside stable

/-- A box's held and cut bits agree with every interpretation it contains. -/
theorem atoms_sound (atoms : α → PartialBit) (world : α → Bool)
    (inside : (region atoms).Contains (interpretation world)) (a : α) : Refines (atoms a) (world a) := by
  intro b marked
  cases b with
  | true => exact inside.1 a marked
  | false => exact Bool.eq_false_iff.mpr (fun h => inside.2 a h marked)

/-- From the empty prefix this scan constructs a column on the whole graph.
At each step, either the earlier child signs justify the new sign by the
layer law, or an external seed at this same ordinal has a stable derivation.
Both branches concern the original interpretation. -/
theorem scan_sound (atoms : α → PartialBit) (seeds : Nat → PartialBit) (cols : Columns)
    (nodes : List (Node α)) {fs earlier : List (Formula α)} {T : Theory α} {bits : Array PartialBit}
    (decoded : decode cols earlier nodes = some fs) (before : Column T (region atoms) earlier bits)
    (seeded : ∀ i F b, fs[i]? = some F → seeds i = some b → StableProducerKnowledge.Sign T (region atoms) F b) :
    ∃ out, scan atoms seeds cols bits nodes = some out ∧ Column T (region atoms) fs out := by
  induction nodes generalizing earlier bits with
  | nil => simp [decode] at decoded; exact ⟨bits, rfl, decoded ▸ before⟩
  | cons raw rest advance =>
    cases resolved : resolve cols raw with
    | none => simp [decode, resolved] at decoded
    | some layer =>
      by_cases valid : fits cols.operands earlier.length layer = true
      · simp [decode, resolved, valid, guard] at decoded
        let F := meaning cols.operands earlier layer
        let fromChildren := readLayer atoms cols.operands (fun i => bits[i]?.join) layer
        let next := (seeds bits.size).orElse (fun _ => fromChildren)
        obtain ⟨tail, full, _⟩ := SignedOriginal.decode_extends cols _ rest decoded
        have ordinal : fs[bits.size]? = some F := by simp [before.1, full, F]
        have layerSound (world : α → Bool) (inside : (region atoms).Contains (interpretation world))
            (stable : Stable (interpretation world) T) : Refines next (formulaValue world F) := by
          cases seed : seeds bits.size with
          | none =>
            simpa [next, seed, fromChildren, F] using (read_layer_sound world atoms
              (atoms_sound atoms world inside) _ earlier _ layer valid (before.2 world inside stable))
          | some b =>
            have signed := seeded _ F b ordinal seed (interpretation world) inside stable
            have value := Bool.eq_iff_iff.mpr ((formula_value_true world F).trans signed)
            simp [next, seed, value, Refines]
        obtain ⟨out, ran, sound⟩ := advance decoded (extend before F next layerSound)
        exact ⟨out, by simpa [scan, resolved, before.1, valid, guard, next, fromChildren] using ran, sound⟩
      · simp [decode, resolved, valid, guard] at decoded
        contradiction

/-- All requested IDs must have their specified original signs. Missing and
open positions fail for both polarities, including a requested false sign. -/
def matchesKeys (bits : Array PartialBit) (keys : List SignedOriginal.Key) : Bool :=
  keys.all (fun k => bits[k.id]? == some (some k.sign))

/-- A matching row carries its signs to each contained region. Existence at
the actual decoded index follows from column length even if the ancestor
has no stable completions. Only descendants can use its evidence. -/
theorem matched_signs {T : Theory α} {parent child : Region α} {fs : List (Formula α)}
    {bits : Array PartialBit} (column : Column T parent fs bits) (descendant : FormulaBounds.Inside child parent)
    {keys : List SignedOriginal.Key} (matched : matchesKeys bits keys = true) :
    ∀ k ∈ keys, ∃ F, fs[k.id]? = some F ∧ StableProducerKnowledge.Sign T child F k.sign := by
  classical
  intro k member
  have atId : bits[k.id]? = some (some k.sign) := beq_iff_eq.mp (List.all_eq_true.mp matched k member)
  have bound : k.id < fs.length := column.1 ▸ (Array.getElem?_eq_some_iff.mp atId).choose
  refine ⟨fs[k.id], List.getElem?_eq_getElem bound, ?_⟩
  intro z inside stable
  have parentContains : parent.Contains z := ⟨sub_trans descendant.1 inside.1, sub_trans inside.2 descendant.2⟩
  let world := fun a => decide (z a)
  have represented : interpretation world = z := funext fun a => propext (by simp [interpretation, world])
  have agreement := column.2 world (represented ▸ parentContains) (represented ▸ stable) k.id bound
    k.sign (by simp [atId])
  have value : formulaValue world fs[k.id] = k.sign := by
    simpa [List.getD_eq_getElem?_getD, List.getElem?_eq_getElem bound] using agreement
  rw [← represented, ← formula_value_true, value]

/-- Combining a successful original scan and a successful frozen-row check
excludes every stable completion of the child. Holding the fixed witness
and an additional atom is required only at that child. External seeds must
have provenance on the parent. All remaining nonroot signs follow from
the scan. With empty seeds the seed premise is vacuous. -/
theorem regional_row_excludes {cols : Columns} {nodes : List (Node α)} {fs T : List (Formula α)}
    (decoded : decode cols [] nodes = some fs) (roots : List Nat)
    (asserted : OperandArena.gather fs roots = some T) (old tested : α → Bool)
    (strict : ProperSub (interpretation tested) (interpretation old)) (keys : List SignedOriginal.Key)
    {marks : Array PartialBit} (accepted : checkRow old tested cols nodes roots keys = some marks)
    (atoms : α → PartialBit) (seeds : Nat → PartialBit)
    (seeded : ∀ i F b, fs[i]? = some F → seeds i = some b → StableProducerKnowledge.Sign T (region atoms) F b)
    {bits : Array PartialBit} (computed : scan atoms seeds cols #[] nodes = some bits)
    (matched : matchesKeys bits keys = true) (child : Region α) (descendant : FormulaBounds.Inside child (region atoms))
    (held : Sub (interpretation tested) child.lower) {a : α} (extra : child.lower a)
    (outside : ¬ interpretation tested a) :
    Models (interpretation old) T ∧ ReductRegions.Countermodel (interpretation old) T (interpretation tested) ∧
      ∀ z, child.Contains z → ¬ Stable z T := by
  obtain ⟨out, complete, sound⟩ := scan_sound atoms seeds cols nodes decoded
    (show Column T (region atoms) [] #[] by simp [Column]) seeded
  rw [computed] at complete
  cases complete
  exact checked_row_excludes decoded roots asserted old tested strict keys accepted child held extra outside
    (matched_signs sound descendant matched)

end Zetesis.Refinement.RegionalOriginal
