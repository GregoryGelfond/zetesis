# zetesis-core contract

This std-only crate owns symbolic keys, admitted relational templates, sparse
complete candidates, and explicitly eager static lowering. It has no parser,
Rayon, GPU, learning, filesystem, or process dependency. All public types are
re-exported at `zetesis_core`.

## Templates and admission

`Value` retains its scalar variants and their relative storage ordering, and adds
`Structured(StructuralValue)`. `Value::from_nodes` validates a single closed
preorder tree under `ValueLimits`, without recursive traversal. Constructor
sign/name/arity and ordered children all participate in identity. Positive
nullary functions normalize to `Symbol`; tuples (including empty and singleton)
and negative nullary functions remain distinct. `Ord` is deterministic storage
order; `compare_terms` separately implements ASP term order. Neither coerces a
number, string, symbol or compound to another class.

Structural nodes and cached canonical spelling are immutable shared buffers.
Cloning a structure shares them; Eq/Ord/Hash inspect values, never addresses or
allocation capacities. Validated depth and spelling are deterministic derivatives
of the nodes. Display streams cached spelling without internal allocation;
construction, comparison, hashing, debug and drop do not recurse through logical
depth. `ValueLimits.max_bytes` counts node-buffer capacity, text lengths, spelling
capacity and simultaneously reserved validation/render frames. It does not count
allocator headers, text spare capacity or reference-count headers and is not RSS.
`payload_bytes` excludes construction scratch; consumers may conservatively charge
shared payload per use. `canonical_bytes` counts exact tags, lengths, signed
constructor headers and leaf payload, independently of storage capacity.

Predicate signatures compare complete name/arity/sign. Opposite signs are distinct
core identities; source adapters provide coherence. Atoms compare the signature
and complete tuple. String and symbolic values with equal spelling remain distinct.
The additive public enum variant requires downstream exhaustive matches to migrate.

`Predicate::new(name, arity)`, `Atom::new(predicate, values)`, and
`AtomPattern::new(predicate, terms)` return typed local construction errors.
Their fields are private. `Term::{Variable(usize), Constant(Value)}` and
`Filter::{Eq(Term, Term), Neq(Term, Term)}` describe normalized arguments.

```text
Template::new(head, positive, gate_true, gate_false, filters) -> Template
Program::new(Vec<Template>, AdmissionLimits) -> Result<Program, AdmissionError>
```

The optional head distinguishes constraints. Ordinary positive antecedents read
the growing consequence relation; true and false gates read the frozen seed.
Filters require exact typed value equality/inequality. `Template::head`,
`positive`, `gate_true`, `gate_false`, `filters`, and `variable_count` expose
immutable views. `AtomPattern::instantiate` and `Filter::evaluate` resolve a
complete variable assignment and return the missing variable ID on failure.

Admission preserves template order and duplicates, collects values from every
pattern and filter, and requires variable IDs to be exactly `0..variable_count`
within each template. Every used variable must occur in an ordinary positive
pattern. A comparison or gate does not establish variable safety. Admission
limits bound templates, arity, variables, positive antecedents, and domain values.
`AdmissionError::template_index()` locates a template-level error for adapters.

The domain is exactly the constants supplied in these templates. Nullary atoms
exist even when that domain is empty; positive-arity carriers are then empty.
An adapter wanting an additional domain constant must supply it through its
normalized program. Parser syntax, source integer folding, source choice
lowering, provenance, and source-language preservation remain adapter concerns.

## Instance identity and sparse candidates

`Program` is an immutable Arc-backed handle. `clone()` shares its identity;
independently admitting equal syntax creates a distinct instance.
`Program::same_instance` checks this identity without hashing or global counters.
It is an in-process identity, not a persistent checkpoint identity.

`Seed::new(&program, true_atoms)` stores a canonical `BTreeSet<Atom>`, validates
each atom against the symbolic gate carrier, and binds the seed to that program.
The gate carrier includes every tuple over the domain for every signature
appearing in any gate, including constraint-only gates and filter-false rules.
`Seed::contains` is exact membership; absence is false. The empty seed does not
enumerate or allocate the false complement. `Seed::atoms()` and `program()`
provide immutable views. `Model` is a canonical derived set, not an acceptance
certificate; constructing one does not assert stability.

## Symbolic carrier iteration

`Program::domain()`, `predicates()`, and `gate_predicates()` expose sorted slices.
`gate_atoms()` and `carrier_atoms()` return cloneable, fused
`Iterator<Item = Result<Atom, CarrierError>>` values. Creating an iterator performs
no carrier enumeration and allocates no tuple indexes. Each requested result
holds only one tuple and O(arity) cursor state, advancing in canonical order.
A tuple-buffer reservation error appears once, followed by exhaustion. Callers
must preserve that error; it cannot certify logical exhaustion. Dropping a cursor
is cancellation, not a coverage certificate. Cloning copies its current cursor,
including O(arity) state, and resumes at the same next tuple.

The iterator supplies no total carrier count because computing that count can
overflow even when a sparse candidate is easy to check. Obtain it only when the
selected eager backend requires it. Container reservations are fallible where
reported, but ordinary String/Arc/BTreeSet cloning and allocation retain Rust's
process-wide allocator behavior; no global out-of-memory recovery is claimed.

## Explicit eager static profile

`GroundProgram::compile(&program, StaticLimits)` is the sole static-lowering door.
It checks Cartesian count arithmetic and atom/substitution budgets before
enumeration, and checks the retained-rule budget before each publication. It
materializes the entire atom carrier and every filter-valid ground rule.
This intentionally pays grounding cost and must be identified as eager in
backend reports. The lazy CPU evaluator does not call it implicitly.

Each `GroundRule` exposes an optional `head()`, and sorted duplicate-free
`positive()`, `gate_true()`, `gate_false()` u32-ID slices. Distinct pattern
antecedents that alias after substitution are coalesced. Filters are already
evaluated; filter-false rows are omitted. Duplicate complete rules are retained,
which is semantically harmless but can cost work. Contradictory gates are retained
and disabled by the oracle, preserving a simple normalization boundary.

`GroundProgram::atoms()` is canonical; its slice index is the atom's dense ID.
`atom_id()`, `atom_count()`, `rules()`, `program()`, and `gate_atom_ids()` supply
the backend views. The gate IDs include the entire symbolic gate carrier even
when no retained ground rule consults a particular tuple. Dense IDs are scoped
to this graph; they are not portable semantic atom identities.

`word_count()` counts u32 words per world. `seed_words()` checks program identity
and zeros all complement and padding bits. `model_from_words()` checks exact
word count and zero padding; it returns a model without claiming acceptance.
Zero atoms means zero words and `model_from_words(&[])` succeeds. There is no
64-atom semantic ceiling. The static profile requires atom count to fit u32 and
configured allocation bounds; symbolic programs have no such dense-ID ceiling.

## Verification

`tests/integration/contracts.rs` checks typed identity and admission, safe/dense variables,
canonical carrier enumeration and cloned cursor continuation, sparse construction
over a 2^32 gate carrier, empty domains, substitution/filter behavior, duplicate
ground edges, program identity, retained symbolic gate tuples, count overflow,
and u32 word round trips across 64 atoms. The crate example exercises its public
choice/seed/static APIs as a doctest. These are executable regression checks;
the separate Lean project does not yet prove this Rust implementation refines it.
