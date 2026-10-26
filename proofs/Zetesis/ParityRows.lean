import Zetesis.EntailedPruning
import Zetesis.DisjunctiveSupport

/-!
# Parity rows over GF(2)

An interpretation restricted to a list of atoms is a vector over the two-element
field GF(2): each atom contributes its truth value. A parity row requires the
exclusive-or of some of those values to equal a constant, an affine equation over
GF(2) in which exclusive-or is addition and conjunction multiplication. A system
of such rows has an affine solution set, and adding one row to another preserves
it. Elimination over GF(2) decides such a system in polynomial time, where
resolution needs exponentially many steps on some of them.

Two representations meet here. An atom list's `parity` counts occurrences modulo
two, so neither the list's order nor a pair of repeated atoms matters
(`parity_perm`, `parity_cancel`). A mask over a coordinate list is the bit row of
an implementation: it selects the atoms of its true coordinates, its row's value
is the parity of that selection (`parity_select`), and two masks add by
coordinatewise exclusive-or, in which repeated atoms cancel.

The laws:
- `combination_sound`: an interpretation satisfying every row of a list satisfies
  their sum, so row operations preserve the solution set;
- `region_refutes` and `region_forces`: a row whose selected atoms a region
  decides refutes the region when the held atoms' parity differs from the row's
  constant, and a row with one undecided atom of odd multiplicity forces it;
- `affine_group_sound`: rows recovered from a group of constraints over a few
  atoms hold in every model of the group, given the certificate that every
  pattern the group allows satisfies them;
- `completion_holds` and `completion_xor_sound`: an atom whose only producers are
  rules with it as their sole head is true in an answer set exactly when one of
  their bodies holds, so a two-rule exclusive-or definition is a parity row of
  every answer set.

Rows from a constraint group hold in every model of the group, and so in every
answer set (`EntailedPruning`); rows from a completion hold in every answer set
by necessary support. Either way a refuted region contains no answer set.

These laws do not establish which groups or definitions a recognizer accepts,
the elimination that finds a certificate, bit packing, word operations,
coordinate assignment, resource accounting or the narrower's use of a
consequence. A recognizer must establish that each constraint of a group
excludes exactly the interpretations showing its forbidden pattern, and that the
rows it records are the rows the certificate checks.
-/

namespace Zetesis.ParityRows

open Ferraris DisjunctiveSupport

universe u
variable {α : Type u}

open Classical in
/-- An atom's truth value in an interpretation, as a bit. -/
noncomputable def truth (M : Atoms α) (a : α) : Bool := decide (M a)

theorem truth_eq_true {M : Atoms α} {a : α} : truth M a = true ↔ M a := by
  classical
  exact decide_eq_true_iff

theorem truth_eq_false {M : Atoms α} {a : α} : truth M a = false ↔ ¬ M a := by
  classical
  exact decide_eq_false_iff_not

/-- The parity of the listed atoms' truth in an interpretation: `true` when an
odd number of the list's positions hold. Lists count occurrences. -/
noncomputable def parity (M : Atoms α) : List α → Bool
  | [] => false
  | a :: rest => truth M a ^^ parity M rest

/-- Parity adds over concatenation. -/
theorem parity_append (M : Atoms α) (xs ys : List α) :
    parity M (xs ++ ys) = (parity M xs ^^ parity M ys) := by
  induction xs with
  | nil => simp [parity]
  | cons a rest ih => simp [parity, ih]

/-- Two adjacent occurrences of one atom cancel. -/
theorem parity_cancel (M : Atoms α) (a : α) (rest : List α) :
    parity M (a :: a :: rest) = parity M rest := by
  simp [parity]

/-- Parity does not depend on the order of the list. -/
theorem parity_perm (M : Atoms α) {xs ys : List α} (same : xs.Perm ys) :
    parity M xs = parity M ys := by
  induction same with
  | nil => rfl
  | cons a _ ih => simp [parity, ih]
  | swap a b rest => simp [parity, Bool.xor_left_comm]
  | trans _ _ first second => exact first.trans second

/-- An odd number of copies of one atom has that atom's truth as its parity. -/
theorem parity_replicate_odd (M : Atoms α) (a : α) :
    ∀ n, n % 2 = 1 → parity M (List.replicate n a) = truth M a
  | 0, odd => by simp at odd
  | 1, _ => by simp [parity]
  | n + 2, odd => by
    have smaller : n % 2 = 1 := by omega
    rw [List.replicate_succ, List.replicate_succ, parity_cancel]
    exact parity_replicate_odd M a n smaller

