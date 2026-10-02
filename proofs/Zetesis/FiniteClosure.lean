import Zetesis.Semantics

/-!
# Executable least closure of a finite normalized reduct

The finite presentation below has already discharged ground filters: rules with
false filters have been omitted, and every retained filter evaluated to true.
Its denotation is the existing normalized-rule semantics. A scan visits rules
in list order and immediately publishes a fresh head;
later rules in that same scan may use it. Positive truth starts empty, while
the seed is read only for frozen gates.

Sound insertions and an unchanged complete scan establish leastness. The finite
list of rule heads bounds growth without a finite ambient atom type. Fuel counts
whole scans, not machine operations. These executable Lean definitions and their
proofs do not establish a correspondence to Rust, source grounding, storage,
cancellation or device execution.
-/

namespace Zetesis.FiniteClosure

universe u
variable {α : Type u}

/-- A finite ground rule after its filter has evaluated to true.
    A missing head is a constraint, never a consequence to insert. -/
structure Rule (α : Type u) where
  head : Option α
  positive : List α
  gateTrue : List α
  gateFalse : List α
deriving DecidableEq

/-- This executable presentation denotes the existing normalized rule. -/
def Rule.denote (rule : Rule α) : Semantics.Rule α :=
  ⟨rule.head, rule.positive, rule.gateTrue, rule.gateFalse, True⟩

def program (rules : List (Rule α)) : Semantics.Program α := rules.map Rule.denote

def atoms (values : List α) : Atoms α := fun atom => atom ∈ values

def heads (rules : List (Rule α)) : List α := rules.filterMap Rule.head

variable [DecidableEq α]

/-- Only the frozen seed is consulted for the two gate polarities. -/
def gates (rule : Rule α) (seed : List α) : Bool :=
  rule.gateTrue.all (fun atom => decide (atom ∈ seed)) &&
    rule.gateFalse.all (fun atom => decide (atom ∉ seed))

def body (rule : Rule α) (known : List α) : Bool :=
  rule.positive.all (fun atom => decide (atom ∈ known))

theorem gates_exact (rule : Rule α) (seed : List α) :
    gates rule seed = true ↔ Semantics.Gate rule.denote (atoms seed) := by
  simp [gates, Semantics.Gate, Rule.denote, atoms, List.all_eq_true]

theorem body_exact (rule : Rule α) (known : List α) :
    body rule known = true ↔ Semantics.Body rule.denote (atoms known) := by
  simp [body, Semantics.Body, Rule.denote, atoms, List.all_eq_true]

/-- Insert one justified head unless it is already present. Constraints do not
    modify positive truth; they are checked against the completed closure. -/
def step (rule : Rule α) (seed known : List α) : List α :=
  match rule.head with
  | none => known
  | some head =>
      if gates rule seed && body rule known then
        if head ∈ known then known else head :: known
      else known

/-- A sequential, inflationary scan. Each step reads its predecessors' output. -/
def scan (rules : List (Rule α)) (seed known : List α) : List α :=
  rules.foldl (fun current rule => step rule seed current) known

theorem step_retains (rule : Rule α) (seed known : List α) :
    known.Sublist (step rule seed known) := by
  unfold step
  split
  · exact List.Sublist.refl _
  · split
    · split
      · exact List.Sublist.refl _
      · exact List.Sublist.cons _ (List.Sublist.refl _)
    · exact List.Sublist.refl _

theorem scan_retains (rules : List (Rule α)) (seed known : List α) :
    known.Sublist (scan rules seed known) := by
  induction rules generalizing known with
  | nil => exact List.Sublist.refl _
  | cons rule rest ih =>
    exact (step_retains rule seed known).trans (ih (step rule seed known))

theorem step_nodup (rule : Rule α) (seed known : List α)
    (unique : known.Nodup) : (step rule seed known).Nodup := by
  unfold step
  split
  · exact unique
  · split
    · split
      · exact unique
      · exact List.nodup_cons.mpr ⟨by assumption, unique⟩
    · exact unique

theorem scan_nodup (rules : List (Rule α)) (seed known : List α)
    (unique : known.Nodup) : (scan rules seed known).Nodup := by
  induction rules generalizing known with
  | nil => exact unique
  | cons rule rest ih => exact ih _ (step_nodup rule seed known unique)

