# Rust functions during source construction

Rust is the intended first-class implementation language for `@` functions.
Python and Lua scripting remain excluded. This is a future capability: current
zetesis source admission refuses external terms. The public registration,
callback and symbol-conversion API belongs to themelios; this document specifies
zetesis's execution obligations, not a second extension framework. See the
[integration contract](themelios-solve-integration.md) for the inspected themelios
design and the unchanged dependency pin.

## A function result belongs to a program version

A solve sees one immutable source/context version. A callback invocation is
identified by its registered function identity, typed argument vector and context
version. Its successful result is a finite collection of shared `Symbol` values.
The shared contract must settle ordering and duplicate semantics; the engine
cannot substitute string equality or silently change a sequence into a set.
Empty success is distinct from a callback failure.

For a deterministic function, every use of that invocation in a version must
agree on its result. A cache may reuse a successful result only under those exact
identities. Replacing a function implementation or its context starts a new
version and invalidates dependent expansion and analysis results. A Rust `&self`
receiver alone is no evidence of determinism: interior mutability, clocks and
external services can all change results.

The first implementation should require deterministic results relative to the
registered context snapshot. Effectful functions need a separate contract for
execution and replay before they can participate in lazy construction. They
cannot be scheduled speculatively or called again during reduct evaluation under
an assumed purity guarantee.

## Execution and admission

| Stage | Required behavior |
| --- | --- |
| Source inspection | Resolve the function name and capability; retain the original call site and surrounding variable scope |
| Argument construction | Evaluate typed arguments only when their required variables are bound; nested calls respect their data dependencies |
| Host invocation | Apply the shared callback contract, contain unwind-capable panics or use declared isolation, and attribute failure to the original site |
| Result admission | Check symbol support and output/construction limits before publishing results to possible-support relations |
| Expansion | Preserve the language's multi-valued term semantics and all required result combinations; charge generated bindings and formula growth |
| Exact membership | Evaluate the fixed admitted theory and its Ferraris reduct; callback execution supplies no independent atom support |

A zero-argument function can be a finite source of values if its declared
capability permits that use. A function with an unbound argument is not an
enumerator for that argument. Returning a value also does not establish the
safety of unrelated source variables. Function outputs may contain terms beyond
zetesis's current scalar fragment; until those terms are supported, conversion
must return a located refusal without conflating distinct symbols.

Each invocation needs finite output and byte allowances. The whole construction
also needs limits on distinct invocation keys, nesting, generated combinations,
possible atoms, formula nodes and total work. Per-call finiteness does not prove
global finiteness: recursive rules can continually form new arguments. Exhausting
any required construction allowance means incomplete construction, never a proof
that the program has no answer sets. These allowances bound admitted data and
engine construction, not arbitrary allocations made inside an opaque callback
before it returns its collection. Callback memory limits require an additional
cooperative or isolation capability.

Rust host code cannot in general be safely preempted inside an ordinary function
call. Cooperative cancellation or a stronger isolation contract is required for
bounded callback latency. Checking a deadline before and after a callback only
bounds the engine's own scheduling; it does not make a blocking callback obey the
deadline. The adapter must expose this distinction through the shared capabilities
and completion vocabulary. Likewise, an aborting Rust panic cannot be contained
by catching unwinding; the callback ABI/build contract or isolation mechanism
must establish the promised failure boundary.

## Lazy and accelerated execution

The possible-support generator may request a function result when its arguments
become relevant. It must retain the same coverage obligation as any other deferred
relation: an accepted answer cannot depend on overlooking a required source
instance. For a completed version, results must agree on observed invocation
keys, and the admitted expansions must establish sufficient final semantic
coverage. This does not require executing every unreachable syntactic call. A
function failure encountered while establishing that coverage remains a failure.

Analysis treats an opaque callback conservatively. Placement and batching may
use declared determinism, thread safety, argument dependencies and measured cost.
Rayon invocation requires the shared concurrency contract and immutable version
identity; result publication must preserve symbol and source identities regardless
of completion order. Cached failures cannot become empty successful results.

Initial Rust callbacks run on the host. Their admitted finite outputs can feed
the same relation, threshold and reduct operators as ordinary source data. This
does not require compiling arbitrary Rust libraries into WGSL. A future device
implementation of a particular registered function needs its own equivalence
contract and tests against the Rust implementation; availability of Metal or an
NVIDIA adapter alone cannot authorize substitution.

## Refinement and conformance obligations

Lean should model a version-bound, finite invocation table rather than arbitrary
Rust execution. The key obligations are extensional substitution of equal table
entries, cache-hit equivalence, independence of a permitted evaluation order and
preservation of candidate coverage under deferred expansion. The resulting fixed
theory still uses the existing original-modelhood and proper-subset reduct
criterion. Those proofs would assume the callback contract; they would not prove
Rust purity, panic containment, scheduling or foreign-library behavior.

Before claiming this capability, compare the future themelios clingo adapter and
zetesis adapter on the same typed function registrations. Include empty and
multi-valued results, duplicate values, nested calls, shared symbols and compound
terms, several call sites, context replacement, recursive expansion limits,
callback errors/panics, cancellation and concurrent calls. Check complete models,
optimal ties, located failures and completion status. Existing single-shot corpus
and Lean results establish none of these extension guarantees yet. In particular,
the current `LiftedBridge` coverage result concerns normalized S0 final snapshots;
extending that bridge to general Ferraris source construction with callbacks is
separate proof work.
