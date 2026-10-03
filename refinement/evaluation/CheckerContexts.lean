import RuntimeContexts

open Aeneas Aeneas.Std Result ControlFlow Error
open ZetesisExtract

/-!
# Effect contexts for the reference checker

These definitions generalize only the listed phase calls in the exact generated
reference checker. Pure scalar, vector, iterator and ownership calls are retained.
The separate source checker requires exact call counts and reversible bodies;
the reconstruction laws recover each original generated definition. Neither
condition by itself establishes correspondence to changing runtime observations.
-/
namespace CheckerContexts

variable {M : Type → Type} [Monad M] [MonadLiftT Result M]

def rootBody
  (tick : ∀ (_self : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    oracle.Work))
  (values : Slice Bool) (iter : core.slice.iter.Iter Std.Usize)
  (work : oracle.Work) :
  M (ControlFlow ((core.slice.iter.Iter Std.Usize) × oracle.Work)
    ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop) ×
    oracle.Work))
  := do
  let (o, iter1) ← core.slice.iter.IteratorSliceIter.next iter
  match o with
  | none => ok (done (core.result.Result.Ok none, work))
  | some root =>
    let (r, work1) ← tick work
    let cf ← core.result.Result.Insts.CoreOpsTry.branch r
    match cf with
    | core.ops.control_flow.ControlFlow.Continue _ =>
      let b ← Slice.index_usize values root
      if b
      then ok (cont (iter1, work1))
      else ok (done (core.result.Result.Ok o, work1))
    | core.ops.control_flow.ControlFlow.Break residual =>
      let r1 ←
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
          (Option Std.Usize) (core.convert.FromSame
          zetesis_cpu.cancellation.Stop) residual
      ok (done (r1, work1))

def selectionBody
  (tick : ∀ (_self : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    oracle.Work))
  (candidate : theory.Interpretation) (iter : core.ops.range.Range Std.Usize)
  (selected : alloc.vec.Vec Std.Usize) (work : oracle.Work) :
  M (ControlFlow ((core.ops.range.Range Std.Usize) × (alloc.vec.Vec
    Std.Usize) × oracle.Work) ((core.result.Result Unit
    zetesis_cpu.cancellation.Stop) × (alloc.vec.Vec Std.Usize) ×
    oracle.Work))
  := do
  let (o, iter1) ←
    core.iter.range.IteratorRange.next core.iter.range.StepUsize iter
  match o with
  | none => ok (done (core.result.Result.Ok (), selected, work))
  | some atom =>
    let (r, work1) ← tick work
    let cf ← core.result.Result.Insts.CoreOpsTry.branch r
    match cf with
    | core.ops.control_flow.ControlFlow.Continue _ =>
      let b ← theory.Interpretation.contains candidate atom
      if b
      then
        let selected1 ← alloc.vec.Vec.push selected atom
        ok (cont (iter1, selected1, work1))
      else ok (cont (iter1, selected, work1))
    | core.ops.control_flow.ControlFlow.Break residual =>
      let r1 ←
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
          Unit (core.convert.FromSame zetesis_cpu.cancellation.Stop) residual
      ok (done (r1, selected, work1))

def carryBody
  (tick : ∀ (_self : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    oracle.Work))
  (iter : core.slice.iter.Iter Std.Usize) (subset : theory.Interpretation)
  (present : Std.Usize) (work : oracle.Work) :
  M (ControlFlow ((core.slice.iter.Iter Std.Usize) ×
    theory.Interpretation × Std.Usize × oracle.Work) ((core.result.Result
    Unit zetesis_cpu.cancellation.Stop) × theory.Interpretation × Std.Usize
    × oracle.Work))
  := do
  let (o, iter1) ← core.slice.iter.IteratorSliceIter.next iter
  match o with
  | none => ok (done (core.result.Result.Ok (), subset, present, work))
  | some atom =>
    let (r, work1) ← tick work
    let cf ← core.result.Result.Insts.CoreOpsTry.branch r
    match cf with
    | core.ops.control_flow.ControlFlow.Continue _ =>
      let i ← atom / 64#usize
      let (packed, index_mut_back) ←
        alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice Std.U64)
          subset.words i
      let i1 ← atom % 64#usize
      let bit ← 1#u64 <<< i1
      let i2 ← lift (packed &&& bit)
      if i2 = 0#u64
      then
        let packed1 ← lift (packed ||| bit)
        let present1 ← present + 1#usize
        let v := index_mut_back packed1
        ok (done (core.result.Result.Ok (), { subset with words := v },
          present1, work1))
      else
        let i3 ← lift (~~~ bit)
        let packed1 ← lift (packed &&& i3)
        let present1 ← present - 1#usize
        let v := index_mut_back packed1
        ok (cont (iter1, { subset with words := v }, present1, work1))
    | core.ops.control_flow.ControlFlow.Break residual =>
      let r1 ←
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
          Unit (core.convert.FromSame zetesis_cpu.cancellation.Stop) residual
      ok (done (r1, subset, present, work1))

