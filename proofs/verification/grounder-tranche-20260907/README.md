# Grounder tranche proof integration

The complete audited public inventory grows from 616 to 646 theorems in 45
modules. `StructuralBindings` contributes 14 laws, `FiniteValues` eight and
`ConsequentAlternatives` eight. Their modules are independently importable and
included in the `Zetesis` umbrella and reading guide.

The strict record convention accepts ordinary line-leading public `theorem`
declarations. Four reusable structural laws previously used `private theorem`:
`extends_refl`, `extends_trans`, `bind_sound` and `bind_fits`. They now have public
names and concise mathematical contracts. Their statements and proof bodies
are preserved; the pre-change module is retained as
`StructuralBindings.before.lean`. Comparison through the existing checker's
comment/string masking confirms identical token text after removing only the
old `private` modifiers. The checker itself is unchanged.

Structural matching proves consistent finite extensions, rollback, explicit
support-row coverage and retention of original logical atoms. Finite value
plans use supplied partial operations and declared source inputs, with exact
constructor/tuple identity and original/frozen transport under resolved-atom
identity. Conditional rows keep their antecedents and existential signed
consequents intact; completed empty domains, polarity placement and extensional
collection replacement have explicit laws. None of these results proves source
recognition, safety/scope, actual complete joins, allocation/work counters,
machine arithmetic, Rust lowering or backend refinement.

The final sorted `theorems.json` and `Audit.lean` derive from the existing
`scripts/proof_record.py` declaration inventory convention. The pinned Lean
4.33.1 clean build and strict audit cover all 646 declarations. Commands,
actual exit statuses, elapsed times and raw logs are retained here. The audit
admits only standard Lean logical axioms: `Classical.choice`, `Quot.sound` and
`propext`; there are no project axioms, proof holes or native-evaluation shortcuts.
The complete retained record passes the unchanged consistency checker.

The prior umbrella, index, audit, README and verification manifest were backed
up before this refresh. They have not been overwritten. `guide.before.md`
preserves the reading guide bytes matching the prior record's hash. Earlier
historical records remain authentic at their original paths; current hashes
identify this integration. Lean checking establishes these mathematical laws,
while the record checker establishes inventory/hash/log consistency. Neither
one verifies a source-to-Rust correspondence or reports runtime performance.