/-- A single step cannot invent an atom outside the old set and its own head. -/
theorem step_origin (rule : Rule α) (seed known : List α) (atom : α)
    (present : atom ∈ step rule seed known) :
    atom ∈ known ∨ rule.head = some atom := by
  unfold step at present
  split at present
  · exact Or.inl present
  · rename_i head equation
    split at present
    · split at present
      · exact Or.inl present
      · rcases List.mem_cons.mp present with equal | old
        · exact Or.inr (equal ▸ equation)
        · exact Or.inl old
    · exact Or.inl present

/-- Every scan result comes from its initial truth or the finite head list. -/
theorem scan_origin (rules : List (Rule α)) (seed known : List α) (atom : α)
    (present : atom ∈ scan rules seed known) :
    atom ∈ known ∨ atom ∈ heads rules := by
  induction rules generalizing known with
  | nil => exact Or.inl present
  | cons rule rest ih =>
    rcases ih (step rule seed known) present with current | later
    · rcases step_origin rule seed known atom current with old | head
      · exact Or.inl old
      · exact Or.inr (List.mem_filterMap.mpr ⟨rule, List.mem_cons_self, head⟩)
    · obtain ⟨source, member, head⟩ := List.mem_filterMap.mp later
      exact Or.inr (List.mem_filterMap.mpr ⟨source, List.mem_cons_of_mem _ member, head⟩)

/-- A justified insertion stays inside the semantic least reduct closure. -/
theorem step_sound (rules : List (Rule α)) (rule : Rule α) (seed known : List α)
    (member : rule ∈ rules)
    (sound : Sub (atoms known) (Semantics.Gamma (program rules) (atoms seed))) :
    Sub (atoms (step rule seed known)) (Semantics.Gamma (program rules) (atoms seed)) := by
  intro atom present
  unfold step at present
  split at present
  · exact sound atom present
  · rename_i head equation
    split at present
    · rename_i enabled
      have selected : gates rule seed = true ∧ body rule known = true := by
        simpa only [Bool.and_eq_true] using enabled
      have gate := (gates_exact rule seed).mp selected.1
      have positive := (body_exact rule known).mp selected.2
      split at present
      · exact sound atom present
      · rcases List.mem_cons.mp present with equal | old
        · subst atom
          exact Semantics.gamma_closed (program rules) (atoms seed) head
            ⟨rule.denote, List.mem_map.mpr ⟨rule, member, rfl⟩, equation,
              True.intro, gate, Semantics.body_mono sound positive⟩
        · exact sound atom old
    · exact sound atom present

/-- Soundness holds for every scan prefix, even before any fixed point. -/
theorem scan_sound (rules visited : List (Rule α)) (seed known : List α)
    (covered : ∀ rule ∈ visited, rule ∈ rules)
    (sound : Sub (atoms known) (Semantics.Gamma (program rules) (atoms seed))) :
    Sub (atoms (scan visited seed known)) (Semantics.Gamma (program rules) (atoms seed)) := by
  induction visited generalizing known with
  | nil => exact sound
  | cons rule rest ih =>
    have after_rule := step_sound rules rule seed known (covered rule List.mem_cons_self) sound
    exact ih (step rule seed known) (fun item member => covered item (List.mem_cons_of_mem _ member))
      after_rule

theorem step_derives (rule : Rule α) (seed known : List α) (head : α)
    (has_head : rule.head = some head) (gate : gates rule seed = true)
    (positive : body rule known = true) : head ∈ step rule seed known := by
  by_cases present : head ∈ known
  · simp [step, has_head, gate, positive, present]
  · simp [step, has_head, gate, positive, present]

/-- Every rule enabled already at the start has its head present after the
    complete scan. Earlier insertions cannot invalidate a positive body. -/
theorem scan_derives (rules : List (Rule α)) (seed known : List α)
    (rule : Rule α) (head : α) (member : rule ∈ rules)
    (has_head : rule.head = some head) (gate : gates rule seed = true)
    (positive : Semantics.Body rule.denote (atoms known)) :
    head ∈ scan rules seed known := by
  induction rules generalizing known with
  | nil => exact False.elim (List.not_mem_nil member)
  | cons first rest ih =>
    rcases List.mem_cons.mp member with equal | later
    · subst rule
      have derived := step_derives first seed known head has_head gate
        ((body_exact first known).mpr positive)
      exact (scan_retains rest seed (step first seed known)).subset derived
    · have retained : Sub (atoms known) (atoms (step first seed known)) :=
        fun _ present => (step_retains first seed known).subset present
      exact ih (step first seed known) later (Semantics.body_mono retained positive)

/-- An unchanged full scan establishes closedness; checking one rule or a
    truncated scan would not supply this conclusion. -/
