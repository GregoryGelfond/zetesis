import Std

/-!
# Check the restricted context generalization

This executable checks a reviewed source transformation, not a kernel theorem
about extraction. Each context must have exactly the declared header changes and
call-site substitutions. Reversing every substitution must recover the complete
generated body. The generated source, this checker and the authored context are
separately retained in the package's artifact record. Kernel reconstruction and
runtime event laws remain separate checks.

The accepted source grammar is deliberately narrow: these generated definitions
have one `:= do` separator and end before the next top-level definition or
documentation boundary. Internal blank lines are retained, so they cannot hide
added operations. Unexpected shapes are rejected. No file is modified.
-/
namespace ContextAudit

structure Context where
  original : String
  name : String
  parameters : String
  cuts : List (String × String × Nat)
  unusedClosure : Bool := false
  originalWrapped : Bool := false

private def deadlineName : String :=
  "zetesis_cpu.cancellation.Cancellation.poll.closure.Insts.CoreOpsFunctionFnOnceTupleSharedArcDeadlineOwnerBool"

private def readParameter : String :=
  "(read : core.sync.atomic.Atomic Bool (core.sync.atomic.private.Align1 U8) →\n    core.sync.atomic.Ordering → M Bool)"

private def contexts : List Context := [
  { original := deadlineName ++ ".call_once"
    name := "deadlineContext"
    parameters := readParameter
    cuts := [("core.sync.atomic.AtomicBoolAlign1U8.load", "read", 1)]
    unusedClosure := true
    originalWrapped := true },
  { original := "zetesis_cpu.cancellation.Cancellation.poll"
    name := "pollContext"
    parameters := readParameter ++
      "\n  (deadline : Option (alloc.sync.Arc zetesis_cpu.cancellation.DeadlineOwner) → M Bool)"
    cuts := [("core.sync.atomic.AtomicBoolAlign1U8.load", "read", 1),
      ("core.option.Option.is_some_and\n        " ++ deadlineName ++ "\n        o ()",
        "deadline o", 1)] },
  { original := "oracle.Work.tick"
    name := "tickContext"
    parameters := "(poll : zetesis_cpu.cancellation.Cancellation →\n    M (core.result.Result Unit zetesis_cpu.cancellation.Stop))"
    cuts := [("zetesis_cpu.cancellation.Cancellation.poll", "poll", 1)] },
  { original := "oracle.evaluate_loop.body"
    name := "evaluationContext"
    parameters := "(tick : oracle.Work → M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) × oracle.Work))"
    cuts := [("oracle.Work.tick", "tick", 1)] } ]


