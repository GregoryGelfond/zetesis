//! Public experiment boundary: deterministic CPU qualification and typed failure.

use clap::Parser;
use zetesis_experiments::{BenchmarkError, Options, run};

#[test]
fn all_families_qualify_cpu_results() {
    let options = Options::try_parse_from([
        "zetesis-bench",
        "--backend",
        "cpu",
        "--atoms",
        "1,33",
        "--batches",
        "1,16",
        "--repetitions",
        "2",
        "--workers",
        "2",
    ])
    .unwrap();
    let mut output = Vec::new();
    run(&options, &mut output).unwrap();
    let report = String::from_utf8(output).unwrap();
    assert!(report.contains("# status=PASS"));
    assert!(report.contains("GPU_execution=not_requested"));
    assert_eq!(
        report
            .lines()
            .filter(|line| line.contains("\twarm\t"))
            .count(),
        48
    );
    assert!(!report.contains("\tmetal\t"));
}

#[test]
fn refuses_invalid_dimensions_and_incomplete_cpu_work() {
    let mut options =
        Options::try_parse_from(["zetesis-bench", "--backend", "cpu", "--atoms", "4089"]).unwrap();
    let mut output = Vec::new();
    assert!(matches!(
        run(&options, &mut output),
        Err(BenchmarkError::Dimensions)
    ));
    assert!(output.is_empty());
    options.atoms = vec![std::num::NonZeroUsize::new(1).unwrap()];
    options.max_work = 0;
    assert!(matches!(
        run(&options, &mut output),
        Err(BenchmarkError::Stop(_))
    ));
    assert!(!String::from_utf8(output).unwrap().contains("status=PASS"));
}