theorem unchanged_closed (rules : List (Rule α)) (seed known : List α)
    (unchanged : scan rules seed known = known) :
    Closed (Semantics.Consequence (program rules) (atoms seed)) (atoms known) := by
  intro atom consequence
  obtain ⟨source, member, has_head, _, gate, positive⟩ := consequence
  obtain ⟨rule, in_rules, same⟩ := List.mem_map.mp member
  subst source
  have derived := scan_derives rules seed known rule atom in_rules has_head
    ((gates_exact rule seed).mpr gate) positive
  exact unchanged ▸ derived

/-- Zero fuel is unfinished, including on an empty program. A successful return
    has observed an unchanged complete scan. Fuel counts scans only. -/
def iterate (rules : List (Rule α)) (seed : List α) : Nat → List α → Option (List α)
  | 0, _ => none
  | fuel + 1, known =>
      let next := scan rules seed known
      if next = known then some known else iterate rules seed fuel next

/-- A completed execution returns a closed, sound set when its initial set was
    sound. Neither property is postulated as an implementation agreement. -/
theorem iterate_exact (rules : List (Rule α)) (seed : List α) (fuel : Nat)
    (known result : List α)
    (sound : Sub (atoms known) (Semantics.Gamma (program rules) (atoms seed)))
    (completed : iterate rules seed fuel known = some result) :
    atoms result = Semantics.Gamma (program rules) (atoms seed) := by
  induction fuel generalizing known with
  | zero => simp [iterate] at completed
  | succ fuel ih =>
    simp only [iterate] at completed
    split at completed
    · rename_i unchanged
      have same : known = result := Option.some.inj completed
      subst result
      exact exact_of_sound_and_closed sound (unchanged_closed rules seed known unchanged)
    · exact ih (scan rules seed known)
        (scan_sound rules rules seed known (fun _ member => member) sound) completed

/-- Each changing scan strictly increases the retained list. -/
theorem changing_scan_grows (rules : List (Rule α)) (seed known : List α)
    (changed : scan rules seed known ≠ known) : known.length < (scan rules seed known).length := by
  have retained := scan_retains rules seed known
  have unequal : known.length ≠ (scan rules seed known).length := by
    intro equal
    exact changed (retained.eq_of_length equal).symm
  exact Nat.lt_of_le_of_ne retained.length_le unequal

/-- A conservative constructive bound uses the head-list length, including
    duplicate heads. Enough remaining scans always reaches a completed closure. -/
theorem iterate_completes (rules : List (Rule α)) (seed : List α) (fuel : Nat)
    (known : List α) (unique : known.Nodup)
    (covered : ∀ atom ∈ known, atom ∈ heads rules)
    (enough : (heads rules).length < known.length + fuel) :
    ∃ result, iterate rules seed fuel known = some result := by
  induction fuel generalizing known with
  | zero =>
    have bound := unique.length_le_of_subset covered
    simp only [Nat.add_zero] at enough
    omega
  | succ fuel ih =>
    by_cases unchanged : scan rules seed known = known
    · exact ⟨known, by simp [iterate, unchanged]⟩
    · have growth := changing_scan_grows rules seed known unchanged
      have next_unique := scan_nodup rules seed known unique
      have next_covered : ∀ atom ∈ scan rules seed known, atom ∈ heads rules := by
        intro atom present
        rcases scan_origin rules seed known atom present with old | head
        · exact covered atom old
        · exact head
      have next_enough : (heads rules).length < (scan rules seed known).length + fuel := by omega
      obtain ⟨result, completed⟩ := ih (scan rules seed known) next_unique next_covered next_enough
      exact ⟨result, by simpa [iterate, unchanged] using completed⟩

/-- Starting from empty positive truth needs at most one more scan than the
    number of listed heads. The frozen seed is never copied into that truth. -/
theorem empty_completes (rules : List (Rule α)) (seed : List α) :
    ∃ result, iterate rules seed ((heads rules).length + 1) [] = some result := by
  apply iterate_completes rules seed _ []
  · exact List.nodup_nil
  · intro _ impossible
    exact False.elim (List.not_mem_nil impossible)
  · simp

/-- The proved head-list bound makes the unfinished branch impossible. This
    accessor extracts that computed result; no choice of a semantic model occurs. -/
def closure (rules : List (Rule α)) (seed : List α) : List α :=
  (iterate rules seed ((heads rules).length + 1) []).get (by
    obtain ⟨result, completed⟩ := empty_completes rules seed
    simp [completed])