def rootLoop
  (loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)
  (body : ∀ (_values : Slice Bool) (_iter : core.slice.iter.Iter Std.Usize)
  (_work : oracle.Work),
    M (ControlFlow ((core.slice.iter.Iter Std.Usize) × oracle.Work)
    ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop) ×
    oracle.Work)))
  (iter : core.slice.iter.Iter Std.Usize) (values : Slice Bool)
  (work : oracle.Work) :
  M ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop)
    × oracle.Work)
  := do
  loopOp
    (fun (iter1, work1) => body values iter1 work1)
    (iter, work)

def rootScan
  (runLoop : ∀ (_iter : core.slice.iter.Iter Std.Usize) (_values : Slice Bool)
  (_work : oracle.Work),
    M ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop)
    × oracle.Work))
  (program : theory.Theory) (values : Slice Bool) (work : oracle.Work) :
  M ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop)
    × oracle.Work)
  := do
  let s ← theory.Theory.roots program
  let iter ←
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter s
  runLoop iter values work

def selectionLoop
  (loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)
  (body : ∀ (_candidate : theory.Interpretation) (_iter : core.ops.range.Range Std.Usize)
  (_selected : alloc.vec.Vec Std.Usize) (_work : oracle.Work),
    M (ControlFlow ((core.ops.range.Range Std.Usize) × (alloc.vec.Vec
    Std.Usize) × oracle.Work) ((core.result.Result Unit
    zetesis_cpu.cancellation.Stop) × (alloc.vec.Vec Std.Usize) ×
    oracle.Work)))
  (iter : core.ops.range.Range Std.Usize) (candidate : theory.Interpretation)
  (selected : alloc.vec.Vec Std.Usize) (work : oracle.Work) :
  M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Std.Usize) × oracle.Work)
  := do
  loopOp
    (fun (iter1, selected1, work1) => body candidate
      iter1 selected1 work1)
    (iter, selected, work)

def selectAtoms
  (runLoop : ∀ (_iter : core.ops.range.Range Std.Usize) (_candidate : theory.Interpretation)
  (_selected : alloc.vec.Vec Std.Usize) (_work : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Std.Usize) × oracle.Work))
  (program : theory.Theory) (candidate : theory.Interpretation)
  (selected : alloc.vec.Vec Std.Usize) (work : oracle.Work) :
  M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Std.Usize) × oracle.Work)
  := do
  let i ← theory.Theory.atom_count program
  runLoop { start := 0#usize, «end» := i } candidate
    selected work

def carryLoop
  (loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)
  (body : ∀ (_iter : core.slice.iter.Iter Std.Usize) (_subset : theory.Interpretation)
  (_present : Std.Usize) (_work : oracle.Work),
    M (ControlFlow ((core.slice.iter.Iter Std.Usize) ×
    theory.Interpretation × Std.Usize × oracle.Work) ((core.result.Result
    Unit zetesis_cpu.cancellation.Stop) × theory.Interpretation × Std.Usize
    × oracle.Work)))
  (iter : core.slice.iter.Iter Std.Usize) (subset : theory.Interpretation)
  (present : Std.Usize) (work : oracle.Work) :
  M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    theory.Interpretation × Std.Usize × oracle.Work)
  := do
  loopOp
    (fun (iter1, subset1, present1, work1) => body
      iter1 subset1 present1 work1)
    (iter, subset, present, work)

