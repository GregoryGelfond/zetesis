import Std

/-!
Finite isolated draft: exact truth-vector transfer for enabled Boolean gates.
The proof is intentionally independent of the production audit inventory.
It does not model WGSL compilation, atomics, shader execution or convergence.
-/

namespace GateTransferExperiment

abbrev Domain := Fin 4
abbrev Operation := Fin 3
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

/-- All 3 × 5 × 4³ enabled-gate transfers, including empty and mixed domains. -/
theorem exact_transfer : ∀ (op : Operation) (a : Aliases) (dx dy dz : Domain),
    direct op a dx dy dz = reference op a dx dy dz := by
  decide

#print axioms GateTransferExperiment.exact_transfer

end GateTransferExperiment