/-- The truth values of a coordinate list's atoms: the interpretation as a
vector over GF(2). -/
noncomputable def value (M : Atoms α) (S : List α) : List Bool := S.map (truth M)

theorem value_length (M : Atoms α) (S : List α) : (value M S).length = S.length := by
  simp [value]

/-- The dot product over GF(2) of a vector and a mask. Positions past the shorter
list do not count. -/
def dot : List Bool → List Bool → Bool
  | x :: xs, m :: ms => (x && m) ^^ dot xs ms
  | _, _ => false

/-- The atoms at a mask's true coordinates. -/
def select : List α → List Bool → List α
  | a :: S, true :: m => a :: select S m
  | _ :: S, false :: m => select S m
  | _, _ => []

/-- A mask's dot product with an interpretation's vector is the parity of the
atoms it selects. -/
theorem parity_select (M : Atoms α) : ∀ (S : List α) (m : List Bool),
    parity M (select S m) = dot (value M S) m
  | [], m => by cases m <;> rfl
  | _ :: _, [] => rfl
  | a :: S, true :: m => by
    simp [select, value, dot, parity, parity_select M S m]
  | a :: S, false :: m => by
    simp [select, value, dot, parity_select M S m]

/-- A parity row over a coordinate list: a mask, and the parity its selection
must have. -/
abbrev Row := List Bool × Bool

/-- An interpretation satisfies a row when the atoms its mask selects have the
row's parity. -/
def Holds (M : Atoms α) (S : List α) (r : Row) : Prop :=
  parity M (select S r.1) = r.2

theorem holds_iff_dot (M : Atoms α) (S : List α) (r : Row) :
    Holds M S r ↔ dot (value M S) r.1 = r.2 := by
  rw [Holds, parity_select]

/-- The sum of two rows: the coordinatewise exclusive-or of their masks, and the
exclusive-or of their constants. -/
def add (r s : Row) : Row := (List.zipWith (· ^^ ·) r.1 s.1, r.2 ^^ s.2)

/-- The zero row over `n` coordinates. -/
def zero (n : Nat) : Row := (List.replicate n false, false)

/-- The sum of a list of rows over `n` coordinates. -/
def sum (n : Nat) (rows : List Row) : Row := rows.foldr add (zero n)

/-- A mask of zeros selects nothing. -/
theorem dot_zero : ∀ (x : List Bool) (n : Nat), dot x (List.replicate n false) = false
  | [], _ => rfl
  | _ :: _, 0 => rfl
  | _ :: xs, n + 1 => by simp [dot, List.replicate_succ, dot_zero xs n]

/-- The dot product distributes over the sum of two masks of equal length. -/
theorem dot_add : ∀ (x m₁ m₂ : List Bool), m₁.length = m₂.length →
    dot x (List.zipWith (· ^^ ·) m₁ m₂) = (dot x m₁ ^^ dot x m₂)
  | [], _, _, _ => rfl
  | _ :: _, [], [], _ => rfl
  | _ :: _, [], _ :: _, lengths => by simp at lengths
  | _ :: _, _ :: _, [], lengths => by simp at lengths
  | x :: xs, a :: m₁, b :: m₂, lengths => by
    have tail : m₁.length = m₂.length := by simpa using lengths
    have distributes := dot_add xs m₁ m₂ tail
    simp only [List.zipWith_cons_cons, dot, distributes, Bool.and_xor_distrib_left]
    cases x <;> cases a <;> cases b <;> simp

/-- Every row of a list over `n` coordinates keeps the sum's mask at `n`
coordinates. -/
theorem sum_length (n : Nat) (rows : List Row)
    (lengths : ∀ r ∈ rows, r.1.length = n) : (sum n rows).1.length = n := by
  induction rows with
  | nil => simp [sum, zero]
  | cons r rest ih =>
    have head := lengths r (by simp)
    have tail := ih (fun s member => lengths s (by simp [member]))
    change (List.zipWith (· ^^ ·) r.1 (sum n rest).1).length = n
    simp [List.length_zipWith, head, tail]

/-- Row operations preserve the solution set: an interpretation satisfying every
row of a list satisfies their sum.