theorem closure_completed (rules : List (Rule α)) (seed : List α) :
    iterate rules seed ((heads rules).length + 1) [] = some (closure rules seed) := by
  exact (Option.some_get _).symm

/-- Executable sequential closure is exactly the independently defined least
    consequence set. It requires no assumption about the algorithm's result. -/
theorem closure_exact (rules : List (Rule α)) (seed : List α) :
    atoms (closure rules seed) = Semantics.Gamma (program rules) (atoms seed) := by
  apply iterate_exact rules seed _ [] _ _ (closure_completed rules seed)
  intro _ impossible
  exact False.elim (List.not_mem_nil impossible)

/-- Check constraints only after the positive closure has completed. -/
def constraints (rules : List (Rule α)) (seed known : List α) : Bool :=
  rules.all (fun rule =>
    if rule.head = none then !(gates rule seed && body rule known) else true)

/-- The finite Boolean constraint test reads exactly the original normalized
    constraints under the fixed seed. It neither truncates nor alters closure. -/
theorem constraints_exact (rules : List (Rule α)) (seed known : List α) :
    constraints rules seed known = true ↔
      Semantics.ConstraintsOK (program rules) (atoms seed) (atoms known) := by
  constructor
  · intro checked source member no_head _ gate positive
    obtain ⟨rule, in_rules, same⟩ := List.mem_map.mp member
    subst source
    have selected := (List.all_eq_true.mp checked) rule in_rules
    have no_head : rule.head = none := no_head
    have gate_true := (gates_exact rule seed).mpr gate
    have body_true := (body_exact rule known).mpr positive
    simp [no_head, gate_true, body_true] at selected
  · intro sound
    apply List.all_eq_true.mpr
    intro rule member
    by_cases no_head : rule.head = none
    · have cannot_fire : ¬ (gates rule seed = true ∧ body rule known = true) := by
        intro enabled
        exact sound rule.denote (List.mem_map.mpr ⟨rule, member, rfl⟩) no_head True.intro
          ((gates_exact rule seed).mp enabled.1) ((body_exact rule known).mp enabled.2)
      cases gate : gates rule seed <;> cases positive : body rule known <;>
        simp_all
    · simp [no_head]

/-- Finite agreement is two inclusions, so an extra seed atom outside the
    carrier is rejected too. Neither list must be duplicate free. -/
def agrees (carrier seed known : List α) : Bool :=
  known.all (fun atom => if atom ∈ carrier then decide (atom ∈ seed) else true) &&
    seed.all (fun atom => decide (atom ∈ known ∧ atom ∈ carrier))

theorem agrees_exact (carrier seed known : List α) :
    agrees carrier seed known = true ↔ Inter (atoms known) (atoms carrier) = atoms seed := by
  constructor
  · intro checked
    have both :
        known.all (fun atom => if atom ∈ carrier then decide (atom ∈ seed) else true) = true ∧
          seed.all (fun atom => decide (atom ∈ known ∧ atom ∈ carrier)) = true := by
      simpa only [agrees, Bool.and_eq_true] using checked
    apply atoms_ext
    intro atom
    constructor
    · intro ⟨derived, inside⟩
      have selected := List.all_eq_true.mp both.1 atom derived
      have member : atom ∈ carrier := inside
      change atom ∈ seed
      simpa only [if_pos member, decide_eq_true_eq] using selected
    · intro selected
      have derived := List.all_eq_true.mp both.2 atom selected
      change atom ∈ known ∧ atom ∈ carrier
      exact of_decide_eq_true derived
  · intro same
    simp only [agrees, Bool.and_eq_true]
    constructor
    · apply List.all_eq_true.mpr
      intro atom derived
      by_cases inside : atom ∈ carrier
      · have selected : atom ∈ seed := by
          have source : Inter (atoms known) (atoms carrier) atom := ⟨derived, inside⟩
          change atoms seed atom
          rw [← same]
          exact source
        simp [inside, selected]
      · simp [inside]
    · apply List.all_eq_true.mpr
      intro atom selected
      apply decide_eq_true
      change Inter (atoms known) (atoms carrier) atom
      rw [same]
      exact selected

/-- Decide normalized seed acceptance using the computed least closure.
    This is a total finite algorithm, with no claim about machine resource limits. -/
def accepts (rules : List (Rule α)) (carrier seed : List α) : Bool :=
  let known := closure rules seed
  agrees carrier seed known && constraints rules seed known

