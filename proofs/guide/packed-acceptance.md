# Packed acceptance after normal closure

[`PackedAcceptance.lean`](../Zetesis/PackedAcceptance.lean) completes the logical
decision around [packed normal closure](packed-closure.md). It works in
`Zetesis.Refinement.PackedAcceptance` over finite normalized rules and bounded
atom coordinates. Ground filters have already been discharged.

Two final operations determine acceptance. `constraints` selects the enabled
headless rules using the frozen seed, then rejects when a selected positive body
is true in the completed closure. `agrees` compares frozen and derived packed
membership at every coordinate in the supplied gate carrier. These mirror the
final loops in `zetesis-cpu/src/static_oracle.rs::check_static_view`.

## Seed admission is a separate obligation

The carrier comparison reads only its supplied coordinates. It cannot detect a
true seed atom outside them. For example, with empty rules and an empty carrier,
both final loops pass even if the seed contains another atom; normalized
`Semantics.Accept` would reject that seed because it requires exact projection
equality.

Accordingly, `agrees_exact` and `check_exact` require every seed atom to belong to
the carrier. This is a structural admission premise, not an assumption that
the computed closure or verdict is correct. The carrier may conservatively
contain more atoms than occur in the retained rules' gates. No uniqueness or
ordering premise is needed for either the seed list or the carrier list.

Rust establishes this boundary before the final loop:

- `Seed::new` delegates to `SelectionBuilder::manual`, which uses
  `Program::locate_atom(atom, true)` to require gate-carrier membership.
- Construction from carrier tokens rejects nongate predicates; construction
  from gate tokens preserves their checked coordinates and program identity.
  `SeedView` and the inseparable token fields cannot be assembled publicly.
- `GroundProgram::compile` builds its dense carrier from all admitted program
  atoms and retains exactly the gate atoms in `gate_atom_ids`.
- `SeedAtom::resolve_in` checks program identity. Indexed entries address that
  gate list; manual entries resolve previously admitted logical carrier atoms
  in the complete graph.

These are reviewed Rust correspondence points, not new extraction theorems.
Proving that the constructors and canonical graph mapping establish the Lean
admission premise remains part of a concrete implementation refinement.

## From completed words to acceptance

`constraints_exact` derives the existing `Semantics.ConstraintsOK` condition
from the packed rule-selection and body tests. `agrees_exact` derives full
projection equality from carrier-only comparison plus seed admission.

`check` runs `PackedClosure.close` with the proved head-list bound, extracts its
completed word list, and combines those two final decisions. `check_exact`
composes `PackedClosure.close_exact` with the final-loop laws to establish:
the computed Boolean is true exactly when `Semantics.Accept` holds. No closure,
constraint-checking or agreement oracle is assumed.

`check_sound` then uses the existing semantic acceptance theorem: when the
supplied carrier covers every rule gate, successful checking establishes that
the least closure is an answer set. Carrier coverage and seed admission are
distinct premises. Enumeration of all candidate seeds remains separate.

The mathematical checker is total because its scan fuel is proved sufficient.
It does not model allocation, cancellation, deadlines, work or consequence
limits, program ownership, or machine-sized arithmetic. It also makes no claim
about the Rust loops' exact short-circuit order or statistics. A machine resource
stop must still remain distinct from either completed Boolean decision.
