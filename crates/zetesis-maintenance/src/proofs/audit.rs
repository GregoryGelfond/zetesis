//! Exact audit requests, output and command-log correspondence.
use super::{
    hashes,
    source::{NAME, mask},
};
use crate::{Error, files::Tree, json, require};
use regex::Regex;
use serde_json::Value;
use std::{collections::BTreeSet, sync::LazyLock};

pub(super) const ALLOWED: [&str; 3] = ["Classical.choice", "Quot.sound", "propext"];
pub(super) const COMMAND: [&str; 6] = [
    "lake",
    "env",
    "lean",
    "-DautoImplicit=false",
    "-DwarningAsError=true",
    "Audit.lean",
];
static REQUEST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^#print axioms ({NAME})$")).unwrap());
static ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"'({NAME})'\s+(?:depends on axioms:\s*\[([^\]]*)\]|does not depend on any axioms)"
    ))
    .unwrap()
});

pub(super) fn check(
    tree: &mut Tree,
    indexed: &BTreeSet<String>,
) -> Result<BTreeSet<String>, Error> {
    let requests = requests(&tree.text("Audit.lean")?, indexed)?;
    let output = tree.text("axiom-audit.txt")?;
    let mut names = Vec::new();
    let mut used = BTreeSet::new();
    let mut position = 0;
    for entry in ENTRY.captures_iter(&output) {
        let whole = entry.get(0).expect("whole match");
        require(
            output[position..whole.start()].trim().is_empty(),
            "unexpected axiom audit output",
        )?;
        let values: Vec<&str> = entry
            .get(2)
            .filter(|value| !value.as_str().trim().is_empty())
            .map_or_else(Vec::new, |value| {
                value.as_str().split(',').map(str::trim).collect()
            });
        let unique: BTreeSet<&str> = values.iter().copied().collect();
        require(
            unique.len() == values.len() && unique.iter().all(|value| ALLOWED.contains(value)),
            format!("disallowed or duplicate axiom for {}", &entry[1]),
        )?;
        used.extend(unique.into_iter().map(str::to_owned));
        names.push(entry[1].to_owned());
        position = whole.end();
    }
    require(
        output[position..].trim().is_empty(),
        "unexpected trailing axiom audit output",
    )?;
    require(
        names == requests,
        "axiom output names/order differ from Audit.lean",
    )?;
    Ok(used)
}
fn requests(source: &str, indexed: &BTreeSet<String>) -> Result<Vec<String>, Error> {
    let mut imported = false;
    let mut requested = Vec::new();
    for line in mask(source)?.lines() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        if text == "import Zetesis" {
            require(!imported, "duplicate Audit import")?;
            imported = true;
        } else {
            let captures = REQUEST
                .captures(text)
                .ok_or_else(|| Error::Invalid("unsupported Audit.lean command".into()))?;
            requested.push(captures[1].to_owned());
        }
    }
    let names: BTreeSet<String> = requested.iter().cloned().collect();
    require(
        imported && requested.len() == names.len() && &names == indexed,
        "Audit.lean names differ from unique index",
    )?;
    Ok(requested)
}
pub(super) fn commands(tree: &mut Tree, record: &Value) -> Result<(), Error> {
    let commands = json::array(&record["commands"], "current commands")?;
    require(!commands.is_empty(), "missing current commands")?;
    let logs = &record["verification_log_sha256"];
    hashes(tree, logs, None, "verification logs")?;
    let logs = json::object(logs, "verification logs")?;
    let mut seen = BTreeSet::new();
    let mut audits = Vec::new();
    let mut build = false;
    for entry in commands {
        json::object(entry, "current command")?;
        let command: Vec<&str> = json::array(&entry["command"], "current command arguments")?
            .iter()
            .map(|value| json::string(value, "command argument"))
            .collect::<Result<_, _>>()?;
        require(
            !command.is_empty() && command.iter().all(|value| !value.is_empty()),
            "invalid current command arguments",
        )?;
        require(
            entry["result"] == "PASS" && entry["exit_code"].as_i64() == Some(0),
            "current command did not pass",
        )?;
        require(
            entry["elapsed_seconds"]
                .as_f64()
                .is_some_and(|value| value.is_finite() && value >= 0.0),
            "invalid command elapsed time",
        )?;
        require(
            entry["cwd"].as_str().is_some_and(|value| !value.is_empty()),
            "missing recorded command cwd",
        )?;
        let log = json::string(&entry["log"], "command log")?;
        require(logs.contains_key(log), "current command log is not hashed")?;
        require(seen.insert(log), "duplicate current command log")?;
        if command == COMMAND {
            audits.push(log);
        }
        build |= command == ["lake", "build"];
    }
    require(build, "missing current lake build command")?;
    require(
        audits.len() == 1,
        "expected exactly one current strict Audit.lean command",
    )?;
    require(
        tree.read(audits[0])? == tree.read("axiom-audit.txt")?,
        "current Audit command log differs from axiom-audit.txt",
    )
}
