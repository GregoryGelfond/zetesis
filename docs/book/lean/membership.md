# Refining packed membership

The optional [membership package](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/membership)
checks a small executable operation beneath answer-set semantics. Charon and
Aeneas translate the actual Rust `Interpretation::contains` body into Lean.
The theorem concerns that extracted body, rather than a separately written
membership implementation.

## Statement

An interpretation stores its declared atom count and a vector of 64-bit words.
For an atom index `a`, packed membership is the bit at position `a % 64` in
word `a / 64`, provided `a` belongs to the declared universe. Outside that
universe, membership is false.

`Membership.Represented` requires a stored word for every declared atom. Under
that premise, `Membership.contains_refines` proves that the extracted method
returns exactly packed membership without an indexing or arithmetic failure.
The premise says nothing about which bits are set. Padding bits need not be zero.

## Argument

1. Outside the universe, the source's short circuit returns false before reading
   any word. This case needs no storage assumption.
2. Inside the universe, division by 64 is defined. The representation premise
   establishes the resulting word index is in bounds.
3. The remainder modulo 64 lies below 64, so shifting a 64-bit unit mask succeeds.
4. The one-bit mask law equates the bitwise test with the selected bit. Substituting
   the quotient and remainder establishes the declared membership result.

The [proof](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/membership/Membership.lean)
names the indexing and shift bounds. Its
[mask lemma](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/membership/Mask.lean)
provides the final bit-vector correspondence. The
[generated body](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/membership/Harness/Funs.lean)
retains the source operations and source spans.

## Scope of the result

Lean checks the generated body against Aeneas's vector, scalar and result models.
A concrete read-only `Arc` model stores a value and returns it on dereference;
it adds no axiom about membership. Its correspondence to Rust reference counting,
allocation and concurrent access is outside this proof. The translation also
trusts the pinned Rust compiler, Charon and Aeneas. Machine-sized integer and
platform correspondence remain part of that boundary.

The package does not yet prove that Rust constructors establish the storage
invariant or that subsequent operations preserve it. It does not establish
satisfaction, reduct evaluation, answer-set checking, enumeration or GPU
correctness. Those operations require further refinement arguments.

## Reproduction

The package records source hashes, extraction options and tool identities. Its
README gives commands to check the retained proof and repeat extraction. Rust
path remapping keeps generated source spans independent of a developer's home
directory; generated Lean definitions are retained without rewriting.

This package uses the extraction backend's pinned Lean 4.31.0. The main semantic
library remains on Lean 4.33.1, with its own build, theorem index and axiom audit.
The membership proof is not included in that library's declaration count.
The small Rust caller is covered by the ordinary Rust gates; the optional
extraction and Lean checks have separate prerequisites and commands.
