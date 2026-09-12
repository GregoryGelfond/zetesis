//! Original Boolean choice rules beside the dependency's set-shaped program.
//!
//! Ordinary Boolean choices count source element occurrences, with all local
//! witnesses of one occurrence coalesced. Equal complete rules may have unequal
//! occurrence counts after their nested elements are deduplicated. A program set
//! therefore cannot recover this information from its merged root provenance.
//! The upstream occurrence stream retains that information before collection.
//! This catalog copies only affected rules, then consumes the stream into the
//! ordinary program. Explicit aggregate tuples and atomic choices retain the
//! dependency's existing identity. No source lexing or fragment parsing occurs.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use themelios_base::span::Location;
use themelios_program::program::{Head, LiteralInner, Program, Statement};
use themelios_program::provenance::{Origin, WithProvenance};
use themelios_program::raise::{Occurrences, raise_occurrences};
use themelios_syntax::ast;
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::AstNode;

use crate::expansion::Budget;
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, SourceMetadata, metadata};

/// Only `include` produces variants, from an already admitted original parse.
/// The upstream raise materializes one occurrence owner, bounded by the admitted
/// source tree. Cumulative work charges selected source bytes plus syntax nodes
/// for payload copying and traversal. Retained nodes/locations reserve Values
/// and Origins conservatively before copying. Retained variants and the upstream
/// owner coexist until collection. Allocator overhead is excluded.
#[derive(Default)]
pub(crate) struct Catalog {
    variants: Vec<WithProvenance<Statement>>,
    locations: BTreeMap<Location, usize>,
}

impl Catalog {
    fn include(
        &mut self,
        parsed: &Parse<ast::Program>,
        occurrences: &Occurrences,
        budget: &mut Budget,
    ) -> Result<(), FormulaFailure> {
        let mut syntax = parsed.tree().statements();
        for occurrence in occurrences.occurrences() {
            if !has_boolean_choice(occurrence.statement().get()) {
                continue;
            }
            let location = occurrence.location();
            // Both views originate from this one parse. Advance monotonically;
            // part delimiters and unaffected statements need no retained entry.
            let statement = syntax
                .find(|statement| parsed.location(statement.syntax().text_range()) == location)
                .ok_or(FormulaFailure::ChoiceSource { location })?;
            let nodes = statement.syntax().descendants().count() as u128;
            budget.charge(
                ExpansionResource::TermWork,
                u128::from(location.span.len()) + nodes,
                location,
            )?;
            budget.charge(ExpansionResource::Values, nodes, location)?;
            // Nested provenance, validation sets and the root index reserve a
            // conservative four locations per original syntax node.
            budget.charge(
                ExpansionResource::Origins,
                nodes.saturating_mul(4),
                location,
            )?;
            let ast::Statement::Rule(rule) = &statement else {
                return Err(FormulaFailure::ChoiceSource { location });
            };
            let Some(ast::Head::Aggregate(ast::Aggregate::Set(choice))) = rule.head() else {
                return Err(FormulaFailure::ChoiceSource { location });
            };
            let expected: BTreeSet<_> = choice
                .elements()
                .filter(is_boolean)
                .map(|element| parsed.location(element.syntax().text_range()))
                .collect();
            if boolean_origins(occurrence.statement().get()) != Some(expected) {
                return Err(FormulaFailure::ChoiceSource { location });
            }
            match self.locations.entry(location) {
                Entry::Occupied(_) => return Err(FormulaFailure::ChoiceSource { location }),
                Entry::Vacant(entry) => {
                    entry.insert(self.variants.len());
                }
            }
            self.variants.push(occurrence.statement().clone());
        }
        Ok(())
    }

    /// Constants still resolve from `source`. Only the rule compilation iterator
    /// substitutes variants, once each, for every affected merged carrier. The
    /// caller supplies the raised union of exactly the parses included above.
    pub(crate) fn statements<'a>(
        &'a self,
        source: &'a Program,
        fallback: Location,
    ) -> impl Iterator<Item = Result<&'a WithProvenance<Statement>, FormulaFailure>> {
        source
            .statements()
            .filter_map(move |carrier| {
                if !has_boolean_choice(carrier.get()) {
                    return Some(Ok(carrier));
                }
                let mut found = false;
                for origin in carrier.provenance().origins() {
                    if let Origin::Parsed(location) = origin {
                        found = true;
                        if self
                            .locations
                            .get(location)
                            .is_none_or(|&index| self.variants[index].get() != carrier.get())
                        {
                            return Some(Err(FormulaFailure::ChoiceSource {
                                location: *location,
                            }));
                        }
                    }
                }
                if found {
                    None
                } else {
                    Some(Err(FormulaFailure::ChoiceSource {
                        location: crate::extended::origin(carrier, fallback),
                    }))
                }
            })
            .chain(self.variants.iter().map(Ok))
    }
}

/// Raise once, preserving diagnostics before metadata and copy admission. Only
/// this composition pairs the original parse with its occurrence stream.
pub(crate) fn raise(
    parsed: &Parse<ast::Program>,
    metadata: &mut SourceMetadata,
    budget: &mut Budget,
    choices: &mut Catalog,
) -> Result<Program, FormulaFailure> {
    let occurrences = raise_occurrences(parsed);
    if !occurrences.diagnostics().is_empty() {
        return Err(AdmissionFailure::Raise(occurrences.diagnostics().to_vec()).into());
    }
    metadata::collect_occurrences(&occurrences, metadata)?;
    choices.include(parsed, &occurrences, budget)?;
    Ok(occurrences.into_raised().into_program())
}

fn is_boolean(element: &ast::SetElement) -> bool {
    let literal = match element {
        ast::SetElement::Literal(literal) => Some(literal.clone()),
        ast::SetElement::ConditionalLiteral(conditional) => conditional.literal(),
    };
    literal.is_some_and(|literal| {
        matches!(
            literal.inner(),
            Some(ast::LiteralInner::True(_) | ast::LiteralInner::False(_))
        )
    })
}

fn has_boolean_choice(statement: &Statement) -> bool {
    let Statement::Rule(rule) = statement else {
        return false;
    };
    let Head::Choice(choice) = rule.head().get() else {
        return false;
    };
    choice.elements().any(|element| {
        matches!(
            element.get().literal().inner,
            LiteralInner::True | LiteralInner::False
        )
    })
}

/// The raised nodes must retain exactly all original Boolean occurrence ranges.
/// A constructed/transformed origin alone cannot establish an occurrence.
fn boolean_origins(statement: &Statement) -> Option<BTreeSet<Location>> {
    let Statement::Rule(rule) = statement else {
        return None;
    };
    let Head::Choice(choice) = rule.head().get() else {
        return None;
    };
    let mut locations = BTreeSet::new();
    for element in choice.elements().filter(|element| {
        matches!(
            element.get().literal().inner,
            LiteralInner::True | LiteralInner::False
        )
    }) {
        let mut found = false;
        for origin in element.provenance().origins() {
            if let Origin::Parsed(location) = origin {
                found = true;
                if !locations.insert(*location) {
                    return None;
                }
            }
        }
        if !found {
            return None;
        }
    }
    Some(locations)
}

#[cfg(test)]
#[path = "formula_choice_source_tests.rs"]
mod tests;
