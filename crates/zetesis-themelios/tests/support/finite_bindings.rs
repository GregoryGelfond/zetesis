//! Independent complete-interpretation and frozen-formula test evaluators.

use serde_json::Value as Json;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use zetesis_core::catalog::AtomRef;
use zetesis_core::{Sign, ValueNodeRef};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::AdmittedFormula;

pub(super) type Models = BTreeSet<BTreeSet<String>>;

pub(super) fn atom_text<'a>(atom: impl Into<AtomRef<'a>>) -> String {
    let atom = atom.into();
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let name = format!("{sign}{}", atom.predicate().name());
    if atom.values().is_empty() {
        return name;
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value.descriptor() {
            ValueNodeRef::Number(number) => number.to_string(),
            ValueNodeRef::Symbol(value) => value.to_owned(),
            ValueNodeRef::String(value) => serde_json::to_string(value).unwrap(),
            ValueNodeRef::Infimum => "#inf".into(),
            ValueNodeRef::Supremum => "#sup".into(),
            ValueNodeRef::Function { .. } | ValueNodeRef::Tuple { .. } => value.to_string(),
        })
        .collect();
    format!("{name}({})", values.join(","))
}

pub(super) fn values(theory: &Theory, mask: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut result = Vec::new();
    for (index, node) in theory.nodes().iter().enumerate() {
        let value = match *node {
            Node::False => false,
            Node::Atom(atom) => mask & (1 << atom) != 0,
            Node::And(left, right) => result[left] && result[right],
            Node::Or(left, right) => result[left] || result[right],
            Node::Implies(left, right) => !result[left] || result[right],
        };
        result.push(value && frozen.is_none_or(|outer| outer[index]));
    }
    result
}

pub(super) fn holds(theory: &Theory, values: &[bool]) -> bool {
    theory.roots().iter().all(|&root| values[root])
}

pub(super) fn exhaustive(input: &AdmittedFormula) -> Models {
    assert!(input.atoms().len() <= 12, "bounded independent carrier");
    let mut result = BTreeSet::new();
    for mask in 0..1_usize << input.atoms().len() {
        let outer = values(input.theory(), mask, None);
        if !holds(input.theory(), &outer) {
            continue;
        }
        let mut subset = mask;
        let mut countermodel = false;
        while subset != 0 {
            subset = (subset - 1) & mask;
            if holds(
                input.theory(),
                &values(input.theory(), subset, Some(&outer)),
            ) {
                countermodel = true;
                break;
            }
        }
        if !countermodel {
            assert!(
                result.insert(
                    input
                        .atoms()
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| mask & (1 << index) != 0)
                        .map(|(_, atom)| atom_text(atom))
                        .collect()
                )
            );
        }
    }
    result
}

pub(super) fn native(input: &AdmittedFormula) -> Models {
    let mut search = zetesis_sat::StableModels::new(
        input.theory(),
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut result = BTreeSet::new();
    for model in search.by_ref() {
        assert!(
            result.insert(
                model
                    .unwrap()
                    .atoms()
                    .map(|index| atom_text(input.atoms().at(index).unwrap()))
                    .collect()
            )
        );
    }
    assert!(search.exhausted(), "complete original-theory enumeration");
    result
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-finite-bindings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn external(source: &str, valid: bool) -> Json {
    let directory = Directory::new();
    let source_path = directory.0.join("source.lp");
    let out = directory.0.join("stdout");
    let err = directory.0.join("stderr");
    fs::write(&source_path, source).unwrap();
    let mut child = Command::new(std::env::var_os("CLINGO").unwrap_or_else(|| "clingo".into()))
        .args([
            "--models=0",
            "--outf=2",
            "--parallel-mode=1",
            "--opt-mode=enum",
            "-",
        ])
        .stdin(File::open(source_path).unwrap())
        .stdout(File::create(&out).unwrap())
        .stderr(File::create(&err).unwrap())
        .spawn()
        .expect("independently installed clingo");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let oversized = [&out, &err]
            .iter()
            .any(|path| fs::metadata(path).unwrap().len() > 65_536);
        if oversized || Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("bounded reference incomplete; no compatibility result");
        }
        if let Some(status) = child.try_wait().unwrap() {
            let diagnostics = fs::read_to_string(&err).unwrap();
            let stdout = fs::read_to_string(&out).unwrap();
            println!(
                "{}",
                serde_json::json!({"source": source, "valid": valid, "exit": status.code(), "stdout": stdout, "stderr": diagnostics})
            );
            assert!(
                if valid {
                    matches!(status.code(), Some(10 | 20 | 30))
                } else {
                    status.code() == Some(65) && diagnostics.contains("unsafe variables")
                },
                "{diagnostics}"
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        [&out, &err]
            .iter()
            .all(|path| fs::metadata(path).unwrap().len() <= 65_536)
    );
    let stdout = fs::read_to_string(out).unwrap();
    serde_json::from_str(&stdout).unwrap()
}
