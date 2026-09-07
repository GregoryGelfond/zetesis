//! Formula experiments preserve exact meaning and explicit qualification scope.

use std::error::Error;
use std::io::{self, Write};

use clap::Parser;
use zetesis_cpu::Control;
use zetesis_experiments::{
    CommandOptions, Experiment, FormulaBenchmarkError, FormulaFamily, FormulaFixture,
    FormulaOptions, run_formula,
};
use zetesis_ferraris::{Interpretation, Limits, check};

fn options() -> FormulaOptions {
    let parsed = CommandOptions::try_parse_from([
        "zetesis-bench",
        "formula",
        "--backend",
        "cpu",
        "--atoms",
        "3",
        "--batches",
        "8",
        "--repetitions",
        "1",
    ])
    .unwrap();
    let Some(Experiment::Formula(options)) = parsed.command else {
        panic!("formula subcommand")
    };
    options
}

#[test]
fn formula_and_original_static_commands_remain_distinct() {
    let static_case = CommandOptions::try_parse_from([
        "zetesis-bench",
        "--backend",
        "cpu",
        "--atoms",
        "8",
        "--families",
        "wide",
    ])
    .unwrap();
    assert!(static_case.command.is_none());
    assert_eq!(static_case.static_options.atoms[0].get(), 8);
    assert_eq!(options().atoms[0].get(), 3);
    assert_eq!(options().cpu_workers.get(), 4);
    for arguments in [
        vec!["formula", "--atoms", "0"],
        vec!["formula", "--cpu-workers", "0"],
        vec!["formula", "--families", "wide"],
        vec!["--backend", "cpu", "formula"],
        vec!["--max-work", "1", "formula"],
    ] {
        assert!(
            CommandOptions::try_parse_from(["zetesis-bench"].into_iter().chain(arguments)).is_err()
        );
    }
}

fn truth(family: FormulaFamily, atoms: usize, candidate: usize, tested: usize) -> bool {
    let m = |atom: usize| candidate & (1 << atom) != 0;
    let j = |atom: usize| m(atom) && tested & (1 << atom) != 0;
    (0..atoms).all(|a| {
        let b = (a + 1) % atoms;
        match family {
            FormulaFamily::Choices => j(a) || !m(a),
            FormulaFamily::Cycle => (!m(a) || m(b)) && (!j(a) || j(b)),
            FormulaFamily::Conjunction => j(a) && j(b),
            FormulaFamily::Disjunction => {
                if a % 2 == 1 {
                    true
                } else {
                    j(a) || j(if a + 1 < atoms { b } else { a })
                }
            }
            FormulaFamily::MaskedImplication => {
                let c = (a + 2) % atoms;
                let implication = (!m(a) || m(b)) && (!j(a) || j(b));
                (!m(a) || m(b) || m(c)) && (implication || j(c))
            }
        }
    })
}

#[test]
fn all_tiny_fixture_models_match_independent_frozen_formula_definitions() {
    for family in [
        FormulaFamily::Choices,
        FormulaFamily::Cycle,
        FormulaFamily::Conjunction,
        FormulaFamily::Disjunction,
        FormulaFamily::MaskedImplication,
    ] {
        for atoms in 1..=5 {
            let fixture = FormulaFixture::new(family, atoms).unwrap();
            for candidate in 0..1 << atoms {
                let interpretation = Interpretation::new(
                    fixture.theory(),
                    (0..atoms).filter(|a| candidate & (1 << a) != 0),
                )
                .unwrap();
                let stable = truth(family, atoms, candidate, candidate)
                    && !(0..1 << atoms).any(|tested| {
                        tested != candidate
                            && tested & !candidate == 0
                            && truth(family, atoms, candidate, tested)
                    });
                let actual = check(
                    fixture.theory(),
                    &interpretation,
                    Limits::default(),
                    &Control::default(),
                )
                .unwrap();
                assert_eq!(actual.accepted(), stable, "{family:?}/{atoms}/{candidate}");
            }
        }
    }
}

#[test]
fn candidates_keep_theory_identity_and_replace_the_entire_epoch() {
    let fixture = FormulaFixture::new(FormulaFamily::Choices, 65).unwrap();
    let first = fixture.candidates(257, 0).unwrap();
    assert_eq!(first[0].atoms().count(), 0);
    assert_eq!(first[1].atoms().count(), 65);
    assert_eq!(first[2].atoms().collect::<Vec<_>>(), vec![0]);
    assert_eq!(first[256].atoms().count(), 0);
    let next = fixture.candidates(257, 1).unwrap();
    assert!(
        first
            .iter()
            .chain(&next)
            .all(|candidate| candidate.theory().same_instance(fixture.theory()))
    );
    assert_ne!(next[0].atoms().count(), 0);
    assert_eq!(
        first[0].atoms().count(),
        0,
        "a later epoch does not mutate earlier candidates"
    );
    for count in [0, 4097, usize::MAX] {
        assert!(matches!(
            fixture.candidates(count, 0),
            Err(FormulaBenchmarkError::Dimensions)
        ));
        assert!(matches!(
            FormulaFixture::new(FormulaFamily::Choices, count),
            Err(FormulaBenchmarkError::Dimensions)
        ));
    }
}

