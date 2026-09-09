//! A synthetic consistent record; no assertion of kernel acceptance.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub record: Value,
    pub entries: Value,
}
impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let entries = json!([
            {"name":"Zetesis.first","file":"Zetesis/A.lean","line":8},
            {"name":"Zetesis.Sub.second'","file":"Zetesis/Sub/B.lean","line":2}
        ]);
        let mut fixture = Self {
            directory,
            entries,
            record: json!({
                "schema_version":1,"status":"PASS","semantic_modules":2,"theorems_audited":2,
                "transitive_axioms":["Classical.choice","Quot.sound","propext"],
                "project_axioms":[],"proof_holes":false,"native_evaluation_proof_shortcuts":false,
                "commands":[
                    {"command":["lake","build"],"cwd":"fixture","result":"PASS","exit_code":0,"elapsed_seconds":0.1,"log":"verification/current/build.log"},
                    {"command":["lake","env","lean","-DautoImplicit=false","-DwarningAsError=true","Audit.lean"],"cwd":"fixture","result":"PASS","exit_code":0,"elapsed_seconds":0.1,"log":"verification/current/audit.log"}
                ],
                "audit_consistency":{"source_theorem_declarations":2,"indexed_theorems":2,"printed_axiom_entries":2,"unique_qualified_names":2,"source_index_line_check":"PASS","allowed_transitive_axioms_only":"PASS"},
                "source_sha256":{"Audit.lean":"","Zetesis.lean":"","lakefile.lean":"","lake-manifest.json":"","lean-toolchain":"","Zetesis/A.lean":"","Zetesis/Sub/B.lean":""},
                "generated_artifact_sha256":{"README.md":"","theorems.json":"","axiom-audit.txt":""},
                "verification_log_sha256":{"verification/current/build.log":"","verification/current/audit.log":""}
            }),
        };
        for (path, content) in [
            (
                "Zetesis/A.lean",
                "import Std\nnamespace Zetesis\n/- theorem ignored : False := by sorry\n/- nested comment -/ -/\ndef label := \"theorem also_ignored namespace Hidden end\"\nsection Local\n-- theorem another_ignored\ntheorem first : True := by trivial\nend Local\nend Zetesis\n",
            ),
            (
                "Zetesis/Sub/B.lean",
                "namespace Zetesis.Sub\ntheorem second'\n    : True := by trivial\nend Zetesis.Sub\n",
            ),
            ("Zetesis.lean", "import Zetesis.A\nimport Zetesis.Sub.B\n"),
            ("lakefile.lean", "import Lake\n"),
            ("lake-manifest.json", "{\"packages\": []}\n"),
            ("lean-toolchain", "leanprover/lean4:v4.33.1\n"),
            ("README.md", "Synthetic record fixture, not checked Lean.\n"),
            (
                "Audit.lean",
                "import Zetesis\n\n#print axioms Zetesis.first\n#print axioms Zetesis.Sub.second'\n",
            ),
            (
                "axiom-audit.txt",
                "'Zetesis.first' depends on axioms: [propext,\n Classical.choice,\n Quot.sound]\n'Zetesis.Sub.second'' does not depend on any axioms\n",
            ),
            (
                "verification/current/build.log",
                "Build completed successfully.\n",
            ),
        ] {
            fixture.write(path, content);
        }
        fixture.write("theorems.json", &fixture.entries.to_string());
        fixture.write(
            "verification/current/audit.log",
            &fixture.read("axiom-audit.txt"),
        );
        fixture.refresh();
        fixture
    }
    pub fn root(&self) -> &Path {
        self.directory.path()
    }
    pub fn write(&self, path: &str, content: &str) {
        let path = self.root().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    pub fn read(&self, path: &str) -> String {
        fs::read_to_string(self.root().join(path)).unwrap()
    }
    pub fn digest(&self, path: &str) -> String {
        format!(
            "{:x}",
            Sha256::digest(fs::read(self.root().join(path)).unwrap())
        )
    }
    pub fn save(&self) {
        self.write("verification.json", &self.record.to_string());
    }
    pub fn refresh(&mut self) {
        for section in [
            "source_sha256",
            "generated_artifact_sha256",
            "verification_log_sha256",
        ] {
            let names: Vec<_> = self.record[section]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            for name in names {
                self.record[section][&name] = self.digest(&name).into();
            }
        }
        self.save();
    }
    pub fn verify(
        &self,
    ) -> Result<zetesis_maintenance::proofs::Summary, zetesis_maintenance::Error> {
        zetesis_maintenance::proofs::verify(
            self.root(),
            "verification.json",
            zetesis_maintenance::proofs::Limits::default(),
        )
    }
    pub fn reject(&self) {
        assert!(self.verify().is_err());
    }
}
