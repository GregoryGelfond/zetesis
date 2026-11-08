# Retain the peak of an observed byte requirement

[BytePeak](../Zetesis/BytePeak.lean) takes three natural numbers: a previous
peak, a supplied total and the current inclusive ceiling. `observe` accepts
exactly when the total fits. Either outcome retains the maximum of the old
peak and the supplied total.

## Separate the decision from the receipt

`accepted_iff` and `refused_iff` characterize the ceiling decision and resulting
peak together. `receipt_eq_max` forgets the decision and states the same maximum
for both outcomes. `receipt_covers_inputs` follows from the two bounds on that
maximum: it cannot decrease the old peak or omit the supplied total.

For peak 8, total 16 and ceiling 12, the result is refusal with peak 16.
This records the observation; it does not admit the resource or claim that
memory was allocated. The caller establishes what its supplied total means.

Only the total is checked against the ceiling. `acceptance_independent_of_previous`
shows that changing the historical peak cannot change that decision.
`observe_zero` therefore accepts zero and preserves the old peak under every
ceiling, even peak 16 with total 0 and ceiling 0.

## Compose receipts without erasing execution boundaries

`successive_receipts` supplies the first observation's receipt as the second
observation's previous peak. Applying `receipt_eq_max` twice and associativity
of maximum gives the maximum of the initial peak and both totals. The ceilings
may differ because neither decision changes how the receipt is formed.

This is a statement about receipts, not permission to execute after refusal.
It does not combine two ceiling decisions, remove an intervening poll or
preserve a caller's failed trace under batching. Unlike
[AtomicWork](atomic-work.md), which preserves the accepted-work prefix on
refusal, this operation retains a refused byte observation in its peak.

## Locate the executable correspondence

`zetesis_algebra::MeterBytes::observe_peak` borrows the existing u128 peak,
publishes its maximum with the supplied total, then checks that total.
It adds no current/live account, allocation, release, work charge or poll.
The stateful `MeterBytes::observe` assigns its current total before delegating.
Arithmetic preparation retains its explicit poll before reporting a requested
byte envelope, including a declined requirement. Frozen-positive witness
construction keeps requested preflight and allocation before observing actual
simultaneous capacity; its charged copy loop remains after that observation.

The scalar laws do not establish the source of a total, actual allocator
capacity, current/live ownership, release lifetime, finite-width calculations
that produce totals, native error mapping, control order or Rust execution.
A retained peak alone proves neither allocation nor completed solving.
