# Admit a whole work request without rewriting its history

[AtomicWork](../Zetesis/AtomicWork.lean) separates the work already accepted from
the limit supplied for the next operation. Its inputs are natural numbers:
receipt `recorded`, current inclusive `limit`, and the whole requested `amount`.
`charge` accepts exactly when `recorded + amount ≤ limit`. It returns that sum
on success and the original receipt on refusal.

## Read admission and refusal separately

`accepted_iff` states both the comparison and the resulting sum;
`accepts_iff_fits` forgets the result to expose just the admission criterion.
`accepted_additive` and `accepted_within_limit` are direct consequences.
The latter needs no assumption that the old receipt fits the current limit:
a successful complete demand establishes that fact.

`refused_iff` states excess demand and unchanged receipt together.
`refused_preserves_prefix` therefore admits none of the new request. It neither
clamps the old receipt nor charges the unused portion of the allowance.
For example, receipt 7 and request 2 under limit 8 produce refusal with receipt 7,
not receipt 8 or 9.

A caller can import work accepted under earlier limits. If its receipt is 7 and
the new limit is 3, `above_new_limit_refused` applies to every request, including
zero. `charge_zero` and `zero_accepted_iff` state the checked zero decision;
`zero_preserves_receipt` shows that either outcome retains the same number.
The caller may reject the next operation without erasing its previous work.

## Compose successful receipts only

`accepted_composition` starts with two successful charges under one unchanged
limit. The first identifies the intermediate receipt; the second bounds and
identifies the final receipt. Addition then gives the same final result for the
combined request. This is not an instruction to batch execution or polling.
It does not equate failed traces: separate requests can retain a successful
first prefix when the second fails, whereas a refused combined request retains
the receipt from before both.

The existing [WorkCharge](../Zetesis/WorkCharge.lean) describes a different
policy: consuming the available prefix of a failed batch, equivalent to repeated
unit charging when the initial receipt fits the limit. Its laws remain useful,
but they do not prove atomic refusal. [WorkPermits](../Zetesis/WorkPermits.lean)
separately describes conservation of committed, available and outstanding grants.

## Locate the executable correspondence

`zetesis_algebra::Meter::charge_receipt` borrows the caller's accepted u64 receipt,
checks a u128 requested amount against the current u64 ceiling, and commits only
on success. It does not construct another stored meter, observe control,
allocate, obtain permits or refund them. The observation evaluator retains its
Statistics receipt and its explicit cancellation/deadline poll before calling
this operation; importing a receipt for later rendering does not itself charge
work or reject a lower new limit.

For representable sums, the comparison and commit correspond to `AtomicWork`.
Naturals do not prove finite-width checked addition, the public
`MeterWorkError::Overflow` case, conversions or exact native failure payloads.
The observation adapter relies on its source producer bound to rule out that
overflow; artificial overflowing private inputs are outside that argument.
Source attribution, cancellation order, failure snapshots, byte accounting and
Rust execution remain separate obligations. Neither this scalar law nor a
successful work admission proves that the work executed or that an answer set
was found.