Proof. By induction on the list. The empty sum selects nothing and has constant
`false`. A further row adds its mask and its constant, and the dot product adds
with them (`dot_add`). -/
theorem combination_sound (M : Atoms α) (S : List α) (rows : List Row)
    (lengths : ∀ r ∈ rows, r.1.length = S.length)
    (satisfied : ∀ r ∈ rows, Holds M S r) : Holds M S (sum S.length rows) := by
  induction rows with
  | nil =>
    rw [holds_iff_dot]
    exact dot_zero (value M S) S.length
  | cons r rest ih =>
    have restLengths : ∀ s ∈ rest, s.1.length = S.length :=
      fun s member => lengths s (by simp [member])
    have restHolds : Holds M S (sum S.length rest) :=
      ih restLengths (fun s member => satisfied s (by simp [member]))
    have headHolds : Holds M S r := satisfied r (by simp)
    have aligned : r.1.length = (sum S.length rest).1.length := by
      rw [lengths r (by simp), sum_length S.length rest restLengths]
    rw [holds_iff_dot] at restHolds headHolds ⊢
    change dot (value M S) (List.zipWith (· ^^ ·) r.1 (sum S.length rest).1) =
      (r.2 ^^ (sum S.length rest).2)
    rw [dot_add _ _ _ aligned, headHolds, restHolds]

/-- A region decides an atom when it holds the atom or has cut it. -/
def Decided (c : Cube α) (a : α) : Prop := c.lower a ∨ ¬ c.upper a

/-- In a region, a decided atom has the value the region's held atoms give it. -/
theorem truth_of_decided {c : Cube α} {z : Atoms α} (inside : c.Contains z)
    {a : α} (decided : Decided c a) : truth z a = truth c.lower a := by
  rcases decided with held | cut
  · rw [truth_eq_true.mpr held, truth_eq_true.mpr (inside.1 a held)]
  · have absent : ¬ z a := fun present => cut (inside.2 a present)
    have notHeld : ¬ c.lower a := fun held => absent (inside.1 a held)
    rw [truth_eq_false.mpr notHeld, truth_eq_false.mpr absent]

/-- In a region, a list of decided atoms has the parity of its held atoms. -/
theorem parity_of_decided {c : Cube α} {z : Atoms α} (inside : c.Contains z) :
    ∀ xs : List α, (∀ a ∈ xs, Decided c a) → parity z xs = parity c.lower xs
  | [], _ => rfl
  | a :: rest, decided => by
    have head := truth_of_decided inside (decided a (by simp))
    have tail := parity_of_decided inside rest (fun b member => decided b (by simp [member]))
    simp [parity, head, tail]

/-- A row whose selected atoms the region decides, with the held atoms' parity
different from the row's constant, refutes the region: no interpretation in it
satisfies the row. Applied to the sum of a certificate's rows
(`combination_sound`), this is elimination's refutation of a region.

Proof. Inside the region every selected atom has its held value, so the row's
parity is the held atoms' parity, which differs from the constant. -/
theorem region_refutes (S : List α) (r : Row) (c : Cube α)
    (decided : ∀ a ∈ select S r.1, Decided c a)
    (wrong : parity c.lower (select S r.1) ≠ r.2) :
    ∀ z, c.Contains z → ¬ Holds z S r := by
  intro z inside satisfied
  have held : parity z (select S r.1) = parity c.lower (select S r.1) :=
    parity_of_decided inside _ decided
  exact wrong (held.symm.trans satisfied)

/-- A list's parity is the parity of one atom's occurrences in it plus that of the
rest. -/
theorem parity_split [DecidableEq α] (M : Atoms α) (a : α) (xs : List α) :
    parity M xs =
      (parity M (xs.filter (fun b => decide (b = a))) ^^
        parity M (xs.filter (fun b => !decide (b = a)))) := by
  rw [← parity_append]
  exact (parity_perm M (List.filter_append_perm (fun b => decide (b = a)) xs)).symm

/-- A row with exactly one undecided atom, occurring an odd number of times among
its selected atoms, forces that atom in the region: every interpretation in the
region satisfying the row gives the atom the value the row's constant and the
other atoms' held values leave.