def advanceSubset
  (runLoop : ∀ (_iter : core.slice.iter.Iter Std.Usize) (_subset : theory.Interpretation)
  (_present : Std.Usize) (_work : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    theory.Interpretation × Std.Usize × oracle.Work))
  (selected : Slice Std.Usize) (subset : theory.Interpretation)
  (present : Std.Usize) (work : oracle.Work) :
  M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    theory.Interpretation × Std.Usize × oracle.Work)
  := do
  let iter ←
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter
      selected
  runLoop iter subset present work

def evaluationLoop
  (loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)
  (body : ∀ (_interpretation : theory.Interpretation) (_frozen : Option (Slice Bool))
  (_iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter
  theory.Node)) (_output : alloc.vec.Vec Bool) (_l : oracle.Limits)
  (_c : zetesis_cpu.cancellation.Cancellation) (_s : oracle.Statistics),
    M (ControlFlow ((core.iter.adapters.enumerate.Enumerate
    (core.slice.iter.Iter theory.Node)) × (alloc.vec.Vec Bool) ×
    oracle.Limits × zetesis_cpu.cancellation.Cancellation ×
    oracle.Statistics) ((core.result.Result Unit zetesis_cpu.cancellation.Stop)
    × (alloc.vec.Vec Bool) × oracle.Work)))
  (iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter
  theory.Node)) (interpretation : theory.Interpretation)
  (frozen : Option (Slice Bool)) (output : alloc.vec.Vec Bool)
  (l : oracle.Limits) (c : zetesis_cpu.cancellation.Cancellation)
  (s : oracle.Statistics) :
  M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Bool) × oracle.Work)
  := do
  loopOp
    (fun (iter1, output1, l1, c1, s1) => body
      interpretation frozen iter1 output1 l1 c1 s1)
    (iter, output, l, c, s)

def evaluate
  (runLoop : ∀ (_iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter
  theory.Node)) (_interpretation : theory.Interpretation)
  (_frozen : Option (Slice Bool)) (_output : alloc.vec.Vec Bool)
  (_l : oracle.Limits) (_c : zetesis_cpu.cancellation.Cancellation)
  (_s : oracle.Statistics),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Bool) × oracle.Work))
  (program : theory.Theory) (interpretation : theory.Interpretation)
  (frozen : Option (Slice Bool)) (output : alloc.vec.Vec Bool)
  (work : oracle.Work) :
  M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Bool) × oracle.Work)
  := do
  let output1 ← alloc.vec.Vec.clear Global output
  let s ← theory.Theory.nodes program
  let i ← core.slice.Slice.iter s
  let iter ←
    core.iter.traits.iterator.Iterator.enumerate.trait_default
      (core.iter.traits.iterator.IteratorSliceIter theory.Node) i
  runLoop iter interpretation frozen output1 work.limits
    work.cancellation work.statistics

def subsetQuery
  (poll : ∀ (_self : zetesis_cpu.cancellation.Cancellation),
    M (core.result.Result Unit zetesis_cpu.cancellation.Stop))
  (evaluate : ∀ (_program : theory.Theory) (_interpretation : theory.Interpretation)
  (_frozen : Option (Slice Bool)) (_output : alloc.vec.Vec Bool)
  (_work : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Bool) × oracle.Work))
  (roots : ∀ (_program : theory.Theory) (_values : Slice Bool) (_work : oracle.Work),
    M ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop)
    × oracle.Work))
  (program : theory.Theory) (subset : theory.Interpretation)
  (frozen : Slice Bool) (values : alloc.vec.Vec Bool) (work : oracle.Work) :
  M ((core.result.Result Bool zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Bool) × oracle.Work)
  := do
  let r ← poll work.cancellation
  let cf ← core.result.Result.Insts.CoreOpsTry.branch r
  match cf with
  | core.ops.control_flow.ControlFlow.Continue _ =>
    if work.statistics.subsets >= work.limits.max_subsets
    then
      ok (core.result.Result.Err zetesis_cpu.cancellation.Stop.CandidateLimit,
        values, work)
    else
      let i ← work.statistics.subsets + 1#u64
      let (r1, values1, work1) ←
        evaluate program subset (some frozen) values
          { work with statistics := { work.statistics with subsets := i } }
      let cf1 ← core.result.Result.Insts.CoreOpsTry.branch r1
      match cf1 with
      | core.ops.control_flow.ControlFlow.Continue _ =>
        let s := alloc.vec.Vec.deref values1
        let (r2, work2) ← roots program s work1
        let cf2 ← core.result.Result.Insts.CoreOpsTry.branch r2
        match cf2 with
        | core.ops.control_flow.ControlFlow.Continue val =>
          let b := core.option.Option.is_none val
          ok (core.result.Result.Ok b, values1, work2)
        | core.ops.control_flow.ControlFlow.Break residual =>
          let r3 ←
            core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
              Bool (core.convert.FromSame zetesis_cpu.cancellation.Stop)
              residual
          ok (r3, values1, work2)
      | core.ops.control_flow.ControlFlow.Break residual =>
        let r2 ←
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
            Bool (core.convert.FromSame zetesis_cpu.cancellation.Stop) residual
        ok (r2, values1, work1)
  | core.ops.control_flow.ControlFlow.Break residual =>
    let r1 ←
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
        Bool (core.convert.FromSame zetesis_cpu.cancellation.Stop) residual
    ok (r1, values, work)

