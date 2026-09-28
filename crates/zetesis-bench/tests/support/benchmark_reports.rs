//! Small published-report fixtures; no measurements or solver processes run.

use serde_json::{Value, json};
use std::num::NonZeroUsize;
use zetesis_validation::selected::{Grounder, NativeExecution};

pub(super) const CASES: [&str; 2] = ["generated/choice-2.lp", "generated/cycle-2.lp"];

pub(super) fn profiles() -> [NativeExecution; 2] {
    [1, 4].map(|workers| NativeExecution {
        grounder: Grounder::Auto,
        workers: NonZeroUsize::new(workers).unwrap(),
        completion_workers: NonZeroUsize::MIN,
        batch_size: NonZeroUsize::new(64).unwrap(),
        ..NativeExecution::default()
    })
}

/// The earlier report has one timed-out cell for each native profile. Its later
/// positions remain unattempted; the view must never turn their capture durations
/// into a successful timing population. The later report passes every cell.
pub(super) fn reports() -> [Value; 2] {
    [
        report([1_000_000, 3_000_000, 2_000_000], 5_000_000, true),
        report([10_000_000, 30_000_000, 20_000_000], 8_000_000, false),
    ]
}

fn report(native: [u64; 3], reference: u64, interrupted: bool) -> Value {
    let mut samples = Vec::new();
    for case in 0..CASES.len() {
        let offset = u64::try_from(case).unwrap() * 10_000_000;
        for (round, elapsed) in native.iter().enumerate() {
            samples.push(sample(
                case,
                round,
                &json!({"solver": "reference"}),
                reference + offset,
            ));
            for profile in 0..profiles().len() {
                let mut record = sample(
                    case,
                    round,
                    &json!({"solver": "native", "profile": profile}),
                    elapsed + offset + u64::try_from(profile).unwrap() * 1_000_000,
                );
                if interrupted && case == 1 {
                    record["decision"] = json!(if round == 0 {
                        "timeout"
                    } else {
                        "not_attempted"
                    });
                    record["capture"] = if round == 0 {
                        json!({"elapsed_ns": 9_999_999_999_u64})
                    } else {
                        Value::Null
                    };
                    if round > 0 {
                        // The first native position for this profile within
                        // this case: three earlier rounds, then one reference.
                        record["blocked_by"] = json!(case * 9 + 1 + profile);
                    }
                }
                samples.push(record);
            }
        }
    }
    if interrupted {
        for (producer, peak) in [
            (json!({"solver": "native", "profile": 0}), 3_670_016),
            (json!({"solver": "reference"}), 5_242_880),
        ] {
            let mut memory = sample(0, 0, &producer, 99_000_000);
            memory["slot"]["phase"] = json!("memory");
            memory["memory"] = json!({"peak_rss_bytes": peak});
            samples.push(memory);
        }
    }
    json!({
        "passed": !interrupted,
        "accounted": true,
        "report": {
            "cases": CASES,
            "plan": {"profiles": profiles()},
            "before": [
                {"sha256": if interrupted { "11".repeat(32) } else { "22".repeat(32) }},
                {"sha256": "33".repeat(32)},
                {"sha256": "44".repeat(32)}
            ],
            "started_unix_ns": 1_000_000_000_u64,
            "finished_unix_ns": 2_000_000_000_u64,
            "samples": samples
        }
    })
}

fn sample(case: usize, round: usize, producer: &Value, elapsed_ns: u64) -> Value {
    json!({
        "slot": {"case": case, "phase": "timed", "round": round, "producer": producer},
        "decision": "pass",
        "capture": {
            "elapsed_ns": elapsed_ns,
            "stdout": {"encoding": "utf8", "data": "{}"},
            "stderr": {"encoding": "utf8", "data": ""}
        }
    })
}