private def searchContexts : List Context := [
  { original := "oracle.failed_root_loop.body"
    name := "rootBody"
    parameters := "(tick : ∀ (_self : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    oracle.Work))"
    cuts := [("oracle.Work.tick", "tick", 1)] },
  { original := "oracle.select_atoms_loop.body"
    name := "selectionBody"
    parameters := "(tick : ∀ (_self : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    oracle.Work))"
    cuts := [("oracle.Work.tick", "tick", 1)] },
  { original := "oracle.advance_subset_loop.body"
    name := "carryBody"
    parameters := "(tick : ∀ (_self : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    oracle.Work))"
    cuts := [("oracle.Work.tick", "tick", 1)] },
  { original := "oracle.failed_root_loop"
    name := "rootLoop"
    parameters := "(loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)\n  (body : ∀ (_values : Slice Bool) (_iter : core.slice.iter.Iter Std.Usize)\n  (_work : oracle.Work),\n    M (ControlFlow ((core.slice.iter.Iter Std.Usize) × oracle.Work)\n    ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop) ×\n    oracle.Work)))"
    cuts := [("  loop\n", "  loopOp\n", 1),
      ("oracle.failed_root_loop.body", "body", 1)] },
  { original := "oracle.failed_root"
    name := "rootScan"
    parameters := "(runLoop : ∀ (_iter : core.slice.iter.Iter Std.Usize) (_values : Slice Bool)\n  (_work : oracle.Work),\n    M ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop)\n    × oracle.Work))"
    cuts := [("oracle.failed_root_loop", "runLoop", 1)] },
  { original := "oracle.select_atoms_loop"
    name := "selectionLoop"
    parameters := "(loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)\n  (body : ∀ (_candidate : theory.Interpretation) (_iter : core.ops.range.Range Std.Usize)\n  (_selected : alloc.vec.Vec Std.Usize) (_work : oracle.Work),\n    M (ControlFlow ((core.ops.range.Range Std.Usize) × (alloc.vec.Vec\n    Std.Usize) × oracle.Work) ((core.result.Result Unit\n    zetesis_cpu.cancellation.Stop) × (alloc.vec.Vec Std.Usize) ×\n    oracle.Work)))"
    cuts := [("  loop\n", "  loopOp\n", 1),
      ("oracle.select_atoms_loop.body", "body", 1)] },
  { original := "oracle.select_atoms"
    name := "selectAtoms"
    parameters := "(runLoop : ∀ (_iter : core.ops.range.Range Std.Usize) (_candidate : theory.Interpretation)\n  (_selected : alloc.vec.Vec Std.Usize) (_work : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    (alloc.vec.Vec Std.Usize) × oracle.Work))"
    cuts := [("oracle.select_atoms_loop", "runLoop", 1)] },
  { original := "oracle.advance_subset_loop"
    name := "carryLoop"
    parameters := "(loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)\n  (body : ∀ (_iter : core.slice.iter.Iter Std.Usize) (_subset : theory.Interpretation)\n  (_present : Std.Usize) (_work : oracle.Work),\n    M (ControlFlow ((core.slice.iter.Iter Std.Usize) ×\n    theory.Interpretation × Std.Usize × oracle.Work) ((core.result.Result\n    Unit zetesis_cpu.cancellation.Stop) × theory.Interpretation × Std.Usize\n    × oracle.Work)))"
    cuts := [("  loop\n", "  loopOp\n", 1),
      ("oracle.advance_subset_loop.body", "body", 1)] },
  { original := "oracle.advance_subset"
    name := "advanceSubset"
    parameters := "(runLoop : ∀ (_iter : core.slice.iter.Iter Std.Usize) (_subset : theory.Interpretation)\n  (_present : Std.Usize) (_work : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    theory.Interpretation × Std.Usize × oracle.Work))"
    cuts := [("oracle.advance_subset_loop", "runLoop", 1)] },
  { original := "oracle.evaluate_loop"
    name := "evaluationLoop"
    parameters := "(loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)\n  (body : ∀ (_interpretation : theory.Interpretation) (_frozen : Option (Slice Bool))\n  (_iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter\n  theory.Node)) (_output : alloc.vec.Vec Bool) (_l : oracle.Limits)\n  (_c : zetesis_cpu.cancellation.Cancellation) (_s : oracle.Statistics),\n    M (ControlFlow ((core.iter.adapters.enumerate.Enumerate\n    (core.slice.iter.Iter theory.Node)) × (alloc.vec.Vec Bool) ×\n    oracle.Limits × zetesis_cpu.cancellation.Cancellation ×\n    oracle.Statistics) ((core.result.Result Unit zetesis_cpu.cancellation.Stop)\n    × (alloc.vec.Vec Bool) × oracle.Work)))"
    cuts := [("  loop\n", "  loopOp\n", 1),
      ("oracle.evaluate_loop.body", "body", 1)] },
  { original := "oracle.evaluate"
    name := "evaluate"
    parameters := "(runLoop : ∀ (_iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter\n  theory.Node)) (_interpretation : theory.Interpretation)\n  (_frozen : Option (Slice Bool)) (_output : alloc.vec.Vec Bool)\n  (_l : oracle.Limits) (_c : zetesis_cpu.cancellation.Cancellation)\n  (_s : oracle.Statistics),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    (alloc.vec.Vec Bool) × oracle.Work))"
    cuts := [("oracle.evaluate_loop", "runLoop", 1)] },
  { original := "oracle.check_subset"
    name := "subsetQuery"
    parameters := "(poll : ∀ (_self : zetesis_cpu.cancellation.Cancellation),\n    M (core.result.Result Unit zetesis_cpu.cancellation.Stop))\n  (evaluate : ∀ (_program : theory.Theory) (_interpretation : theory.Interpretation)\n  (_frozen : Option (Slice Bool)) (_output : alloc.vec.Vec Bool)\n  (_work : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    (alloc.vec.Vec Bool) × oracle.Work))\n  (roots : ∀ (_program : theory.Theory) (_values : Slice Bool) (_work : oracle.Work),\n    M ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop)\n    × oracle.Work))"
    cuts := [("zetesis_cpu.cancellation.Cancellation.poll", "poll", 1),
      ("oracle.evaluate", "evaluate", 1),
      ("oracle.failed_root", "roots", 1)] },
  { original := "oracle.find_countermodel_loop.body"
    name := "searchBody"
    parameters := "(query : ∀ (_program : theory.Theory) (_subset : theory.Interpretation)\n  (_frozen : Slice Bool) (_values : alloc.vec.Vec Bool) (_work : oracle.Work),\n    M ((core.result.Result Bool zetesis_cpu.cancellation.Stop) ×\n    (alloc.vec.Vec Bool) × oracle.Work))\n  (advance : ∀ (_selected : Slice Std.Usize) (_subset : theory.Interpretation)\n  (_present : Std.Usize) (_work : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    theory.Interpretation × Std.Usize × oracle.Work))"
    cuts := [("oracle.check_subset", "query", 1),
      ("oracle.advance_subset", "advance", 1)] },
  { original := "oracle.find_countermodel_loop"
    name := "searchLoop"
    parameters := "(loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)\n  (body : ∀ (_program : theory.Theory) (_frozen : Slice Bool) (_selected : Slice Std.Usize)\n  (_subset : theory.Interpretation) (_values : alloc.vec.Vec Bool)\n  (_work : oracle.Work) (_present : Std.Usize),\n    M (ControlFlow (theory.Interpretation × (alloc.vec.Vec Bool) ×\n    oracle.Work × Std.Usize) (theory.Interpretation × (alloc.vec.Vec Bool) ×\n    oracle.Work × (core.result.Result Bool zetesis_cpu.cancellation.Stop))))"
    cuts := [("  loop\n", "  loopOp\n", 1),
      ("oracle.find_countermodel_loop.body", "body", 1)] },
  { original := "oracle.find_countermodel"
    name := "findCountermodel"
    parameters := "(runLoop : ∀ (_program : theory.Theory) (_frozen : Slice Bool) (_selected : Slice Std.Usize)\n  (_subset : theory.Interpretation) (_values : alloc.vec.Vec Bool)\n  (_work : oracle.Work) (_present : Std.Usize),\n    M (theory.Interpretation × (alloc.vec.Vec Bool) × oracle.Work ×\n    (core.result.Result Bool zetesis_cpu.cancellation.Stop)))"
    cuts := [("oracle.find_countermodel_loop", "runLoop", 1)] },
  { original := "oracle.reserve"
    name := "reserve"
    parameters := "(tryReserve : {T : Type} → (_allocator : Type) → alloc.vec.Vec T → Std.Usize →\n    M ((core.result.Result Unit alloc.collections.TryReserveError) × alloc.vec.Vec T))"
    cuts := [("alloc.vec.Vec.try_reserve_exact", "tryReserve", 1)] },
  { original := "oracle.check"
    name := "check"
    parameters := "(poll : ∀ (_self : zetesis_cpu.cancellation.Cancellation),\n    M (core.result.Result Unit zetesis_cpu.cancellation.Stop))\n  (reserve : ∀ (T : Type) (_count : Std.Usize),\n    M (core.result.Result (alloc.vec.Vec T) zetesis_cpu.cancellation.Stop))\n  (evaluate : ∀ (_program : theory.Theory) (_interpretation : theory.Interpretation)\n  (_frozen : Option (Slice Bool)) (_output : alloc.vec.Vec Bool)\n  (_work : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    (alloc.vec.Vec Bool) × oracle.Work))\n  (roots : ∀ (_program : theory.Theory) (_values : Slice Bool) (_work : oracle.Work),\n    M ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop)\n    × oracle.Work))\n  (selectAtoms : ∀ (_program : theory.Theory) (_candidate : theory.Interpretation)\n  (_selected : alloc.vec.Vec Std.Usize) (_work : oracle.Work),\n    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×\n    (alloc.vec.Vec Std.Usize) × oracle.Work))\n  (search : ∀ (_program : theory.Theory) (_frozen : Slice Bool) (_selected : Slice Std.Usize)\n  (_subset : theory.Interpretation) (_values : alloc.vec.Vec Bool)\n  (_work : oracle.Work),\n    M ((core.result.Result Bool zetesis_cpu.cancellation.Stop) ×\n    theory.Interpretation × (alloc.vec.Vec Bool) × oracle.Work))"
    cuts := [("zetesis_cpu.cancellation.Cancellation.poll", "poll", 1),
      ("oracle.reserve", "reserve", 4),
      ("oracle.evaluate", "evaluate", 1),
      ("oracle.failed_root", "roots", 1),
      ("oracle.select_atoms", "selectAtoms", 1),
      ("oracle.find_countermodel", "search", 1)] },
  { original := "checked.check_interpretation"
    name := "checkInterpretation"
    parameters := "(runCheck : ∀ (_program : theory.Theory) (_candidate : theory.Interpretation)\n  (_limits : oracle.Limits)\n  (_cancellation : zetesis_cpu.cancellation.Cancellation),\n    M (core.result.Result oracle.Check zetesis_cpu.cancellation.Stop))"
    cuts := [("oracle.check", "runCheck", 1)] } ]