#[test]
fn cpu_formula_measurements_state_their_scope_and_complete_all_families() {
    let mut output = Vec::new();
    let mut options = options();
    options.max_work = 12_345_678;
    options.max_batch_bytes = 123_456_789;
    options.gpu_max_rounds = 17;
    options.gpu_max_work = 23_456_789;
    run_formula(&options, &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("GPU_execution=not_requested"));
    assert!(text.contains("source_grounding=excluded outer_search=excluded"));
    assert!(text.contains("status=PASS scope=synthetic-membership"));
    assert_eq!(
        text.lines()
            .filter(|line| line.contains("\tcpu-native\t"))
            .count(),
        10
    );
    assert_eq!(
        text.lines()
            .filter(|line| line.contains("\tcpu-rayon\t"))
            .count(),
        10
    );
    assert!(text.contains("cpu_workers=4 scalar_workers=1 residual_cpu_workers=1"));
    assert!(text.contains("cpu_pool=rayon requested_workers=4 actual_workers=4 pool_init_ns="));
    assert!(text.contains("cpu_work_limit_scope=per_candidate"));
    assert!(text.contains("max_work_per_candidate=12345678 max_batch_bytes=123456789 gpu_max_rounds=17 gpu_max_work_per_candidate=23456789"));
    assert!(!text.contains("\tmetal-with-cpu-residuals\t"));
}

struct CutWriter {
    limit: usize,
    bytes: Vec<u8>,
}
impl Write for CutWriter {
    fn write(&mut self, value: &[u8]) -> io::Result<usize> {
        let count = value.len().min(self.limit - self.bytes.len());
        if count == 0 && !value.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "formula output closed",
            ));
        }
        self.bytes.extend_from_slice(&value[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn incomplete_reference_or_output_never_publishes_qualification_success() {
    let mut options = options();
    options.max_work = 0;
    let mut output = Vec::new();
    let error = run_formula(&options, &mut output).unwrap_err();
    assert!(matches!(error, FormulaBenchmarkError::Incomplete(_)));
    assert!(error.source().is_some());
    assert!(!String::from_utf8(output).unwrap().contains("status=PASS"));
    options.max_work = 100_000_000;
    options.families.truncate(1);
    options.batches = vec![std::num::NonZeroUsize::MIN];
    for limit in [0, 15, 90, 200, 300] {
        let mut output = CutWriter {
            limit,
            bytes: Vec::new(),
        };
        let error = run_formula(&options, &mut output).unwrap_err();
        assert!(matches!(error, FormulaBenchmarkError::Output(_)));
        assert_eq!(error.source().unwrap().to_string(), "formula output closed");
        assert!(
            !String::from_utf8(output.bytes)
                .unwrap()
                .contains("status=PASS")
        );
    }
}

#[test]
fn bad_dimensions_are_refused_before_output_or_hardware() {
    let baseline = options();
    for kind in 0..7 {
        let mut options = baseline.clone();
        match kind {
            0 => options.families.clear(),
            1 => options.batches.clear(),
            2 => options.atoms.clear(),
            3 => options.atoms = vec![std::num::NonZeroUsize::new(4097).unwrap()],
            4 => options.batches = vec![std::num::NonZeroUsize::new(4097).unwrap()],
            5 => options.repetitions = std::num::NonZeroUsize::new(101).unwrap(),
            _ => options.cpu_workers = std::num::NonZeroUsize::new(65).unwrap(),
        }
        let mut output = Vec::new();
        let error = run_formula(&options, &mut output).unwrap_err();
        assert!(matches!(error, FormulaBenchmarkError::Dimensions));
        assert!(error.source().is_none());
        assert!(error.to_string().contains("1..4096"));
        assert!(output.is_empty());
    }
}

#[test]
fn one_worker_rows_preserve_scalar_schema_and_explicit_fixed_sample_order() {
    let mut options = options();
    options.cpu_workers = std::num::NonZeroUsize::MIN;
    options.families.truncate(1);
    let mut output = Vec::new();
    run_formula(&options, &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("cpu_pool=rayon requested_workers=1 actual_workers=1"));
    let rows: Vec<Vec<_>> = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("family"))
        .map(|line| line.split('\t').collect())
        .collect();
    assert_eq!(rows.len(), 4);
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), 11);
        assert_eq!(
            row[4],
            if index % 2 == 0 {
                "cpu-native"
            } else {
                "cpu-rayon"
            }
        );
        assert_eq!(row[5], if index < 2 { "initial-case" } else { "warm" });
        assert_eq!(row[6].parse::<usize>().unwrap(), index / 2);
        assert_eq!(&row[8..], &["0", "0", "0"]);
    }
}
