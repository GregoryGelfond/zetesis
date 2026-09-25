//! Parser-located integer replacement, preserving the admitted include closure.

use themelios_syntax::{
    ast::{self, AstToken},
    dialect::Dialect,
    parse::parse_str,
};

use super::{Budget, ConstantAmendment, Error, examples, reserve};

pub(super) fn edits(
    source: &examples::Source,
    amendments: &[ConstantAmendment<'_>],
    budget: &mut Budget,
) -> Result<Vec<examples::Edit>, Error> {
    source_edits(
        source.source(),
        source.path(),
        source.includes().iter().map(examples::Include::spelling),
        amendments,
        budget,
    )
}

pub(super) fn source_edits<'a>(
    source: &str,
    path: &str,
    mut includes: impl Iterator<Item = &'a str>,
    amendments: &[ConstantAmendment<'_>],
    budget: &mut Budget,
) -> Result<Vec<examples::Edit>, Error> {
    let parsed = parse_str(source, Dialect::Clingo)
        .map_err(|_| Error::Configuration("workload source exceeds parser address space"))?;
    if !parsed.diagnostics().is_empty() {
        return Err(Error::Configuration(
            "workload source has syntax diagnostics",
        ));
    }
    for statement in parsed.tree().statements() {
        if let ast::Statement::Include(include) = statement {
            let path = include.path().ok_or(Error::Configuration(
                "workload library includes are unsupported",
            ))?;
            let spelling = path
                .value(Dialect::Clingo)
                .map_err(|_| Error::Configuration("workload include spelling is invalid"))?;
            if includes.next().is_none_or(|expected| expected != spelling) {
                return Err(Error::Configuration(
                    "workload parsed include closure differs from its manifest",
                ));
            }
        }
    }
    if includes.next().is_some() {
        return Err(Error::Configuration(
            "workload manifest has an unparsed include",
        ));
    }
    let count = amendments
        .iter()
        .filter(|item| item.source_path == path)
        .count();
    let mut edits = reserve(count)?;
    for amendment in amendments.iter().filter(|item| item.source_path == path) {
        let mut matches = parsed.tree().statements().filter_map(|statement| {
            let ast::Statement::Const(constant) = statement else {
                return None;
            };
            constant
                .name()
                .is_some_and(|name| name.syntax().text() == amendment.name)
                .then_some(constant)
        });
        let constant = matches.next().ok_or(Error::Configuration(
            "workload constant declaration is missing",
        ))?;
        if matches.next().is_some() || constant.annotation().is_some() {
            return Err(Error::Configuration(
                "workload constant is repeated or annotated",
            ));
        }
        let term = constant
            .value()
            .ok_or(Error::Configuration("workload constant has no value"))?;
        let ast::Term::Constant(value) = &term else {
            return Err(Error::Configuration(
                "workload constant is not a decimal literal",
            ));
        };
        let Some(ast::Constant::Number(number)) = value.constant() else {
            return Err(Error::Configuration(
                "workload constant is not an integer literal",
            ));
        };
        if number.radix() != ast::Radix::Decimal {
            return Err(Error::Configuration("workload constant is not decimal"));
        }
        let original = number
            .digits()
            .parse::<i32>()
            .map_err(|_| Error::Configuration("workload constant exceeds i32"))?;
        if original != amendment.expected {
            return Err(Error::Configuration(
                "workload constant differs from the expected value",
            ));
        }
        let range = number.syntax().text_range();
        let start = u32::from(range.start()) as usize;
        let end = u32::from(range.end()) as usize;
        let before = source
            .get(start..end)
            .ok_or(Error::Configuration("workload constant span is invalid"))?;
        let after = amendment.replacement.to_string();
        edits.push(
            examples::Edit::replacement(start, end, budget.text(before)?, budget.text(&after)?)
                .map_err(Error::Corpus)?,
        );
    }
    edits.sort_by_key(examples::Edit::start_byte);
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::matrix::WorkloadLimits;

    fn derive(source: &str, expected: i32, replacement: i32) -> Result<String, Error> {
        let mut budget = Budget {
            limits: WorkloadLimits::default(),
            metadata: 0,
            sources: 0,
        };
        let edits = source_edits(
            source,
            "entry.lp",
            std::iter::empty(),
            &[ConstantAmendment {
                source_path: "entry.lp",
                name: "n",
                expected,
                replacement,
            }],
            &mut budget,
        )?;
        examples::derive_source(source, &edits, 1024).map_err(Error::Corpus)
    }

    #[test]
    fn signed_replacements_remain_parseable() {
        for replacement in [-1, i32::MIN, i32::MAX] {
            let derived = derive("#const n=8.\np(n).\n", 8, replacement).unwrap();
            assert_eq!(derived, format!("#const n={replacement}.\np(n).\n"));
            let parsed = parse_str(&derived, Dialect::Clingo).unwrap();
            assert!(parsed.diagnostics().is_empty(), "{replacement}");
        }
    }

    #[test]
    fn only_the_declared_numeric_span_changes() {
        let source = "% #const n=8.\n#const n=8.\np(\"#const n=8.\",8).\n";
        assert_eq!(
            derive(source, 8, 12).unwrap(),
            "% #const n=8.\n#const n=12.\np(\"#const n=8.\",8).\n"
        );
    }

    #[test]
    fn nonliteral_declarations_are_refused() {
        for source in [
            "#const n=-8.",
            "#const n=4+4.",
            "#const n=0x8.",
            "#const n=a.",
            "#const n=8. [default]",
            "#const n=8. [override]",
            "#const n=8. #const n=8.",
            "#const n=2147483648.",
            "p(8).",
        ] {
            assert!(derive(source, 8, 10).is_err(), "{source}");
        }
    }

    #[test]
    fn expected_value_is_a_precondition() {
        assert!(derive("#const n=8.", 7, 10).is_err());
    }

    #[test]
    fn syntax_diagnostics_prevent_derivation() {
        assert!(derive("#const n=8. p(.", 8, 10).is_err());
    }

    #[test]
    fn include_spellings_must_match_the_sealed_closure() {
        for (source, spellings) in [
            ("#include \"a.lp\".", vec![]),
            ("#include \"a.lp\".", vec!["b.lp"]),
            ("p.", vec!["a.lp"]),
            ("#include <incmode>.", vec![]),
        ] {
            let mut budget = Budget {
                limits: WorkloadLimits::default(),
                metadata: 0,
                sources: 0,
            };
            assert!(
                source_edits(source, "entry.lp", spellings.into_iter(), &[], &mut budget).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn constant_amendments_preserve_include_bytes() {
        let source = "#include \"a.lp\".\n#const n=8.\n#include \"b.lp\".\n";
        let mut budget = Budget {
            limits: WorkloadLimits::default(),
            metadata: 0,
            sources: 0,
        };
        let edits = source_edits(
            source,
            "entry.lp",
            ["a.lp", "b.lp"].into_iter(),
            &[ConstantAmendment {
                source_path: "entry.lp",
                name: "n",
                expected: 8,
                replacement: 10,
            }],
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            examples::derive_source(source, &edits, 1024).unwrap(),
            "#include \"a.lp\".\n#const n=10.\n#include \"b.lp\".\n"
        );
    }
}