/-- The executable decision is precisely the existing seed-acceptance predicate.
    In particular, rejection of a mismatching seed is not by itself a claim that
    its computed closure is an unstable interpretation under another seed. -/
theorem accepts_exact (rules : List (Rule α)) (carrier seed : List α) :
    accepts rules carrier seed = true ↔
      Semantics.Accept (program rules) (atoms carrier) (atoms seed) := by
  simp only [accepts, Bool.and_eq_true, agrees_exact, constraints_exact, closure_exact,
    Semantics.Accept]

/-- Collect every atom that a frozen gate can read. Repetitions are harmless. -/
def carrier (rules : List (Rule α)) : List α :=
  rules.flatMap (fun rule => rule.gateTrue ++ rule.gateFalse)

omit [DecidableEq α] in
/-- Gate coverage follows from the computed carrier, not a caller's assertion. -/
theorem carrier_covers (rules : List (Rule α)) :
    Semantics.GateCarrier (program rules) (atoms (carrier rules)) := by
  intro source member
  obtain ⟨rule, in_rules, same⟩ := List.mem_map.mp member
  subst source
  constructor
  · intro atom present
    exact List.mem_flatMap.mpr ⟨rule, in_rules, List.mem_append.mpr (Or.inl present)⟩
  · intro atom present
    exact List.mem_flatMap.mpr ⟨rule, in_rules, List.mem_append.mpr (Or.inr present)⟩

/-- Acceptance reconstructs an answer set of the normalized program. The seed
    itself is only a projection; the output is the full least closure. -/
theorem accepted_stable (rules : List (Rule α)) (seed : List α)
    (accepted : accepts rules (carrier rules) seed = true) :
    Semantics.Stable (program rules) (atoms (closure rules seed)) := by
  rw [closure_exact]
  exact Semantics.accept_sound (carrier_covers rules)
    ((accepts_exact rules (carrier rules) seed).mp accepted)

/-- Every answer set has a finite accepted seed and is reconstructed exactly.
    Only the proof selects its projection from a supplied mathematical model;
    candidate enumeration is a separate algorithmic obligation. -/
theorem stable_has_seed (rules : List (Rule α)) (model : Atoms α)
    (stable : Semantics.Stable (program rules) model) :
    ∃ seed, accepts rules (carrier rules) seed = true ∧ atoms (closure rules seed) = model := by
  classical
  let seed := (carrier rules).filter (fun atom => decide (model atom))
  have projection : atoms seed = Inter model (atoms (carrier rules)) := by
    apply atoms_ext
    intro atom
    simp only [atoms, seed, List.mem_filter, decide_eq_true_eq, Inter]
    exact and_comm
  have semantic := Semantics.stable_complete (carrier_covers rules) stable
  have reconstructed : atoms (closure rules seed) = model := by
    rw [closure_exact, projection]
    exact semantic.2
  have accepted : accepts rules (carrier rules) seed = true := by
    apply (accepts_exact rules (carrier rules) seed).mpr
    rw [projection]
    exact semantic.1
  exact ⟨seed, accepted, reconstructed⟩

/-- The finite executable closure test characterizes the existing answer sets.
    It does not yet enumerate those seeds or certify an external implementation. -/
theorem stable_iff_accepted_seed (rules : List (Rule α)) (model : Atoms α) :
    Semantics.Stable (program rules) model ↔
      ∃ seed, accepts rules (carrier rules) seed = true ∧ atoms (closure rules seed) = model := by
  constructor
  · exact stable_has_seed rules model
  · rintro ⟨seed, accepted, same⟩
    exact same ▸ accepted_stable rules seed accepted

/-- Positive cycles without external support derive no atoms. -/
theorem unsupported_cycle_stays_empty :
    closure ([⟨some 0, [1], [], []⟩, ⟨some 1, [0], [], []⟩] : List (Rule Nat)) [] = [] := by
  decide

/-- A true frozen gate enables its rule but is not itself an initial consequence. -/
theorem gates_do_not_seed_truth :
    closure ([⟨some 1, [], [0], []⟩] : List (Rule Nat)) [0] = [1] := by
  decide

/-- An empty-body constraint rejects the empty interpretation. -/
theorem empty_constraint_rejects :
    accepts ([⟨none, [], [], []⟩] : List (Rule Nat)) [] [] = false := by
  decide

/-- No scans means no completed result, even when the program is empty. -/
theorem zero_fuel_unfinished : iterate ([] : List (Rule Nat)) [] 0 [] = none := rfl

end Zetesis.FiniteClosure
