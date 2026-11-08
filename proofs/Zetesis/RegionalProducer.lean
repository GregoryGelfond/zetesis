import Zetesis.RegionalOriginal

/-!
# A held atom's sole surviving indexed producer

A producer inventory supplies each source's semantic head and body together
with its optional body ordinal on the decoded graph. The input column is
the region's computed original reading. Filtering the full inventory checks
only that particular body's ordinal, and exempts atomic choices from the
competing-head test. A unique survivor of a held atom publishes its body's
own ordinal. No equality test or global lookup of unfolded formulas is used
by the executable filter. The refinement starts at complete, linked original
producers and does not authenticate a compiled Rust owner or extraction.
-/
namespace Zetesis.Refinement.RegionalProducer

open Ferraris TightEvaluation MixedFormulaGraph CheckedSignedReplay RegionalOriginal
open StableProducerKnowledge DisjunctiveSupport
variable {α : Type}

/-- Each original producer borrows its body at the actual node ordinal, if present. -/
structure Indexed (α : Type) where
  source : Source α
  bodyId : Option Nat

/-- The ordinal denotes that very producer's body on this graph. Missing
bodies denote facts, and an out-of-range body cannot satisfy this relation. -/
def Linked (fs : List (Formula α)) (p : Indexed α) : Prop :=
  match p.bodyId with
  | none => p.source.body = none
  | some i => ∃ F, fs[i]? = some F ∧ p.source.body = some F

/-- Every original ordinary or choice row of a complete semantic inventory
has an indexed representative. Rows may repeat. The proof shows that any
supporter of a present atom appears among the survivors. -/
structure Inventory (T : Theory α) (fs : List (Formula α)) (rows : List (Indexed α)) : Prop where
  covered : ∃ rules choices, CoveredWithChoices T rules choices ∧
    (∀ s ∈ rules.map Source.ordinary ++ choices.map Source.choice, ∃ p ∈ rows, p.source = s)
  linked : ∀ p ∈ rows, Linked fs p

/-- Use the ordinary/choice head filter, followed by the concrete body's
indexed negative sign. A missing semantic body has no ordinal to block. -/
def eligible [DecidableEq α] (atoms : α → PartialBit) (bits : Array PartialBit) (a : α) (p : Indexed α) : Bool :=
  possible a (fun b => atoms b == some true) (fun _ => false) p.source &&
    !(p.bodyId.any fun i => bits[i]? == some (some false))

/-- A held atom with exactly one surviving source can publish that source's
body. Zero, multiple, bodyless, and open-head cases publish no body sign. -/
def soleBody [DecidableEq α] (atoms : α → PartialBit) (bits : Array PartialBit)
    (rows : List (Indexed α)) (a : α) : Option Nat :=
  if atoms a == some true then match rows.filter (eligible atoms bits a) with
    | [p] => p.bodyId
    | _ => none
  else none

/-- A semantic supporter cannot be discarded. Its body holds in this stable
completion, so the negative bit at the linked ordinal must be absent. -/
theorem supporter_eligible [DecidableEq α] {T : Theory α} {fs : List (Formula α)}
    {atoms : α → PartialBit} {bits : Array PartialBit} (column : Column T (region atoms) fs bits)
    {p : Indexed α} (linked : Linked fs p) {z : Atoms α}
    (inside : (region atoms).Contains z) (stable : Stable z T) {a : α}
    (supports : p.source.supports z a) : eligible atoms bits a p = true := by
  have headAllowed := supported_possible p.source supports
    (held := fun b => atoms b == some true) (never := fun _ => false)
    (fun b h => inside.1 b (beq_iff_eq.mp h)) (by simp)
  have live : (p.bodyId.any fun i => bits[i]? == some (some false)) = false := by
    cases id : p.bodyId with
    | none => rfl
    | some i =>
      obtain ⟨F, atId, body⟩ := (show ∃ F, fs[i]? = some F ∧ p.source.body = some F by
        simpa [Linked, id] using linked)
      have holds := supported_body p.source supports body
      apply Bool.eq_false_iff.mpr
      intro blocked
      have keyMatch : matchesKeys bits [⟨i,false⟩] = true := by
        simpa [matchesKeys] using blocked
      have sameRegion : FormulaBounds.Inside (region atoms) (region atoms) := ⟨fun _ h => h, fun _ h => h⟩
      obtain ⟨G, atKey, sign⟩ := matched_signs column sameRegion keyMatch ⟨i,false⟩ (by simp)
      change fs[i]? = some G at atKey
      rw [atId] at atKey
      cases atKey
      exact Bool.false_ne_true ((sign z inside stable).mp holds)
  simp [eligible, headAllowed, live]