Proof. Split the selection into the atom's occurrences and the rest. An odd
number of copies has the atom's own parity (`parity_replicate_odd`); the rest is
decided, so it has its held parity. The row's constant fixes their sum. -/
theorem region_forces [DecidableEq α] (S : List α) (r : Row) (c : Cube α) (a : α)
    (decided : ∀ b ∈ select S r.1, b ≠ a → Decided c b)
    (odd : (select S r.1).count a % 2 = 1) :
    ∀ z, c.Contains z → Holds z S r →
      (z a ↔ (r.2 ^^ parity c.lower ((select S r.1).filter (fun b => !decide (b = a)))) = true) := by
  intro z inside satisfied
  have occurrences :
      parity z ((select S r.1).filter (fun b => decide (b = a))) = truth z a := by
    rw [List.filter_eq]
    exact parity_replicate_odd z a _ odd
  have othersDecided :
      ∀ b ∈ (select S r.1).filter (fun b => !decide (b = a)), Decided c b := by
    intro b member
    have both := List.mem_filter.mp member
    exact decided b both.1 (by simpa using both.2)
  have othersHeld :
      parity z ((select S r.1).filter (fun b => !decide (b = a))) =
        parity c.lower ((select S r.1).filter (fun b => !decide (b = a))) :=
    parity_of_decided inside _ othersDecided
  have total :
      (truth z a ^^ parity c.lower ((select S r.1).filter (fun b => !decide (b = a)))) =
        r.2 := by
    have split := parity_split z a (select S r.1)
    rw [occurrences, othersHeld] at split
    exact split.symm.trans satisfied
  have solved :
      truth z a = (r.2 ^^ parity c.lower ((select S r.1).filter (fun b => !decide (b = a)))) := by
    rw [← total, Bool.xor_assoc, Bool.xor_self, Bool.xor_false]
  rw [← solved]
  exact truth_eq_true.symm

/-- The literal giving an atom a value: the atom itself for `true`, its default
negation for `false`. -/
def literal (a : α) : Bool → Formula α
  | true => .atom a
  | false => Ferraris.Neg (.atom a)

/-- The conjunction of a pattern's literals over a coordinate list; the empty
conjunction is truth. -/
def pattern : List α → List Bool → Formula α
  | a :: S, v :: π => .conj (literal a v) (pattern S π)
  | _, _ => .imp .bot .bot

/-- The integrity constraint forbidding a pattern: the negation of its
conjunction. -/
def forbidding (S : List α) (π : List Bool) : Formula α := Ferraris.Neg (pattern S π)

theorem satisfies_literal (M : Atoms α) (a : α) :
    ∀ v, Satisfies M (literal a v) ↔ truth M a = v
  | true => by simp [literal, Satisfies, truth_eq_true]
  | false => by simp [literal, Ferraris.Neg, Satisfies, truth_eq_false]

/-- An interpretation satisfies a pattern's conjunction exactly when it shows that
pattern on the coordinates. -/
theorem satisfies_pattern (M : Atoms α) : ∀ (S : List α) (π : List Bool),
    π.length = S.length → (Satisfies M (pattern S π) ↔ value M S = π)
  | [], [], _ => by simp [pattern, Satisfies, value]
  | [], _ :: _, lengths => by simp at lengths
  | _ :: _, [], lengths => by simp at lengths
  | a :: S, v :: π, lengths => by
    have tail : π.length = S.length := by simpa using lengths
    have rest := satisfies_pattern M S π tail
    simp only [pattern, Satisfies, satisfies_literal, rest, value, List.map_cons,
      List.cons.injEq]

/-- The constraint forbidding a pattern excludes exactly the interpretations that
show it. -/
theorem satisfies_forbidding (M : Atoms α) (S : List α) (π : List Bool)
    (lengths : π.length = S.length) :
    Satisfies M (forbidding S π) ↔ value M S ≠ π := by
  simp [forbidding, Ferraris.Neg, Satisfies, satisfies_pattern M S π lengths]

/-- Rows recovered from a constraint group hold in every model of the group,
given the certificate that every pattern the group allows satisfies them.

The group excludes each forbidden pattern by some constraint; an interpretation
modelling the group therefore shows an allowed pattern on the group's atoms, and
the certificate covers every allowed pattern. The checker enumerates the
patterns of the group's few atoms. When the allowed patterns form an affine
subspace, the recovered rows are its defining equations; the law needs only the
certificate. -/
theorem affine_group_sound (S : List α) (group : Theory α)
    (forbidden : List (List Bool)) (rows : List Row)
    (excludes : ∀ π ∈ forbidden, ∃ F ∈ group, ∀ M, Satisfies M F → value M S ≠ π)
    (certificate : ∀ x : List Bool, x.length = S.length → x ∉ forbidden →
      ∀ r ∈ rows, dot x r.1 = r.2)
    {M : Atoms α} (model : Models M group) : ∀ r ∈ rows, Holds M S r := by
  have allowed : value M S ∉ forbidden := by
    intro shown
    obtain ⟨F, member, excluded⟩ := excludes (value M S) shown
    exact excluded M (model F member) rfl
  intro r listed
  rw [holds_iff_dot]
  exact certificate (value M S) (value_length M S) allowed r listed

