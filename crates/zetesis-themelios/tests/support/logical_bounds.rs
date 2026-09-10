//! Declared complete answers for literal finite logical-bound programs.

use super::Models;

pub const RELATIONS: [&str; 6] = ["=", "!=", "<", "<=", ">", ">="];

// These are source-order facts, independent of Value's representation/order.
pub const BOUNDS: [(&str, [bool; 6]); 7] = [
    ("#inf", [false, true, false, false, true, true]),
    ("word", [false, true, true, true, false, false]),
    ("\"#inf\"", [false, true, true, true, false, false]),
    ("-word", [false, true, true, true, false, false]),
    ("f(1)", [false, true, true, true, false, false]),
    ("(1,)", [false, true, true, true, false, false]),
    ("#sup", [false, true, true, true, false, false]),
];

pub fn expected(records: &[&[&str]]) -> Models {
    records
        .iter()
        .map(|record| record.iter().map(|name| (*name).to_owned()).collect())
        .collect()
}

pub fn sources() -> Vec<(String, Models)> {
    let optional = expected(&[&[], &["a"], &["b"], &["a", "b"]]);
    let mut result = Vec::new();
    for (bound, accepted) in BOUNDS {
        for (index, (relation, accepted)) in RELATIONS.into_iter().zip(accepted).enumerate() {
            let reversed = ["=", "!=", ">", ">=", "<", "<="][index];
            for head in [
                "{a;b}",
                "#count{1:a;2:b}",
                "#sum{-2:a;3:b}",
                "#sum+{0:a;3:b}",
            ] {
                for source in [
                    format!("{head}{relation}{bound}."),
                    format!("{bound}{reversed}{head}."),
                ] {
                    result.push((
                        source,
                        if accepted {
                            optional.clone()
                        } else {
                            Models::new()
                        },
                    ));
                }
            }
            for aggregate in ["#count{1:a;2:b}", "#sum{-2:a;3:b}", "#sum+{0:a;3:b}"] {
                let models: Models = optional
                    .iter()
                    .map(|model| {
                        let mut model = model.clone();
                        if accepted {
                            model.insert("q".into());
                        }
                        model
                    })
                    .collect();
                for source in [
                    format!("{{a;b}}.q:-{aggregate}{relation}{bound}."),
                    format!("{{a;b}}.q:-{bound}{reversed}{aggregate}."),
                ] {
                    result.push((source, models.clone()));
                }
            }
        }
    }
    for (source, records) in [
        ("#inf{a}#sup.", expected(&[&[], &["a"]])),
        ("word{a}.", Models::new()),
        ("1{a}word.", expected(&[&["a"]])),
        ("{not not a}word.", expected(&[&[]])),
        ("{a}N:-N=#min{}.", expected(&[&[], &["a"]])),
        ("N{a}:-N=#min{}.", Models::new()),
        ("q:-N=#min{},N<=#count{}.", expected(&[&[]])),
        (
            "d(word).{a}N:-d(N).",
            expected(&[&["d(word)"], &["a", "d(word)"]]),
        ),
        ("{a}f(N):-N=#count{}.", expected(&[&[], &["a"]])),
        ("{a}word:-#false.", expected(&[&[]])),
        ("word{a}:-#false.", expected(&[&[]])),
        ("#count{}<=word.", expected(&[&[]])),
        ("#sum{}>#inf.", expected(&[&[]])),
        ("#sum+{}=word.", Models::new()),
        ("#count{1:a;2:a}<=word.", expected(&[&[], &["a"]])),
        ("#sum{1:a;1:b}<=word.", optional.clone()),
        ("1{#true;#true}word.", expected(&[&[]])),
        ("word<{#false}.", Models::new()),
        ("d(1..2).{#true:d(X)}word.", expected(&[&["d(1)", "d(2)"]])),
        (
            "{d(1);d(2)}.1{a:d(X)}word.",
            expected(&[&["a", "d(1)"], &["a", "d(2)"], &["a", "d(1)", "d(2)"]]),
        ),
        (
            "{a}.q:-not #count{1:a}>=word.",
            expected(&[&["q"], &["a", "q"]]),
        ),
        (
            "{a}.q:-not not #sum{-2:a}<word.",
            expected(&[&["q"], &["a", "q"]]),
        ),
        ("{a}.q:-not #sum+{0:a}<word.", expected(&[&[], &["a"]])),
    ] {
        result.push((source.into(), records));
    }
    result
}