/-- A completed sole-body scan justifies the positive sign at its returned
original ID for all stable completions. A real supporter exists by the
reduct support law, survives both blockers and hence is the unique row. -/
theorem sole_body_sound [DecidableEq α] {T : Theory α} {fs : List (Formula α)}
    {atoms : α → PartialBit} {bits : Array PartialBit} (column : Column T (region atoms) fs bits)
    {rows : List (Indexed α)} (inventory : Inventory T fs rows) (a : α) {i : Nat}
    (issued : soleBody atoms bits rows a = some i) :
    ∃ F, fs[i]? = some F ∧ Sign T (region atoms) F true := by
  by_cases held : atoms a = some true
  · cases survivors : rows.filter (eligible atoms bits a) with
    | nil => simp [soleBody, held, survivors] at issued
    | cons sole rest =>
      cases rest with
      | cons other more => simp [soleBody, held, survivors] at issued
      | nil =>
        have bodyId : sole.bodyId = some i := by simpa [soleBody, held, survivors] using issued
        have member : sole ∈ rows := (List.mem_filter.mp (survivors ▸ List.mem_cons_self)).1
        obtain ⟨F, atId, body⟩ := (show ∃ F, fs[i]? = some F ∧ sole.source.body = some F by
          simpa [Linked, bodyId] using inventory.linked sole member)
        refine ⟨F, atId, ?_⟩
        intro z inside stable
        have present : z a := inside.1 a held
        obtain ⟨rules, choices, coverage, mapped⟩ := inventory.covered
        have real : ∃ p ∈ rows, p.source.supports z a := by
          rcases answer_set_supported_with_choices z T rules choices coverage stable a present with
            ⟨s, mem, active⟩ | ⟨s, mem, active⟩
          · obtain ⟨p, here, eq⟩ := mapped (.ordinary s) (List.mem_append_left _ (List.mem_map_of_mem mem))
            exact ⟨p, here, eq ▸ active⟩
          · obtain ⟨p, here, eq⟩ := mapped (.choice s) (List.mem_append_right _ (List.mem_map_of_mem mem))
            exact ⟨p, here, eq ▸ active⟩
        obtain ⟨p, here, active⟩ := real
        have found := List.mem_filter.mpr ⟨here, supporter_eligible column (inventory.linked p here) inside stable active⟩
        have same : p = sole := by simpa [survivors] using found
        exact ⟨fun _ => rfl, fun _ => supported_body sole.source (same ▸ active) body⟩
  · simp [soleBody, held] at issued

/-- One full producer round applied to requested heads. A successful body
lookup marks its exact ordinal true. -/
def produced [DecidableEq α] (atoms : α → PartialBit) (column : Array PartialBit)
    (rows : List (Indexed α)) (heads : List α) : Nat → PartialBit :=
  let ids := heads.filterMap (soleBody atoms column rows)
  fun i => if ids.contains i then some true else none

/-- Every published ordinal is justified by a completed full producer scan;
repetitions of heads or returned IDs cannot introduce another polarity. -/
theorem produced_sound [DecidableEq α] {T : Theory α} {fs : List (Formula α)}
    {atoms : α → PartialBit} {bits : Array PartialBit} (column : Column T (region atoms) fs bits)
    {rows : List (Indexed α)} (inventory : Inventory T fs rows) (heads : List α)
    (i : Nat) (F : Formula α) (b : Bool) (entry : fs[i]? = some F)
    (issued : produced atoms bits rows heads i = some b) : Sign T (region atoms) F b := by
  unfold produced at issued
  split at issued
  next found =>
    cases issued
    obtain ⟨a, _, success⟩ := List.mem_filterMap.mp (List.contains_iff_mem.mp found)
    obtain ⟨G, atId, signed⟩ := sole_body_sound column inventory a success
    rw [entry] at atId
    cases atId
    exact signed
  next => contradiction

/-- A first original scan uses only held/cut atoms. Perform a complete
producer round on selected heads, rescan with the newly proved seed ordinals,
and match the supplied row. The final region-exclusion theorem requires no
semantic sign assumptions on any graph nodes. Complete producer coverage
and index links are the inventory's representation obligations. -/
theorem producer_row_excludes [DecidableEq α] {cols : Columns} {nodes : List (Node α)}
    {fs T : List (Formula α)} (decoded : decode cols [] nodes = some fs) (roots : List Nat)
    (asserted : OperandArena.gather fs roots = some T) (old tested : α → Bool)
    (strict : ProperSub (interpretation tested) (interpretation old)) (keys : List SignedOriginal.Key)
    {marks : Array PartialBit} (accepted : checkRow old tested cols nodes roots keys = some marks)
    (atoms : α → PartialBit) {initial : Array PartialBit}
    (first : scan atoms (fun _ => none) cols #[] nodes = some initial)
    (rows : List (Indexed α)) (inventory : Inventory T fs rows) (heads : List α)
    {bits : Array PartialBit} (second : scan atoms (produced atoms initial rows heads) cols #[] nodes = some bits)
    (matched : matchesKeys bits keys = true) (child : Region α) (descendant : FormulaBounds.Inside child (region atoms))
    (held : Sub (interpretation tested) child.lower) {a : α} (extra : child.lower a)
    (outside : ¬ interpretation tested a) :
    Models (interpretation old) T ∧ ReductRegions.Countermodel (interpretation old) T (interpretation tested) ∧
      ∀ z, child.Contains z → ¬ Stable z T := by
  obtain ⟨out, ran, column⟩ := scan_sound atoms (fun _ => none) cols nodes decoded
    (show Column T (region atoms) [] #[] by simp [Column]) (by simp)
  rw [first] at ran
  cases ran
  exact regional_row_excludes decoded roots asserted old tested strict keys accepted atoms _
    (produced_sound column inventory heads) second matched child descendant held extra outside

end Zetesis.Refinement.RegionalProducer
