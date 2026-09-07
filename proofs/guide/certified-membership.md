# A completed membership check answers the original stability question

This is a mathematical reading of
[`CertifiedExecution.completed_membership_exact`](../Zetesis/CertifiedExecution.lean),
the first pilot under the [structured proof convention](../STYLE.md).
The checked declaration is authoritative. Its statement and assumptions are
unchanged by the proof-body refactor.

## The objects and the claim

Fix an original finite formula theory **T** and an interpretation **M**.
`Stable M T` means that M satisfies T and has no proper subset satisfying the
reduct of T frozen at M. It is the same T throughout this argument.

An optional checker produces one of four verdicts: certified stable, not a model
of the original theory, residual work requiring exact completion, or stopped.
A completion operation returns `some true`, `some false`, or `none`. These mean
a completed affirmative answer, a completed negative answer, or no completed
answer. `none` is not a negative answer.

Assume:

- **Sound verdict.** Certified stable implies `Stable M T`; original-model
  rejection implies that M does not satisfy T. Residual and stopped verdicts
  make no stability claim.
- **Exact residual oracle.** Whenever the supplied exact result is a Boolean b,
  b is true if and only if `Stable M T`.
- **Completed result.** Combining the verdict and supplied exact result returns
  a Boolean `result` rather than `none`.

**Claim:** `result = true` if and only if `Stable M T`.

The exact residual premise is an assumption of this composition theorem. This
proof does not construct an exact oracle. The preceding
`ranked_support_verdict_sound` theorem is one way to justify a sound verdict,
under its own complete-producer, rank, original-truth and support-check-soundness
premises. In particular, a positive support test must imply the mathematical
`Supported M rules` property.

## Structured argument

Each top-level obligation has its own local assumption. Its intermediate facts
are used only within that obligation; the final step combines the four cases.
The labels below correspond to named local facts in the Lean source.

### 1. Certified acceptance (`certified_case`)

**Assume** the verdict is certified stable. **Prove** the claim.

1.1. The completion definition returns true in this case. The completed-result
assumption therefore establishes `result = true` (`accepted`).

1.2. The sound-verdict assumption establishes `Stable M T` (`stable_model`).

1.3. Both sides of the required equivalence hold, so either implies the other.
This proves the claim for this case.

### 2. Original-model rejection (`rejected_case`)

**Assume** the verdict rejects M as an original model. **Prove** the claim.

2.1. Completion returns false, so `result = false` (`not_accepted`).

2.2. Verdict soundness says that M does not satisfy T (`not_original_model`).

2.3. Every stable model satisfies its original theory. If M were stable, that
would contradict 2.2; hence M is not stable (`not_stable`).

2.4. Both sides of the required equivalence are false, so the equivalence holds.
This proves the claim for this case.

### 3. Exact residual completion (`residual_case`)

**Assume** the verdict leaves residual work. **Prove** the claim.

3.1. Here completion passes through the supplied exact result. The completed-result
assumption therefore says that the supplied exact result is `some result`
(`exact_result`).

3.2. Apply the exact residual-oracle assumption to 3.1. Its conclusion is precisely
the required equivalence, establishing the claim for this case.

### 4. Stopped completion is impossible (`stopped_case`)

**Assume** the verdict is stopped. **Prove** a contradiction.

4.1. By definition, completion of a stopped verdict returns `none`, irrespective
of any supplied exact result (`no_completed_result`).

4.2. The completed-result assumption requires `some result`. This contradicts 4.1.
Thus a stopped verdict cannot occur under the theorem's completed-result premise.

### 5. QED

The verdict type contains exactly these four alternatives. The first three
establish the claim and the fourth is impossible under the assumptions.
Exhaustive case analysis therefore proves the original claim.

## What this establishes for the architecture

A sound specialization and an exact residual path can implement one membership
contract. A caller asks whether M is stable for T; it need not define stability
in terms of which path was used. In particular, failure to obtain a specialization
certificate need not be mistaken for instability, and interruption need not be
mistaken for rejection.

The theorem does **not** assert that any operation completes, that all candidate
models are enumerated, that source grounding is complete, that an optimum has
been proved, or that a model was successfully written. It also does not verify
Rust counters, memory, a shader or a scheduling policy. Those obligations remain
separate, even though this completed membership composition is kernel-checked.