def searchBody
  (query : ∀ (_program : theory.Theory) (_subset : theory.Interpretation)
  (_frozen : Slice Bool) (_values : alloc.vec.Vec Bool) (_work : oracle.Work),
    M ((core.result.Result Bool zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Bool) × oracle.Work))
  (advance : ∀ (_selected : Slice Std.Usize) (_subset : theory.Interpretation)
  (_present : Std.Usize) (_work : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    theory.Interpretation × Std.Usize × oracle.Work))
  (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Std.Usize)
  (subset : theory.Interpretation) (values : alloc.vec.Vec Bool)
  (work : oracle.Work) (present : Std.Usize) :
  M (ControlFlow (theory.Interpretation × (alloc.vec.Vec Bool) ×
    oracle.Work × Std.Usize) (theory.Interpretation × (alloc.vec.Vec Bool) ×
    oracle.Work × (core.result.Result Bool zetesis_cpu.cancellation.Stop)))
  := do
  let i := Slice.len selected
  if present < i
  then
    let (countermodel, values1, work1) ←
      query program subset frozen values work
    match countermodel with
    | core.result.Result.Ok b =>
      if b
      then ok (done (subset, values1, work1, countermodel))
      else
        let (r, subset1, present1, work2) ←
          advance selected subset present work1
        match r with
        | core.result.Result.Ok _ =>
          ok (cont (subset1, values1, work2, present1))
        | core.result.Result.Err stop =>
          ok (done (subset1, values1, work2, core.result.Result.Err stop))
    | core.result.Result.Err _ =>
      ok (done (subset, values1, work1, countermodel))
  else ok (done (subset, values, work, core.result.Result.Ok false))

def searchLoop
  (loopOp : {State Out : Type} → (State → M (ControlFlow State Out)) → State → M Out)
  (body : ∀ (_program : theory.Theory) (_frozen : Slice Bool) (_selected : Slice Std.Usize)
  (_subset : theory.Interpretation) (_values : alloc.vec.Vec Bool)
  (_work : oracle.Work) (_present : Std.Usize),
    M (ControlFlow (theory.Interpretation × (alloc.vec.Vec Bool) ×
    oracle.Work × Std.Usize) (theory.Interpretation × (alloc.vec.Vec Bool) ×
    oracle.Work × (core.result.Result Bool zetesis_cpu.cancellation.Stop))))
  (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Std.Usize)
  (subset : theory.Interpretation) (values : alloc.vec.Vec Bool)
  (work : oracle.Work) (present : Std.Usize) :
  M (theory.Interpretation × (alloc.vec.Vec Bool) × oracle.Work ×
    (core.result.Result Bool zetesis_cpu.cancellation.Stop))
  := do
  loopOp
    (fun (subset1, values1, work1, present1) =>
      body program frozen selected subset1
      values1 work1 present1)
    (subset, values, work, present)

