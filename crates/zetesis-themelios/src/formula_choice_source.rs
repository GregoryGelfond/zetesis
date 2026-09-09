//! Original Boolean choice rules beside the dependency's set-shaped program.
//!
//! Ordinary Boolean choices count source element occurrences, with all local
//! witnesses of one occurrence coalesced. Equal complete rules may have unequal
//! occurrence counts after their nested elements are deduplicated. A program set
//! therefore cannot recover this information from its merged root provenance.
//! This catalog raises each affected original rule independently and never
//! collects those variants into a program set. Explicit aggregate tuples and
//! ordinary atomic choices retain the dependency's existing identity.
//!
//! The pinned dependency exposes a fragment parser and raiser, but no public
//! operation to raise one node of a program parse. A token-source view selects
//! one original AST rule: tokens within it come unchanged from `Lexer`; bytes
//! outside it become opaque trivia. The view preserves the original text and
//! coordinates, and the resulting rule tree must equal the original tree.
//! This adds no ASP grammar or lexical recognition.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use themelios_base::line::{OffsetOutOfBounds, PositionRefusal};
use themelios_base::source::{NotCharBoundary, Source, SourceId};
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::{Head, LiteralInner, Program, Statement};
use themelios_program::provenance::{Origin, Provenance, WithProvenance};
use themelios_program::raise::raise_statement;
use themelios_syntax::ast::{self, HasDocs};
use themelios_syntax::dialect::Dialect;
use themelios_syntax::lexer::Lexer;
use themelios_syntax::parse::{NestingLimit, Parse, parse_statement};
use themelios_syntax::token::{LexMode, Token, TokenSource};
use themelios_syntax::tree::{AstNode, SyntaxKind};

use crate::expansion::Budget;
use crate::{ExpansionResource, FormulaFailure};

/// Only `include` produces variants, from an already admitted original parse.
/// Cumulative work charges bytes copied into temporary green trees, bytes parsed,
/// and selected syntax nodes raised. Retained nodes/locations reserve Values and
/// Origins conservatively before allocation. Temporary space is O(source bytes);
/// retained space is O(selected syntax and payload). Allocator overhead is excluded.
/// Without the work ceiling, repeated fragment parsing is O(rules × source bytes).
#[derive(Default)]
pub(crate) struct Catalog {
    variants: Vec<WithProvenance<Statement>>,
    locations: BTreeMap<Location, usize>,
}

impl Catalog {
    pub(crate) fn include(
        &mut self,
        source: &Source,
        parsed: &Parse<ast::Program>,
        budget: &mut Budget,
    ) -> Result<(), FormulaFailure> {
        for statement in parsed.tree().statements() {
            let ast::Statement::Rule(rule) = &statement else {
                continue;
            };
            let Some(ast::Head::Aggregate(ast::Aggregate::Set(choice))) = rule.head() else {
                continue;
            };
            if !choice.elements().any(|element| is_boolean(&element)) {
                continue;
            }
            let location = parsed.location(statement.syntax().text_range());
            let nodes = statement.syntax().descendants().count() as u128;
            // The parser owns a green copy of every token, including opaque
            // prefix/suffix trivia. Reserve another pass for tree/content checks.
            // Charge these full-source passes before the fragment parser runs.
            budget.charge(
                ExpansionResource::TermWork,
                (source.text().len() as u128).saturating_mul(3) + nodes,
                location,
            )?;
            budget.charge(ExpansionResource::Values, nodes, location)?;
            // Nested owned provenance, comparison sets and the retained root index
            // each need at most one location per original syntax node.
            budget.charge(
                ExpansionResource::Origins,
                nodes.saturating_mul(4),
                location,
            )?;
            if source.id() != parsed.source()
                || source
                    .slice(location.span)
                    .ok()
                    .is_none_or(|text| statement.syntax().text() != text)
            {
                return Err(FormulaFailure::ChoiceSource { location });
            }
            let window = StatementWindow {
                lexer: Lexer::new(source, parsed.dialect()),
                span: location.span,
            };
            let fragment = parse_statement(&window, NestingLimit::DEFAULT);
            if !fragment.diagnostics().is_empty()
                || fragment.tree().statement().is_none_or(|selected| {
                    selected.syntax().text_range() != statement.syntax().text_range()
                        || selected.syntax().green() != statement.syntax().green()
                })
            {
                return Err(FormulaFailure::ChoiceSource { location });
            }
            let (raised, diagnostics) = raise_statement(&fragment);
            let Some(raised) = raised.filter(|_| diagnostics.is_empty()) else {
                return Err(FormulaFailure::ChoiceSource { location });
            };
            let expected: BTreeSet<_> = choice
                .elements()
                .filter(is_boolean)
                .map(|element| parsed.location(element.syntax().text_range()))
                .collect();
            if boolean_origins(&raised) != Some(expected) {
                return Err(FormulaFailure::ChoiceSource { location });
            }
            let mut provenance = Provenance::from(Origin::Parsed(location));
            let docs: Vec<_> = rule
                .doc_lines()
                .map(|line| line.content().to_owned())
                .collect();
            if !docs.is_empty() {
                provenance = provenance.with_doc(docs.join("\n"));
            }
            match self.locations.entry(location) {
                Entry::Occupied(_) => return Err(FormulaFailure::ChoiceSource { location }),
                Entry::Vacant(entry) => {
                    entry.insert(self.variants.len());
                }
            }
            self.variants.push(WithProvenance::new(raised, provenance));
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

/// The original AST supplies UTF-8-aligned token boundaries. No cursor guesses a
/// statement boundary. Inside the window, the pinned lexer owns every token.
/// Outside it, one positive-length trivia slice advances to the next boundary;
/// EOF occurs only at the unchanged original source length.
struct StatementWindow<'a> {
    lexer: Lexer<'a>,
    span: Span,
}

impl TokenSource for StatementWindow<'_> {
    fn id(&self) -> SourceId {
        self.lexer.id()
    }
    fn dialect(&self) -> Dialect {
        self.lexer.dialect()
    }
    fn text(&self) -> &str {
        self.lexer.text()
    }

    fn token_at(&self, at: ByteOffset, mode: LexMode) -> Result<Token<'_>, PositionRefusal> {
        let offset = at.get() as usize;
        let text = self.text();
        if offset > text.len() {
            return Err(PositionRefusal::OutOfBounds(OffsetOutOfBounds {
                offset: at,
                max: ByteOffset::new(u32::try_from(text.len()).expect("Source length fits u32")),
            }));
        }
        if !text.is_char_boundary(offset) {
            return Err(PositionRefusal::NotCharBoundary(NotCharBoundary {
                offset: at,
            }));
        }
        if self.span.contains(at) || offset == text.len() {
            return self.lexer.token_at(at, mode);
        }
        let end = if at < self.span.start() {
            self.span.start().get() as usize
        } else {
            text.len()
        };
        Ok(Token {
            kind: SyntaxKind::WHITESPACE,
            text: &text[offset..end],
        })
    }
}

#[cfg(test)]
#[path = "formula_choice_source_tests.rs"]
mod tests;