private def coreScaffold : String :=
  "import RuntimeEffects\nimport Control\n\nopen Aeneas Aeneas.Std Result ControlFlow Error\nopen ZetesisExtract\n\n/-!\n# Restricted monadic contexts for returning effects\n\nThe four contexts below generalize exact generated definitions. Only their\nresult monad and named effect call sites change; pure operations remain imported\nbackend calls. ContextAudit checks exact headers, call counts, body text and\nreversal separately from the kernel reconstruction laws. This audited source\ngeneralization is an explicit extraction boundary. Whole-loop runtime\ncorrespondence requires additional returning-event projection laws.\n-/\nnamespace RuntimeContexts\n\nvariable {M : Type → Type} [Monad M] [MonadLiftT Result M]\n\n"

private def searchScaffold : String :=
  "import RuntimeContexts\n\nopen Aeneas Aeneas.Std Result ControlFlow Error\nopen ZetesisExtract\n\n/-!\n# Effect contexts for the reference checker\n\nThese definitions generalize only the listed phase calls in the exact generated\nreference checker. Pure scalar, vector, iterator and ownership calls are retained.\nThe separate source checker requires exact call counts and reversible bodies;\nthe reconstruction laws recover each original generated definition. Neither\ncondition by itself establishes correspondence to changing runtime observations.\n-/\nnamespace CheckerContexts\n\nvariable {M : Type → Type} [Monad M] [MonadLiftT Result M]\n\n"