def findCountermodel
  (runLoop : ∀ (_program : theory.Theory) (_frozen : Slice Bool) (_selected : Slice Std.Usize)
  (_subset : theory.Interpretation) (_values : alloc.vec.Vec Bool)
  (_work : oracle.Work) (_present : Std.Usize),
    M (theory.Interpretation × (alloc.vec.Vec Bool) × oracle.Work ×
    (core.result.Result Bool zetesis_cpu.cancellation.Stop)))
  (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Std.Usize)
  (subset : theory.Interpretation) (values : alloc.vec.Vec Bool)
  (work : oracle.Work) :
  M ((core.result.Result Bool zetesis_cpu.cancellation.Stop) ×
    theory.Interpretation × (alloc.vec.Vec Bool) × oracle.Work)
  := do
  let (subset1, values1, work1, countermodel) ←
    runLoop program frozen selected subset values work
      0#usize
  ok (countermodel, subset1, values1, work1)

def reserve
  (tryReserve : {T : Type} → (_allocator : Type) → alloc.vec.Vec T → Std.Usize →
    M ((core.result.Result Unit alloc.collections.TryReserveError) × alloc.vec.Vec T))
  (T : Type) (count : Std.Usize) :
  M (core.result.Result (alloc.vec.Vec T) zetesis_cpu.cancellation.Stop)
  := do
  let (r, vector) ←
    tryReserve Global (alloc.vec.Vec.new T) count
  let r1 ←
    core.result.Result.map_err
      (oracle.reserve.closure.Insts.CoreOpsFunctionFnOnceTupleTryReserveErrorStop
      T) r ()
  let cf ← core.result.Result.Insts.CoreOpsTry.branch r1
  match cf with
  | core.ops.control_flow.ControlFlow.Continue _ =>
    ok (core.result.Result.Ok vector)
  | core.ops.control_flow.ControlFlow.Break residual =>
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
      (alloc.vec.Vec T) (core.convert.FromSame zetesis_cpu.cancellation.Stop)
      residual

