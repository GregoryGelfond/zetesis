//! Bounded unchanged-source reference capture shared by CLI semantic contracts.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use zetesis_solve::AnswerSelection;
use zetesis_validation::{answers, process};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-cli-reference-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn clingo() -> PathBuf {
    std::env::var_os("CLINGO")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|directory| directory.join("clingo"))
                .find(|path| path.is_file())
        })
        .expect("independently installed clingo")
        .canonicalize()
        .unwrap()
}

/// Capture a complete unchanged-source clingo report for the requested family.
/// All-family mode ignores costs; optimum mode reconciles every optimal tie.
pub fn complete(original: &str, selection: AnswerSelection) -> answers::ReportedAnswers {
    let directory = Directory::new();
    let source = directory.0.join("original.lp");
    fs::write(&source, original).unwrap();
    assert_eq!(fs::read(&source).unwrap(), original.as_bytes());
    let mode = match selection {
        AnswerSelection::All => "--opt-mode=ignore",
        AnswerSelection::Optimal => "--opt-mode=optN",
    };
    let arguments = [
        "0".into(),
        "--outf=2".into(),
        mode.into(),
        source.into_os_string(),
    ];
    let (capture, pending) = process::invoke(
        process::Invocation {
            executable: &clingo(),
            arguments: &arguments,
            directory: &directory.0,
        },
        process::Limits {
            timeout: Duration::from_secs(5),
            max_output_bytes: 64 * 1024,
            cleanup_timeout: Duration::from_secs(1),
        },
    )
    .unwrap()
    .into_parts();
    if let Some(pending) = pending {
        let cleanup = pending.retry(Duration::from_secs(1));
        if let Some(pending) = cleanup.pending {
            panic!(
                "unresolved oracle child {}: {:?}",
                pending.abandon(),
                cleanup.failure
            );
        }
        assert!(cleanup.exit.is_some());
        assert!(cleanup.failure.is_none());
    }
    println!(
        "{}",
        serde_json::json!({
            "source": original, "mode": mode, "exit": capture.exit(),
            "stdout": capture.stdout_text().unwrap(), "stderr": capture.stderr_text().unwrap()
        })
    );
    assert_eq!(capture.stop(), process::Stop::Completed);
    assert!(capture.failure().is_none());
    assert!(capture.cleanup_failure().is_none());
    let exit = capture.exit().unwrap();
    assert!(exit.signal.is_none());
    assert!(matches!(exit.code, Some(10 | 20 | 30)));
    let report = answers::clingo_json(capture.stdout(), answers::Limits::default()).unwrap();
    assert_eq!(report.solver(), "clingo version 5.8.2");
    report
}
