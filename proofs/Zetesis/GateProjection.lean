import Std

/-!
# Exact finite projection for enabled Boolean gates

Domains are two-bit subsets of Bool. Each of the three observed masks may differ
even when positions share storage: coherence constrains a relation row, not the
times of those observations. The independent reference evaluates Boolean
connectives and position equalities; the implementation model intersects masks
of the eight rows indexed by left + 2*right + 4*output.

The finite argument checks all 3 × 5 × 4³ cases. Its corollary permits intersection
into arbitrary current masks at aliased physical slots. Frozen-false gates
remain outside this enabled-transfer contract; their existing output constraint
and disabled connective are not replaced.

Definitions originate in the retained gate-transfer draft at revision 93d2575.
These are mathematical representation laws, not Rust/WGSL compiler refinement,
atomic execution, propagation convergence, complete search, or GPU qualification.
-/

namespace Zetesis.GateProjection

abbrev Domain := Fin 4
/-- 0 is conjunction, 1 disjunction, 2 material implication. -/
abbrev Operation := Fin 3
/-- Distinct, left=right, left=output, right=output, and all equal. -/
abbrev Aliases := Fin 5

def value (op : Operation) (x y : Bool) : Bool :=
  if op.val = 0 then x && y else if op.val = 1 then x || y else !x || y

def coherent (a : Aliases) (x y z : Bool) : Bool :=
  if a.val = 0 then true
  else if a.val = 1 then x == y
  else if a.val = 2 then x == z
  else if a.val = 3 then y == z
  else (x == y) && (y == z)

def contains (d : Domain) (v : Bool) : Bool :=
  (d.val &&& (if v then 2 else 1)) != 0

def reference (op : Operation) (a : Aliases) (dx dy dz : Domain) : Nat × Nat × Nat :=
  ([false, true].flatMap fun x => [false, true].flatMap fun y =>
    [false, true].map fun z => (x, y, z)).foldl (fun result row =>
      let (x, y, z) := row
      if contains dx x && contains dy y && contains dz z &&
          coherent a x y z && (value op x y == z) then
        (result.1 ||| (if x then 2 else 1),
         result.2.1 ||| (if y then 2 else 1),
         result.2.2 ||| (if z then 2 else 1))
      else result) (0, 0, 0)

def operationRows (op : Operation) : Nat :=
  if op.val = 0 then 0x87 else if op.val = 1 then 0xe1 else 0xd2

def aliasRows (a : Aliases) : Nat :=
  if a.val = 0 then 0xff
  else if a.val = 1 then 0x99
  else if a.val = 2 then 0xa5
  else if a.val = 3 then 0xc3
  else 0x81

def expand (d : Domain) (no yes : Nat) : Nat :=
  (if contains d false then no else 0) ||| (if contains d true then yes else 0)

def project (rows no yes : Nat) : Nat :=
  (if rows &&& no != 0 then 1 else 0) ||| (if rows &&& yes != 0 then 2 else 0)

def direct (op : Operation) (a : Aliases) (dx dy dz : Domain) : Nat × Nat × Nat :=
  let rows := operationRows op &&& aliasRows a &&&
    expand dx 0x55 0xaa &&& expand dy 0x33 0xcc &&& expand dz 0x0f 0xf0
  (project rows 0x55 0xaa, project rows 0x33 0xcc, project rows 0x0f 0xf0)


/-- Bit-mask projection yields exactly the position supports of independent
Boolean row enumeration, including empty and differently timed aliased masks. -/
theorem bitwise_support_exact : ∀ (op : Operation) (a : Aliases) (dx dy dz : Domain),
    direct op a dx dy dz = reference op a dx dy dz := by
  -- Kernel reduction discharges the complete finite 960-case representation.
  decide

/-- Every returned support is itself a valid two-bit Boolean domain. -/
theorem bitwise_support_bounded : ∀ (op : Operation) (a : Aliases) (dx dy dz : Domain),
    (direct op a dx dy dz).1 < 4 ∧
    (direct op a dx dy dz).2.1 < 4 ∧
    (direct op a dx dy dz).2.2 < 4 := by
  decide

/-- Physical positions for each realizable equality partition. -/
def slots (a : Aliases) : Fin 3 → Fin 3 :=
  fun i => if a.val = 0 then i
    else if a.val = 1 then (if i.val = 2 then 1 else 0)
    else if a.val = 2 then (if i.val = 1 then 1 else 0)
    else if a.val = 3 then (if i.val = 0 then 0 else 1)
    else 0

/-- Intersect each position's support into its physical slot. A repeated slot
receives every associated support. Current masks need not equal observations. -/
def intersect (a : Aliases) (current : Fin 3 → Domain)
    (support : Nat × Nat × Nat) : Fin 3 → Nat := fun slot =>
  (current slot).val &&& (if slots a 0 = slot then support.1 else 3) &&&
    (if slots a 1 = slot then support.2.1 else 3) &&&
    (if slots a 2 = slot then support.2.2 else 3)

/-- Replacing row enumeration by the bit-mask projection preserves every
physical intersection, even with aliases and any current Boolean domains.
This is equality of finite operations, not an assertion about interleavings. -/
theorem aliased_intersection_exact (op : Operation) (a : Aliases)
    (dx dy dz : Domain) (current : Fin 3 → Domain) :
    intersect a current (direct op a dx dy dz) =
      intersect a current (reference op a dx dy dz) := by
  have positionSupportsAgree :
      direct op a dx dy dz = reference op a dx dy dz :=
    bitwise_support_exact op a dx dy dz
  exact congrArg (intersect a current) positionSupportsAgree

end Zetesis.GateProjection