private def require (condition : Bool) (message : String) : Except String Unit :=
  if condition then .ok () else .error message

/-- A selected token must occur exactly once; replacement preserves all other
text. The same operation is used in reverse to detect changed call counts. -/
private def replaceOnce (text before after : String) : Except String String := do
  require (!before.isEmpty) "an empty replacement token is invalid"
  match text.splitOn before with
  | [beforeText, afterText] => pure (beforeText ++ after ++ afterText)
  | pieces => throw s!"expected one occurrence of {repr before}; found {pieces.length - 1}"

private def replaceCount (text before after : String) (count : Nat) : Except String String := do
  require (!before.isEmpty && count > 0) "a cut needs a nonempty token and positive count"
  let pieces := text.splitOn before
  require (pieces.length == count + 1)
    s!"expected {count} occurrences of {repr before}; found {pieces.length - 1}"
  pure (String.intercalate after pieces)

private def definitionMarker (name : String) (wrapped : Bool := false) : String :=
  (if wrapped then "def\n  " else "def ") ++ name ++ "\n"

private def definition (text name : String) (wrapped : Bool := false) : Except String String := do
  let marker := definitionMarker name wrapped
  match text.splitOn marker with
  | [_, rest] =>
      let candidates := ["\n\ndef ", "\n\n/--", "\n\nend "].filterMap fun boundary =>
        match rest.splitOn boundary with
        | content :: _ :: _ => some content
        | _ => none
      match candidates with
      | first :: others =>
          let content := others.foldl (fun current candidate =>
            if candidate.utf8ByteSize < current.utf8ByteSize then candidate else current) first
          pure (marker ++ content)
      | [] => throw s!"missing next definition boundary after {name}"
  | pieces => throw s!"expected one definition {name}; found {pieces.length - 1}"

private def parts (text : String) : Except String (String × String) := do
  match text.splitOn "\n  := do\n" with
  | [header, body] => pure (header, body)
  | _ => throw "expected exactly one generated do-body separator"

private def checkContext (source retained : String) (context : Context) : Except String Unit := do
  let original ← definition source context.original context.originalWrapped
  let actual ← definition retained context.name
  let (header, body) ← parts original
  let (actualHeader, actualBody) ← parts actual
  let renamed ← replaceOnce header (definitionMarker context.original context.originalWrapped)
    ("def " ++ context.name ++ "\n  " ++ context.parameters ++ "\n")
  let monadic ← replaceOnce renamed ":\n  Result " ":\n  M "
  let expectedHeader ← if context.unusedClosure then
      replaceOnce monadic "(c :" "(_c :"
    else pure monadic
  require (expectedHeader == actualHeader) s!"{context.name}: header differs"
  let expectedBody ← context.cuts.foldlM (fun current cut =>
    replaceCount current cut.1 cut.2.1 cut.2.2) body
  require (expectedBody == actualBody) s!"{context.name}: body differs"
  let restored ← context.cuts.reverse.foldlM (fun current cut =>
    replaceCount current cut.2.1 cut.1 cut.2.2) actualBody
  require (restored == body) s!"{context.name}: reversed body differs"

