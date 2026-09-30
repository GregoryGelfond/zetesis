//! The N-Queens correctness examples: one board parameter, and complete native
//! model coverage.

use std::collections::BTreeSet;
use std::process::Command;
use zetesis_test_support::repository;

#[test]
fn every_queens_variant_accepts_the_same_board_parameter() {
    use clap::Parser;
    use zetesis_cli::{Completion, Options, run_with_diagnostics};
    use zetesis_cpu::Cancellation;

    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--grounder",
        "eager",
        "--models",
        "0",
    ])
    .unwrap();
    let root = repository::correctness().join("standalone/n-queens");
    for variant in 1..=6 {
        let source =
            std::fs::read_to_string(root.join(format!("variant-{variant:02}.lp"))).unwrap();
        assert_eq!(source.matches("#const n = 8.").count(), 1);
        let source = source.replace("#const n = 8.", "#const n = 4.");
        let mut output = Vec::new();
        let report = run_with_diagnostics(
            source,
            &options,
            &mut output,
            &mut Vec::new(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        let text = String::from_utf8(output).unwrap();
        let mut lines = text.lines();
        let mut boards = BTreeSet::new();
        while let Some(line) = lines.next() {
            if line.starts_with("Answer:") {
                let board: BTreeSet<_> = lines.next().unwrap().split_whitespace().collect();
                boards.insert(board);
            }
        }
        let expected = [
            [
                "queen_at(1,2)",
                "queen_at(2,4)",
                "queen_at(3,1)",
                "queen_at(4,3)",
            ],
            [
                "queen_at(1,3)",
                "queen_at(2,1)",
                "queen_at(3,4)",
                "queen_at(4,2)",
            ],
        ]
        .into_iter()
        .map(|board| board.into_iter().collect::<BTreeSet<_>>())
        .collect::<BTreeSet<_>>();
        assert_eq!(boards, expected, "variant {variant}");
        assert_eq!(report.models, 2);
    }
}

#[test]
fn queens_variant_one_completes_all_92_boards() {
    let path = repository::correctness().join("standalone/n-queens/variant-01.lp");
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--backend", "cpu", "--models", "0"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.contains("Coverage: exhausted"));
    assert!(text.contains("Models: 92;"));
    let mut boards = BTreeSet::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if !line.starts_with("Answer:") {
            continue;
        }
        let board: BTreeSet<(u8, u8)> = lines
            .next()
            .unwrap()
            .split_whitespace()
            .map(|atom| {
                let coordinates = atom
                    .strip_prefix("queen_at(")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                let (row, column) = coordinates.split_once(',').unwrap();
                (row.parse().unwrap(), column.parse().unwrap())
            })
            .collect();
        assert_eq!(board.len(), 8);
        for &(row, column) in &board {
            assert!((1..=8).contains(&row) && (1..=8).contains(&column));
            for &(other_row, other_column) in &board {
                if (row, column) == (other_row, other_column) {
                    continue;
                }
                assert_ne!(row, other_row);
                assert_ne!(column, other_column);
                assert_ne!(row.abs_diff(other_row), column.abs_diff(other_column));
            }
        }
        assert!(boards.insert(board));
    }
    assert_eq!(boards.len(), 92);
}
