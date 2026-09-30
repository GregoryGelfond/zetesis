//! Bounded unchanged-source reference capture shared by CLI semantic contracts.

use zetesis_clingo_support as oracle;
use zetesis_solve::AnswerSelection;
use zetesis_validation::answers;

/// Capture a complete unchanged-source clingo report for the requested family.
/// All-family mode ignores costs; optimum mode reconciles every optimal tie.
pub fn complete(original: &str, selection: AnswerSelection) -> answers::ReportedAnswers {
    let mode = match selection {
        AnswerSelection::All => "--opt-mode=ignore",
        AnswerSelection::Optimal => "--opt-mode=optN",
    };
    let run = oracle::run(
        original,
        &["0", "--outf=2", mode],
        oracle::Limits::default(),
    );
    println!(
        "{}",
        serde_json::json!({
            "source": original, "mode": mode, "exit": run.code(),
            "stdout": String::from_utf8_lossy(run.stdout()),
            "stderr": String::from_utf8_lossy(run.stderr())
        })
    );
    oracle::answers(&run)
}
