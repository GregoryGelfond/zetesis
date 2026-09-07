# zetesis-telemetry

A standard-library-only recorder for optional, exclusive host stages. Applications
choose the recorder's interval and enter source preparation, explicit grounding,
solving, and output guards. Nested guards suspend their parent, including on
returned errors or unwind. Instrumentation overhead is not separated: some
bookkeeping falls inside measured spans; unattributed time covers only intervals
outside all stages. Snapshots are typed Rust values; there is no text,
JSON, solver, frontend, device, scheduler or command-line dependency.

`StageRecorder::new(false)` performs no clock reads. `StageTimings::is_complete`
checks the current arithmetic partition, including after public duration edits;
it establishes neither recorder provenance nor semantic completion. Lazy
source joins are marked interleaved; a mixed route retains measured eager spans
without claiming a separate lazy grounding duration. Guards must drop in stack
order; recorder-detected misuse makes unattributed time unavailable rather than changing application
control. The recorder is single-threaded; wrap coordinator wall intervals to
include worker waits without summing overlapping worker times.

```rust
use zetesis_telemetry::{SolveStage, StageRecorder};
let recorder = StageRecorder::new(true);
{
    let _solving = recorder.enter(SolveStage::Solving);
    // Solver setup and work; nested output/grounding can be timed separately.
}
let timings = recorder.snapshot().unwrap();
assert!(timings.is_complete());
assert_eq!(timings.get(SolveStage::Solving).unwrap().calls, 1);
```

MIT licensed; inherits the workspace's authored-code lint baseline.
