# zetesis-core contract

This std-only crate owns typed symbolic values, signed atoms, canonical storage,
borrowed template components, exact models, relations, and packed operations.
Semantic owners interpret rule structure and admission policy. The crate has no
parser, Rayon, GPU, filesystem, or process dependency.

## Typed identities and construction descriptions

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

`Template::new(head, positive, gate_true, gate_false, filters)` constructs an
immutable normalized rule description. Its accessors preserve each supplied
occurrence and its role. The constructor does not establish variable safety,
density, support, or truth. `AtomPattern::instantiate` and `Filter::evaluate`
resolve assignments and return the first missing variable ID; comparisons retain
exact typed equality.

`AdmissionLimits` remains the common limit description for semantic admission
owners. Its field names, defaults, and zero-as-real-limit convention are unchanged.
The owner interpreting a limit supplies its error and source identity. These
limits do not make a core template description an admitted logical program.

## Canonical template components

`TemplateCatalogBuilder` imports ordered scalar fields, pattern occurrences, and
filters into one canonical term/predicate vocabulary. `append` preserves rows,
duplicates, and local variable coordinates; `include_atom_vocabulary` imports
vocabulary without asserting an atom or adding a component row. `finish` freezes
the vocabulary. Failed append prevents publication of an accepted prefix.
Already validated values retain their admitted depth without an implicit lower
limit. Named storage covers canonical data/indexes, component capacities, scratch,
and publication overlap; registered external metadata participates in admission.
Caller descriptions, allocator bookkeeping, and Arc counters are excluded.

`TemplateTerm`, `PatternRef`, `PatternTerms`, `Patterns`, `FilterRef`, and `Filters`
remain root exports. Constants borrow their read owner; variables remain local
coordinates. Individual patterns and filters can borrow ingress descriptions or
explicit borrowed parts as well as canonical metadata. `TemplateRow` exposes the
stored component sequences with exact-size, double-ended iteration. Rows do not
establish rule topology, safety, a carrier domain, or logical truth.

`TemplateComponents` and `TemplateCatalogSelection` retain the existing scoped
metadata paths over an authoritative catalog. A selected catalog retains its
exact source atom snapshot; a closed catalog retains its indexed vocabulary.
Their checked failures and publication accounting remain separate from semantic
admission policy.

## Atoms, models, and relations

`AtomCatalog` retains authoritative canonical atom occurrences. `AtomRef`,
`PredicateRef`, and `TermRef` borrow that storage or ingress descriptions. Semantic
Eq/Ord/Hash use complete signed and typed contents across independent owners.
`AtomInterner::for_template_catalog` starts an independent atom writer over a
closed indexed template vocabulary. It shares term/predicate identities and
refuses new vocabulary; shared vocabulary never confers shared atom identity.

`Model` denotes an exact canonical selected set. Construction, selection, and
cloning preserve the existing owned and borrowed APIs; no model constructor
certifies stability. `AtomIndex`, `CatalogIndex`, and `Model::lookup` retain typed
lookup and original-row identity. Checked comparison and lookup refusals remain
errors before the refused operation, never absence or logical exhaustion.

Relation storage, dictionary translation, selection, unification, packed kernels,
and their named work/storage contracts remain independent of relational program
admission. Named storage measures are not RSS or total process-memory bounds.

## Verification

The canonical template, writer, and model integration tests retain typed identity,
shared payload, complete occurrence/membership, ownership, metadata refusal, and
checked operation-boundary propositions. Existing catalog, relation, unification,
packed, and model publication tests remain in place. The crate doctest constructs
an exact model without asserting acceptance. Rust regression tests remain distinct
from the separate Lean laws; no Rust refinement proof is implied.
