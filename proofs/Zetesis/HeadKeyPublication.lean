/-!
# Publishing each keyed head once

Parallel support grounding stages witness rows on workers, and the coordinator
publishes them in staged order: for each row it admits the row's head atom and
retains it in support. Workers also intern each row's head key into one shared
interner; the coordinator then publishes only the first row of each key and
skips the rest (`zetesis_themelios` `row_blocks::witness_rows::heads`).

`keyed_eq_plain` proves the skip changes nothing. Its assumptions are named
hypotheses, one per obligation the Rust code discharges:

* `keySound`: rows of equal key have equal heads. In Rust the key is the head's
  projected columns' equality identifiers within fixed source relations.
* `insertIdem`: inserting an atom the state already contains leaves it
  unchanged. In Rust admission finds the existing atom and retention returns
  early on the support bit.
* `insertContains` and `insertMono`: after inserting an atom the state contains
  it, and admission loses nothing.

The state is abstract, so the result covers both the published atom order and
the support set, and any atoms pending before publication starts.

These laws do not verify the interner's hash tables, shard locking, the freeze
or the work accounting.
-/

namespace Zetesis.HeadKeyPublication

universe u v w x
variable {ρ : Type u} {β : Type v} {κ : Type w} {σ : Type x}

/-- Ordinary publication: insert every staged row's head, in staged order. -/
def plain (insert : σ → β → σ) (head : ρ → β) (s : σ) (rows : List ρ) : σ :=
  rows.foldl (fun s r => insert s (head r)) s

/-- Keyed publication: skip a row whose key an earlier row already published. -/
def keyed [DecidableEq κ] (insert : σ → β → σ) (head : ρ → β) (key : ρ → κ)
    (state : List κ × σ) (rows : List ρ) : List κ × σ :=
  rows.foldl
    (fun (state : List κ × σ) r =>
      if key r ∈ state.1 then state else (key r :: state.1, insert state.2 (head r)))
    state

/-- The coordinator's invariant: every published key's rows have their heads
in the state already. -/
def Published (contains : σ → β → Prop) (head : ρ → β) (key : ρ → κ)
    (all : List ρ) (state : List κ × σ) : Prop :=
  ∀ k, k ∈ state.1 → ∀ r, r ∈ all → key r = k → contains state.2 (head r)

/-- Keyed publication of any suffix of the staged rows, from a state meeting the
invariant, ends in the state ordinary publication reaches.

Outline. Induct on the suffix. A row whose key is already published has its
head in the state (the invariant), so inserting it would change nothing
(`insertIdem`): skipping it is the ordinary step. A row with a new key takes the
ordinary step; afterwards its key's rows all have the same head (`keySound`),
which the state now contains (`insertContains`), and earlier keys keep their
heads (`insertMono`), so the invariant holds for the rest. -/
theorem keyed_suffix_eq_plain [DecidableEq κ]
    {insert : σ → β → σ} {head : ρ → β} {key : ρ → κ} {contains : σ → β → Prop}
    {all : List ρ}
    (keySound : ∀ r, r ∈ all → ∀ r', r' ∈ all → key r = key r' → head r = head r')
    (insertIdem : ∀ s b, contains s b → insert s b = s)
    (insertContains : ∀ s b, contains (insert s b) b)
    (insertMono : ∀ s b c, contains s c → contains (insert s b) c) :
    ∀ (rows : List ρ) (state : List κ × σ), (∀ r, r ∈ rows → r ∈ all) →
      Published contains head key all state →
      (keyed insert head key state rows).2 = plain insert head state.2 rows := by
  intro rows
  induction rows with
  | nil => intro state _ _; rfl
  | cons r rest ih =>
    intro state inAll published
    have rIn : r ∈ all := inAll r List.mem_cons_self
    have restIn : ∀ r', r' ∈ rest → r' ∈ all :=
      fun r' h => inAll r' (List.mem_cons_of_mem r h)
    by_cases seen : key r ∈ state.1
    · -- A repeat: the ordinary step would leave the state unchanged.
      have present : contains state.2 (head r) := published (key r) seen r rIn rfl
      have unchanged : insert state.2 (head r) = state.2 := insertIdem _ _ present
      have step :
          (keyed insert head key state rest).2 = plain insert head state.2 rest :=
        ih state restIn published
      simp only [keyed, plain, List.foldl_cons, seen, if_true] at step ⊢
      rw [unchanged]
      exact step
    · -- A new key: take the ordinary step and re-establish the invariant.
      have next : Published contains head key all
          (key r :: state.1, insert state.2 (head r)) := by
        intro k hk r' hr' hkey
        rcases List.mem_cons.mp hk with same | earlier
        · have equal : head r' = head r := keySound r' hr' r rIn (hkey.trans same)
          rw [equal]
          exact insertContains _ _
        · exact insertMono _ _ _ (published k earlier r' hr' hkey)
      have step :
          (keyed insert head key (key r :: state.1, insert state.2 (head r)) rest).2 =
            plain insert head (insert state.2 (head r)) rest :=
        ih _ restIn next
      simp only [keyed, plain, List.foldl_cons, seen, if_false] at step ⊢
      exact step

/-- Publishing all staged rows, skipping repeated keys, from any starting state
with no published key yet, ends where ordinary publication does. -/
theorem keyed_eq_plain [DecidableEq κ]
    {insert : σ → β → σ} {head : ρ → β} {key : ρ → κ} {contains : σ → β → Prop}
    (rows : List ρ) (s : σ)
    (keySound : ∀ r, r ∈ rows → ∀ r', r' ∈ rows → key r = key r' → head r = head r')
    (insertIdem : ∀ s b, contains s b → insert s b = s)
    (insertContains : ∀ s b, contains (insert s b) b)
    (insertMono : ∀ s b c, contains s c → contains (insert s b) c) :
    (keyed insert head key ([], s) rows).2 = plain insert head s rows := by
  have fresh : Published contains head key rows ([], s) := by
    intro k hk
    exact absurd hk List.not_mem_nil
  exact keyed_suffix_eq_plain keySound insertIdem insertContains insertMono rows ([], s)
    (fun _ h => h) fresh

end Zetesis.HeadKeyPublication
