//! Full native records are compared without confusing catalogs with meanings.

use super::*;
use crate::performance::matrix::native_family::accept;

pub(super) fn document(names: &[&str]) -> Value {
    let mut table = Vec::new();
    let records: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(number, name)| {
            let mut spelled = Vec::new();
            let index = table.iter().position(|old| old == name).unwrap_or_else(|| {
                table.push(*name);
                spelled.push(json!({"predicate":name,"sign":"positive","arguments":[]}));
                table.len() - 1
            });
            json!({"number":number+1,"model":{"atoms":spelled,"full_model":[index],
            "shown":{"atom_indices":[],"terms":[]},"costs":null}})
        })
        .collect();
    let (mut value, _) = crate::performance::matrix::fixtures::fixture();
    value["schema"] = json!(2);
    value["format"] = json!("zetesis");
    value["models"] = json!(records);
    value["outcome"] = json!({"status":"satisfiable","completion":"exhausted",
        "coverage":"exhausted","published_models":names.len(),"verified_models":names.len(),
        "checked":names.len(),"interruption":null,"optimization":null,"error":null});
    value
}

fn parse(document: &Value) -> answers::native_json::NativeAnswers {
    answers::native_json::parse(
        &serde_json::to_vec(document).unwrap(),
        answers::native_json::Limits::default(),
    )
    .unwrap()
}

#[test]
fn native_catalog_order_does_not_change_family_identity() {
    let mut baseline = None;
    accept(&mut baseline, parse(&document(&["a", "b"]))).unwrap();
    accept(&mut baseline, parse(&document(&["b", "a"]))).unwrap();
}

#[test]
fn hidden_native_atoms_participate_in_parity() {
    let mut baseline = None;
    accept(&mut baseline, parse(&document(&["a"]))).unwrap();
    assert_eq!(
        accept(&mut baseline, parse(&document(&["b"])))
            .unwrap_err()
            .0,
        Decision::ParityMismatch
    );
    accept(&mut baseline, parse(&document(&["a"]))).unwrap();
}

#[test]
fn repeated_native_records_preserve_multiplicity() {
    let mut baseline = None;
    accept(&mut baseline, parse(&document(&["a", "a", "b"]))).unwrap();
    assert_eq!(
        accept(&mut baseline, parse(&document(&["a", "b", "b"])))
            .unwrap_err()
            .0,
        Decision::ParityMismatch
    );
}

#[test]
fn shown_native_terms_preserve_multiplicity() {
    let mut first = document(&["a"]);
    first["models"][0]["model"]["shown"]["terms"] = json!([
        [{"kind":"symbol","value":"t"}], [{"kind":"symbol","value":"t"}]]);
    let mut second = first.clone();
    second["models"][0]["model"]["shown"]["terms"]
        .as_array_mut()
        .unwrap()
        .pop();
    let mut baseline = Some(parse(&first));
    assert_eq!(
        accept(&mut baseline, parse(&second)).unwrap_err().0,
        Decision::ParityMismatch
    );
}

#[test]
fn shown_native_atoms_remain_part_of_record_identity() {
    let first = document(&["a"]);
    let mut second = first.clone();
    second["models"][0]["model"]["shown"]["atom_indices"] = json!([0]);
    let mut baseline = Some(parse(&first));
    assert_eq!(
        accept(&mut baseline, parse(&second)).unwrap_err().0,
        Decision::ParityMismatch
    );
}