def check
  (poll : ∀ (_self : zetesis_cpu.cancellation.Cancellation),
    M (core.result.Result Unit zetesis_cpu.cancellation.Stop))
  (reserve : ∀ (T : Type) (_count : Std.Usize),
    M (core.result.Result (alloc.vec.Vec T) zetesis_cpu.cancellation.Stop))
  (evaluate : ∀ (_program : theory.Theory) (_interpretation : theory.Interpretation)
  (_frozen : Option (Slice Bool)) (_output : alloc.vec.Vec Bool)
  (_work : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Bool) × oracle.Work))
  (roots : ∀ (_program : theory.Theory) (_values : Slice Bool) (_work : oracle.Work),
    M ((core.result.Result (Option Std.Usize) zetesis_cpu.cancellation.Stop)
    × oracle.Work))
  (selectAtoms : ∀ (_program : theory.Theory) (_candidate : theory.Interpretation)
  (_selected : alloc.vec.Vec Std.Usize) (_work : oracle.Work),
    M ((core.result.Result Unit zetesis_cpu.cancellation.Stop) ×
    (alloc.vec.Vec Std.Usize) × oracle.Work))
  (search : ∀ (_program : theory.Theory) (_frozen : Slice Bool) (_selected : Slice Std.Usize)
  (_subset : theory.Interpretation) (_values : alloc.vec.Vec Bool)
  (_work : oracle.Work),
    M ((core.result.Result Bool zetesis_cpu.cancellation.Stop) ×
    theory.Interpretation × (alloc.vec.Vec Bool) × oracle.Work))
  (program : theory.Theory) (candidate : theory.Interpretation)
  (limits : oracle.Limits)
  (cancellation : zetesis_cpu.cancellation.Cancellation) :
  M (core.result.Result oracle.Check zetesis_cpu.cancellation.Stop)
  := do
  let r ← oracle.identities program candidate
  let cf ← core.result.Result.Insts.CoreOpsTry.branch r
  match cf with
  | core.ops.control_flow.ControlFlow.Continue _ =>
    let r1 ← poll cancellation
    let cf1 ← core.result.Result.Insts.CoreOpsTry.branch r1
    match cf1 with
    | core.ops.control_flow.ControlFlow.Continue _ =>
      let s ← oracle.Statistics.Insts.CoreDefaultDefault.default
      let s1 ← theory.Theory.nodes program
      let i := Slice.len s1
      let r2 ← reserve Bool i
      let cf2 ← core.result.Result.Insts.CoreOpsTry.branch r2
      match cf2 with
      | core.ops.control_flow.ControlFlow.Continue val =>
        let (r3, val1, work) ←
          evaluate program candidate none val
            { limits, cancellation, statistics := s }
        let cf3 ← core.result.Result.Insts.CoreOpsTry.branch r3
        match cf3 with
        | core.ops.control_flow.ControlFlow.Continue _ =>
          let s2 := alloc.vec.Vec.deref val1
          let (r4, work1) ← roots program s2 work
          let cf4 ← core.result.Result.Insts.CoreOpsTry.branch r4
          match cf4 with
          | core.ops.control_flow.ControlFlow.Continue val2 =>
            match val2 with
            | none =>
              let i1 ← theory.Theory.atom_count program
              let r5 ← reserve Std.Usize i1
              let cf5 ← core.result.Result.Insts.CoreOpsTry.branch r5
              match cf5 with
              | core.ops.control_flow.ControlFlow.Continue val3 =>
                let (r6, val4, work2) ←
                  selectAtoms program candidate val3 work1
                let cf6 ← core.result.Result.Insts.CoreOpsTry.branch r6
                match cf6 with
                | core.ops.control_flow.ControlFlow.Continue _ =>
                  let i2 := alloc.vec.Vec.len candidate.words
                  let r7 ← reserve Std.U64 i2
                  let cf7 ← core.result.Result.Insts.CoreOpsTry.branch r7
                  match cf7 with
                  | core.ops.control_flow.ControlFlow.Continue val5 =>
                    let i3 := alloc.vec.Vec.len candidate.words
                    let val6 ←
                      alloc.vec.Vec.resize core.clone.CloneU64 val5 i3 0#u64
                    let t ← theory.Theory.Insts.CoreCloneClone.clone program
                    let i4 := Slice.len s1
                    let r8 ← reserve Bool i4
                    let cf8 ← core.result.Result.Insts.CoreOpsTry.branch r8
                    match cf8 with
                    | core.ops.control_flow.ControlFlow.Continue val7 =>
                      let s3 := alloc.vec.Vec.deref val1
                      let s4 := alloc.vec.Vec.deref val4
                      let (r9, subset, _, work3) ←
                        search program s3 s4
                          { theory := t, words := val6 } val7 work2
                      let cf9 ← core.result.Result.Insts.CoreOpsTry.branch r9
                      match cf9 with
                      | core.ops.control_flow.ControlFlow.Continue val8 =>
                        if val8
                        then
                          ok (core.result.Result.Ok
                            {
                              verdict := (oracle.Verdict.NonMinimal subset),
                              statistics := work3.statistics
                            })
                        else
                          ok (core.result.Result.Ok
                            {
                              verdict := oracle.Verdict.Stable,
                              statistics := work3.statistics
                            })
                      | core.ops.control_flow.ControlFlow.Break residual =>
                        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
                          oracle.Check (core.convert.FromSame
                          zetesis_cpu.cancellation.Stop) residual
                    | core.ops.control_flow.ControlFlow.Break residual =>
                      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
                        oracle.Check (core.convert.FromSame
                        zetesis_cpu.cancellation.Stop) residual
                  | core.ops.control_flow.ControlFlow.Break residual =>
                    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
                      oracle.Check (core.convert.FromSame
                      zetesis_cpu.cancellation.Stop) residual
                | core.ops.control_flow.ControlFlow.Break residual =>
                  core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
                    oracle.Check (core.convert.FromSame
                    zetesis_cpu.cancellation.Stop) residual
              | core.ops.control_flow.ControlFlow.Break residual =>
                core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
                  oracle.Check (core.convert.FromSame
                  zetesis_cpu.cancellation.Stop) residual
            | some root =>
              ok (core.result.Result.Ok
                {
                  verdict := (oracle.Verdict.NotModel root),
                  statistics := work1.statistics
                })
          | core.ops.control_flow.ControlFlow.Break residual =>
            core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
              oracle.Check (core.convert.FromSame
              zetesis_cpu.cancellation.Stop) residual
        | core.ops.control_flow.ControlFlow.Break residual =>
          core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
            oracle.Check (core.convert.FromSame zetesis_cpu.cancellation.Stop)
            residual
      | core.ops.control_flow.ControlFlow.Break residual =>
        core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
          oracle.Check (core.convert.FromSame zetesis_cpu.cancellation.Stop)
          residual
    | core.ops.control_flow.ControlFlow.Break residual =>
      core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
        oracle.Check (core.convert.FromSame zetesis_cpu.cancellation.Stop)
        residual
  | core.ops.control_flow.ControlFlow.Break residual =>
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
      oracle.Check (core.convert.FromSame zetesis_cpu.cancellation.Stop)
      residual

