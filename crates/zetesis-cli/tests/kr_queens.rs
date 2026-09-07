//! One unchanged full kr-domains source, with complete native model coverage.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

#[test]
fn unchanged_queens_variant_one_completes_all_92_boards() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation/corpus/kr-domains/standalone/n-queens/variant-01.lp");
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
