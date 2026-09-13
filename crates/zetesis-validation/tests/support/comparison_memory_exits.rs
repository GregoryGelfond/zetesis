//! Real RSS helpers retain synthetic solver exits; this is not solver qualification.
use super::{Fixture, Path, Value, fs};
use zetesis_validation::{
    performance::{self, Decision, Fault, Phase, Producer, Sample, Schedule},
    process,
};

const NO_PATH: &str = "scenarios/shortest-path/variant-01/04-no-path.lp";
const COUNT_INVOCATION: &str = r#"rss_calls=0
if [ -f "$0.calls" ]; then IFS= read -r rss_calls < "$0.calls"; fi
rss_calls=$((rss_calls + 1))
printf '%s\n' "$rss_calls" > "$0.calls"
"#;

fn fixture(native_memory_exit: i32) -> Fixture {
    // One selected case runs native qualification, timing and diagnostics
    // before its fourth invocation, the separately supervised memory sample.
    // Metadata returns before this prefix. Only a private counter file changes;
    // the admitted executable bytes remain unchanged throughout the campaign.
    let prefix = format!(
        r#"{COUNT_INVOCATION}rss_exit=0
if [ "$rss_calls" -eq 4 ]; then rss_exit={native_memory_exit}; fi
"#
    );
    let fixture = Fixture::new(&prefix, |_| {});
    let native = fs::read_to_string(&fixture.native).unwrap();
    assert!(native.contains("; exit 0;; esac"));
    fs::write(
        &fixture.native,
        native.replace("; exit 0;; esac", "; exit \"$rss_exit\";; esac"),
    )
    .unwrap();
    // The reference emits the fixture's complete UNSAT JSON with clingo's
    // accepted code 20, both directly and as a real waited-for helper child.
    let reference = fs::read_to_string(&fixture.reference).unwrap();
    assert!(reference.contains("; exit 0;; esac"));
    fs::write(
        &fixture.reference,
        reference
            .replace(
                "for input do :; done",
                &format!("{COUNT_INVOCATION}for input do :; done"),
            )
            .replace("; exit 0;; esac", "; exit 20;; esac"),
    )
    .unwrap();
    fixture
}

fn assert_memory_capture(sample: &Sample, direct: &Sample, helper: &Path, child_exit: i32) {
    let capture = sample.capture();
    assert_eq!(sample.slot().phase, Phase::Memory);
    assert_eq!(capture.executable(), helper);
    assert_eq!(capture.arguments()[0], "__measure-child");
    assert_eq!(
        capture.arguments()[2].as_os_str(),
        direct.capture().executable().as_os_str()
    );
    assert_eq!(&capture.arguments()[3..], direct.capture().arguments());
    assert_eq!(capture.directory(), direct.capture().directory());
    assert_eq!(
        capture.stop(),
        Some(process::Stop::Completed),
        "{capture:#?}"
    );
    assert_eq!(
        capture.exit().map(|exit| exit.code),
        Some(Some(0)),
        "{capture:#?}"
    );
    assert_eq!(capture.exit().unwrap().signal, None);
    assert!(capture.failure().is_none());
    assert!(capture.cleanup_failure().is_none());
    assert_eq!(capture.stdout(), direct.capture().stdout());
    assert!(capture.stderr().is_empty());
    let memory = sample.memory().unwrap();
    assert!(memory.valid());
    assert_eq!(memory.exit_code, Some(child_exit));
    assert_eq!(memory.signal, None);
    assert!(capture.helper_child_id().unwrap() > 1);
    assert_ne!(Some(memory.child), capture.helper_child_id());
    let raw: Value = serde_json::from_slice(sample.memory_record().unwrap()).unwrap();
    assert_eq!(raw, serde_json::to_value(memory).unwrap());
}

#[test]
fn memory_qualification_uses_the_solver_exit_policy() {
    // 0 is the positive control; 7 is a failure for both producers, while 20
    // is accepted only for the reference. No measurement record is fabricated.
    for native_exit in [0, 7, 20] {
        let fixture = fixture(native_exit);
        let mut request = fixture.request();
        request.schedule = Schedule::for_cases(vec![NO_PATH.into()], 0, 1)
            .unwrap()
            .with_memory(1)
            .unwrap();
        let helper = Path::new(env!("CARGO_BIN_EXE_zetesis-perf"));
        let report = performance::run_with_runner(&request, helper).unwrap();
        let samples = report.samples();
        let expected = [
            (Phase::Qualification, Producer::Reference),
            (Phase::Qualification, Producer::Native),
            (Phase::Timed, Producer::Native),
            (Phase::Timed, Producer::Reference),
            (Phase::Diagnostics, Producer::Native),
            (Phase::Memory, Producer::Native),
            (Phase::Memory, Producer::Reference),
        ];
        let count = if native_exit == 0 { 7 } else { 6 };
        assert_eq!(samples.len(), count, "{samples:#?}; {:#?}", report.faults());
        for (sample, &(phase, producer)) in samples.iter().zip(&expected) {
            let slot = sample.slot();
            assert_eq!((slot.phase, slot.producer), (phase, producer));
            assert_eq!(slot.case.path(), NO_PATH);
            assert_eq!(slot.round, 0);
        }
        for sample in &samples[..5] {
            assert_eq!(sample.decision(), Decision::Pass, "{sample:#?}");
            assert_eq!(sample.selected_models(), Some(0));
            let earlier = usize::from(sample.slot().producer == Producer::Native);
            assert_eq!(
                sample.capture().stdout(),
                samples[earlier].capture().stdout()
            );
        }
        assert_eq!(
            samples[1].capture().stdout(),
            b"UNSATISFIABLE\nModels: 0\nCoverage: exhausted\n"
        );
        assert_memory_capture(&samples[5], &samples[1], helper, native_exit);
        assert!(report.unresolved_children().is_empty());
        assert!(
            report
                .after()
                .iter()
                .all(zetesis_validation::selected::Change::unchanged)
        );
        assert_eq!(
            fs::read(fixture.native.with_extension("calls")).unwrap(),
            b"4\n"
        );
        assert_eq!(
            fs::read(fixture.reference.with_extension("calls")).unwrap(),
            if native_exit == 0 { b"3\n" } else { b"2\n" }
        );
        if native_exit == 0 {
            assert!(report.passed(), "{:#?}", report.faults());
            assert_memory_capture(&samples[6], &samples[0], helper, 20);
            assert_eq!(samples[6].decision(), Decision::Pass);
        } else {
            let failed = &samples[5];
            assert!(!report.passed());
            assert!(report.summaries().is_none());
            assert_eq!(failed.decision(), Decision::InvocationFailure);
            assert_eq!(
                failed.detail(),
                Some("child RSS record lacks an accepted solver exit")
            );
            assert_eq!(failed.selected_models(), None);
            assert!(matches!(report.faults(), [Fault::Observation]));
        }
    }
}