def checkInterpretation
  (runCheck : ∀ (_program : theory.Theory) (_candidate : theory.Interpretation)
  (_limits : oracle.Limits)
  (_cancellation : zetesis_cpu.cancellation.Cancellation),
    M (core.result.Result oracle.Check zetesis_cpu.cancellation.Stop))
  (candidate : theory.Interpretation) (limits : oracle.Limits)
  (cancellation : zetesis_cpu.cancellation.Cancellation) :
  M (core.result.Result checked.CheckedInterpretation
    zetesis_cpu.cancellation.Stop)
  := do
  let t ← theory.Interpretation.impl.theory candidate
  let r ← runCheck t candidate limits cancellation
  let cf ← core.result.Result.Insts.CoreOpsTry.branch r
  match cf with
  | core.ops.control_flow.ControlFlow.Continue val =>
    ok (core.result.Result.Ok { candidate, check := val })
  | core.ops.control_flow.ControlFlow.Break residual =>
    core.result.Result.Insts.CoreOpsTryTraitFromResidualResultInfallible.from_residual
      checked.CheckedInterpretation (core.convert.FromSame
      zetesis_cpu.cancellation.Stop) residual

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem rootBody_reconstruct
  (values : Slice Bool) (iter : core.slice.iter.Iter Std.Usize)
  (work : oracle.Work) :
    rootBody (M := Result) oracle.Work.tick values iter work =
      oracle.failed_root_loop.body values iter work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem selectionBody_reconstruct
  (candidate : theory.Interpretation) (iter : core.ops.range.Range Std.Usize)
  (selected : alloc.vec.Vec Std.Usize) (work : oracle.Work) :
    selectionBody (M := Result) oracle.Work.tick candidate iter selected work =
      oracle.select_atoms_loop.body candidate iter selected work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem carryBody_reconstruct
  (iter : core.slice.iter.Iter Std.Usize) (subset : theory.Interpretation)
  (present : Std.Usize) (work : oracle.Work) :
    carryBody (M := Result) oracle.Work.tick iter subset present work =
      oracle.advance_subset_loop.body iter subset present work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem rootLoop_reconstruct
  (iter : core.slice.iter.Iter Std.Usize) (values : Slice Bool)
  (work : oracle.Work) :
    rootLoop (M := Result) Aeneas.Std.loop oracle.failed_root_loop.body iter values work =
      oracle.failed_root_loop iter values work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem rootScan_reconstruct
  (program : theory.Theory) (values : Slice Bool) (work : oracle.Work) :
    rootScan (M := Result) oracle.failed_root_loop program values work =
      oracle.failed_root program values work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem selectionLoop_reconstruct
  (iter : core.ops.range.Range Std.Usize) (candidate : theory.Interpretation)
  (selected : alloc.vec.Vec Std.Usize) (work : oracle.Work) :
    selectionLoop (M := Result) Aeneas.Std.loop oracle.select_atoms_loop.body iter candidate selected work =
      oracle.select_atoms_loop iter candidate selected work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem selectAtoms_reconstruct
  (program : theory.Theory) (candidate : theory.Interpretation)
  (selected : alloc.vec.Vec Std.Usize) (work : oracle.Work) :
    selectAtoms (M := Result) oracle.select_atoms_loop program candidate selected work =
      oracle.select_atoms program candidate selected work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem carryLoop_reconstruct
  (iter : core.slice.iter.Iter Std.Usize) (subset : theory.Interpretation)
  (present : Std.Usize) (work : oracle.Work) :
    carryLoop (M := Result) Aeneas.Std.loop oracle.advance_subset_loop.body iter subset present work =
      oracle.advance_subset_loop iter subset present work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem advanceSubset_reconstruct
  (selected : Slice Std.Usize) (subset : theory.Interpretation)
  (present : Std.Usize) (work : oracle.Work) :
    advanceSubset (M := Result) oracle.advance_subset_loop selected subset present work =
      oracle.advance_subset selected subset present work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem evaluationLoop_reconstruct
  (iter : core.iter.adapters.enumerate.Enumerate (core.slice.iter.Iter
  theory.Node)) (interpretation : theory.Interpretation)
  (frozen : Option (Slice Bool)) (output : alloc.vec.Vec Bool)
  (l : oracle.Limits) (c : zetesis_cpu.cancellation.Cancellation)
  (s : oracle.Statistics) :
    evaluationLoop (M := Result) Aeneas.Std.loop oracle.evaluate_loop.body iter interpretation frozen output l c s =
      oracle.evaluate_loop iter interpretation frozen output l c s := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem evaluate_reconstruct
  (program : theory.Theory) (interpretation : theory.Interpretation)
  (frozen : Option (Slice Bool)) (output : alloc.vec.Vec Bool)
  (work : oracle.Work) :
    evaluate (M := Result) oracle.evaluate_loop program interpretation frozen output work =
      oracle.evaluate program interpretation frozen output work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem subsetQuery_reconstruct
  (program : theory.Theory) (subset : theory.Interpretation)
  (frozen : Slice Bool) (values : alloc.vec.Vec Bool) (work : oracle.Work) :
    subsetQuery (M := Result) zetesis_cpu.cancellation.Cancellation.poll oracle.evaluate oracle.failed_root program subset frozen values work =
      oracle.check_subset program subset frozen values work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem searchBody_reconstruct
  (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Std.Usize)
  (subset : theory.Interpretation) (values : alloc.vec.Vec Bool)
  (work : oracle.Work) (present : Std.Usize) :
    searchBody (M := Result) oracle.check_subset oracle.advance_subset program frozen selected subset values work present =
      oracle.find_countermodel_loop.body program frozen selected subset values work present := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem searchLoop_reconstruct
  (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Std.Usize)
  (subset : theory.Interpretation) (values : alloc.vec.Vec Bool)
  (work : oracle.Work) (present : Std.Usize) :
    searchLoop (M := Result) Aeneas.Std.loop oracle.find_countermodel_loop.body program frozen selected subset values work present =
      oracle.find_countermodel_loop program frozen selected subset values work present := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem findCountermodel_reconstruct
  (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Std.Usize)
  (subset : theory.Interpretation) (values : alloc.vec.Vec Bool)
  (work : oracle.Work) :
    findCountermodel (M := Result) oracle.find_countermodel_loop program frozen selected subset values work =
      oracle.find_countermodel program frozen selected subset values work := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem reserve_reconstruct [VectorReservation]
  (T : Type) (count : Std.Usize) :
    reserve (M := Result) alloc.vec.Vec.try_reserve_exact T count =
      oracle.reserve T count := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem check_reconstruct [VectorReservation]
  (program : theory.Theory) (candidate : theory.Interpretation)
  (limits : oracle.Limits)
  (cancellation : zetesis_cpu.cancellation.Cancellation) :
    check (M := Result) zetesis_cpu.cancellation.Cancellation.poll oracle.reserve oracle.evaluate oracle.failed_root oracle.select_atoms oracle.find_countermodel program candidate limits cancellation =
      oracle.check program candidate limits cancellation := by
  rfl

/-- Filling each named phase hole with its actual operation recovers the exact
original generated definition. No successful subcall or loop outcome is assumed. -/
theorem checkInterpretation_reconstruct [VectorReservation]
  (candidate : theory.Interpretation) (limits : oracle.Limits)
  (cancellation : zetesis_cpu.cancellation.Cancellation) :
    checkInterpretation (M := Result) oracle.check candidate limits cancellation =
      checked.check_interpretation candidate limits cancellation := by
  rfl

end CheckerContexts