private def checkGroup (source retained scaffold : String) (group : List Context) :
    Except String Unit := do
  group.forM (checkContext source retained)
  let definitions ← group.mapM (fun context => definition retained context.name)
  let checkedRegion := scaffold ++ String.intercalate "\n\n" definitions ++ "\n\n"
  require (retained.startsWith checkedRegion)
    "context scaffold or contiguous definition region differs"

/-- Check the four primitive contexts, including their exact surrounding imports,
namespace and instance parameters. No additional declaration may occur in the
checked region. Proofs following that region are checked by the Lean kernel. -/
def check (source retained : String) : Except String Unit :=
  checkGroup source retained coreScaffold contexts

/-- Check every remaining loop, phase and public-wrapper context with its exact
reviewed scaffold. Public check retains four ordered reservation call sites. -/
def checkSearch (source retained : String) : Except String Unit :=
  checkGroup source retained searchScaffold searchContexts

private def expectRejection (source mutated label : String) : Except String Unit := do
  match check source mutated with
  | .ok () => throw s!"negative check unexpectedly accepted: {label}"
  | .error _ => pure ()

/-- Exercise changes that fixed-handler reconstruction alone must not authorize:
an extra call, different read order, a changed branch and a missing context. -/
def negativeChecks (source retained : String) : Except String Unit := do
  check source retained
  let extra ← replaceOnce retained
    "  let r ← poll self.cancellation\n"
    "  let first ← poll self.cancellation\n\n  let r ← poll self.cancellation\n"
  expectRejection source extra "extra poll call"
  let pollDefinition ← definition retained "pollContext"
  let (pollHeader, _) ← parts pollDefinition
  let reorderedBody :=
    "  let o ← core.option.Option.as_ref self.deadline\n" ++
    "  let b1 ← deadline o\n" ++
    "  let a ← alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref Global self.cancelled\n" ++
    "  let b ← read a core.sync.atomic.Ordering.Relaxed\n" ++
    "  if b\n" ++
    "  then ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.Cancelled)\n" ++
    "  else\n" ++
    "    if b1\n" ++
    "    then ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.Deadline)\n" ++
    "    else ok (core.result.Result.Ok ())"
  let reordered ← replaceOnce retained pollDefinition
    (pollHeader ++ "\n  := do\n" ++ reorderedBody)
  expectRejection source reordered "deadline before cancellation"
  let changed ← replaceOnce retained
    "  then ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.Cancelled)\n"
    "  then ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.Deadline)\n"
  expectRejection source changed "changed cancellation branch"
  let missing ← replaceOnce retained "def pollContext\n" "def absentContext\n"
  expectRejection source missing "missing poll context"

  let scaffold ← replaceOnce retained "namespace RuntimeContexts\n"
    "namespace RuntimeContexts\n\nlocal notation \"False\" => True\n"
  expectRejection source scaffold "changed namespace scaffold"

/-- A duplicated public-wrapper reservation must be rejected even when the
other phase calls and source branches remain unchanged. -/
def negativeSearchCheck (source retained : String) : Except String Unit := do
  checkSearch source retained
  let repeated ← replaceOnce retained "      let r2 ← reserve Bool i\n"
    "      let ignored ← reserve Bool i\n      let r2 ← reserve Bool i\n"
  match checkSearch source repeated with
  | .ok () => throw "negative check unexpectedly accepted: duplicate reservation"
  | .error _ => pure ()

end ContextAudit

/-- Run from the refinement package with the generated and context source paths.
`--self-test` additionally checks six intentionally changed in-memory inputs. -/
def main (arguments : List String) : IO UInt32 := do
  let (selfTest, paths) := match arguments with
    | "--self-test" :: rest => (true, rest)
    | rest => (false, rest)
  match paths with
  | [generatedPath, contextPath, searchPath] =>
      let generated ← IO.FS.readFile generatedPath
      let retained ← IO.FS.readFile contextPath
      let search ← IO.FS.readFile searchPath
      let result : Except String Unit := do
        if selfTest then
          ContextAudit.negativeChecks generated retained
          ContextAudit.negativeSearchCheck generated search
        else
          ContextAudit.check generated retained
          ContextAudit.checkSearch generated search
      match result with
      | .ok () =>
          IO.println (if selfTest then
            "All context source checks and six negative checks passed."
            else "All 22 context scaffolds, headers, bodies, call counts and reversals passed.")
          pure 0
      | .error message =>
          IO.eprintln message
          pure 1
  | _ =>
      IO.eprintln "usage: ContextAudit.lean [--self-test] Evaluator/Funs.lean RuntimeContexts.lean CheckerContexts.lean"
      pure 2