/-- The canonical group of a list of forbidden patterns: one forbidding
constraint each. Its rows hold in every model of it under the same
certificate. -/
theorem forbidding_group_sound (S : List α) (forbidden : List (List Bool))
    (rows : List Row) (aligned : ∀ π ∈ forbidden, π.length = S.length)
    (certificate : ∀ x : List Bool, x.length = S.length → x ∉ forbidden →
      ∀ r ∈ rows, dot x r.1 = r.2)
    {M : Atoms α} (model : Models M (forbidden.map (forbidding S))) :
    ∀ r ∈ rows, Holds M S r := by
  refine affine_group_sound S (forbidden.map (forbidding S)) forbidden rows ?_
    certificate model
  intro π member
  refine ⟨forbidding S π, List.mem_map.mpr ⟨π, member, rfl⟩, ?_⟩
  intro N satisfied
  exact (satisfies_forbidding N S π (aligned π member)).mp satisfied

/-- In an answer set, an atom whose only producers are rules with that atom as
their sole head is true exactly when one of their bodies holds: its completion.

Proof. A body that holds makes its rule's head true, since an answer set models
its theory. Conversely a true atom of an answer set has a supporting rule
(`DisjunctiveSupport.answer_set_supported`); every rule whose head contains the
atom is one of the listed producers, and a supporting rule is enabled. -/
theorem completion_holds (M : Atoms α) (T : Theory α) (rules : List (Producer α))
    (covered : Covered T rules) (original : ∀ r ∈ rules, r.formula ∈ T)
    (p : α) (bodies : List (Formula α))
    (producers : ∀ B ∈ bodies, Producer.rule B (.atom p) ∈ rules)
    (sole : ∀ r ∈ rules, r.head.Contains p → ∃ B ∈ bodies, r = .rule B (.atom p))
    (answer : Stable M T) :
    M p ↔ ∃ B ∈ bodies, Satisfies M B := by
  constructor
  · intro present
    obtain ⟨r, inRules, supports⟩ :=
      answer_set_supported M T rules covered original answer p present
    obtain ⟨B, listed, rfl⟩ := sole r inRules supports.2.1
    exact ⟨B, listed, supports.1⟩
  · rintro ⟨B, listed, enabled⟩
    have asserted : Satisfies M (Producer.rule B (.atom p)).formula :=
      answer.1 _ (original _ (producers B listed))
    exact asserted enabled

/-- The two-rule exclusive-or definition `p :- a, not b. p :- not a, b.`, with
no other producer of `p`, holds in every answer set as the parity row
`p xor a xor b = 0`.

Proof. By `completion_holds`, `p` holds exactly when one of the two bodies does,
that is, exactly when `a` and `b` differ. -/
theorem completion_xor_sound (M : Atoms α) (T : Theory α)
    (rules : List (Producer α)) (covered : Covered T rules)
    (original : ∀ r ∈ rules, r.formula ∈ T) (p a b : α)
    (first : Producer.rule (.conj (.atom a) (Ferraris.Neg (.atom b))) (.atom p) ∈ rules)
    (second : Producer.rule (.conj (Ferraris.Neg (.atom a)) (.atom b)) (.atom p) ∈ rules)
    (sole : ∀ r ∈ rules, r.head.Contains p →
      r = .rule (.conj (.atom a) (Ferraris.Neg (.atom b))) (.atom p) ∨
        r = .rule (.conj (Ferraris.Neg (.atom a)) (.atom b)) (.atom p))
    (answer : Stable M T) : parity M [p, a, b] = false := by
  have completion := completion_holds M T rules covered original p
    [.conj (.atom a) (Ferraris.Neg (.atom b)), .conj (Ferraris.Neg (.atom a)) (.atom b)]
    (by simp [first, second])
    (fun r member contains => by
      rcases sole r member contains with same | same
      · exact ⟨_, by simp, same⟩
      · exact ⟨_, by simp, same⟩)
    answer
  have defined : M p ↔ ((M a ∧ ¬ M b) ∨ (¬ M a ∧ M b)) := by
    simpa [Satisfies, Ferraris.Neg] using completion
  by_cases pa : M a <;> by_cases pb : M b <;> by_cases pp : M p <;>
    simp_all [parity, truth_eq_true.mpr, truth_eq_false.mpr]

end Zetesis.ParityRows
