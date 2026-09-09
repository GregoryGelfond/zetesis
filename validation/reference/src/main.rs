//! Command view of the deterministic finite semantic validation campaign.
use std::{env, fs, process::ExitCode, time::Instant};
use zetesis_reference::{lifted::run_sparse_fixtures, run_validation};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let output = match args.next() {
        None => None,
        Some(flag) if flag == "--report" => match args.next() {
            Some(path) if args.next().is_none() => Some(path),
            _ => {
                eprintln!("usage: zetesis-reference [--report FILE]");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("usage: zetesis-reference [--report FILE]");
            return ExitCode::FAILURE;
        }
    };
    let start = Instant::now();
    match run_validation() {
        Ok(counts) => {
            let ground_seconds = start.elapsed().as_secs_f64();
            let lifted = match run_sparse_fixtures() {
                Ok(reports) => reports,
                Err(err) => {
                    eprintln!("LIFTED VALIDATION FAILED: {err}");
                    return ExitCode::FAILURE;
                }
            };
            let ground_report = counts.json(ground_seconds);
            let lifted_rows = lifted.iter().map(|r| format!(
                "    {{\"fixture\": \"{}\", \"domain_size\": {}, \"symbolic_seed_atoms\": {}, \"true_seed_atoms\": {}, \"target_rule_source_substitutions\": {}, \"derived_atoms\": {}, \"rounds\": {}, \"tuple_probes\": {}, \"complete_bindings\": {}, \"carrier_tuples_enumerated\": {}, \"ground_rows_materialized\": {}}}",
                r.fixture, r.domain_size, r.symbolic_seed_atoms, r.true_seed_atoms,
                r.target_rule_source_substitutions, r.check.closure.len(), r.check.stats.rounds,
                r.check.stats.tuple_probes, r.check.stats.complete_bindings,
                r.check.stats.carrier_tuples_enumerated, r.check.stats.ground_rows_materialized
            )).collect::<Vec<_>>().join(",\n");
            let report = format!(
                "{},\n  \"lifted_sparse_fixtures\": [\n{}\n  ]\n}}\n",
                ground_report.trim_end().trim_end_matches('}').trim_end(),
                lifted_rows
            );
            if let Some(path) = output
                && let Err(err) = fs::write(&path, &report)
            {
                eprintln!("could not write report {path}: {err}");
                return ExitCode::FAILURE;
            }
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("VALIDATION FAILED: {err}");
            ExitCode::FAILURE
        }
    }
}
