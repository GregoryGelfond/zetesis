//! Pre-raise source restrictions and bounded iterative syntax traversal.

use themelios_syntax::ast::{self, AstToken};
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::{AstNode, SyntaxKind, SyntaxNode, WalkEvent};

use crate::diagnostic::unsupported;
use crate::{AdmissionFailure, AdmissionOptions, InputLimit, ProfileFeature};

/// Check the complete syntax walk before inspecting source statements. This
/// preserves node/depth failures ahead of statement-profile failures.
pub(crate) fn check_formula(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    includes: bool,
) -> Result<usize, AdmissionFailure> {
    let nodes = check_traversal(parsed, options)?;
    for statement in parsed.tree().statements() {
        check_statement(&statement, parsed, options, includes)?;
    }
    Ok(nodes)
}

fn check_statement(
    statement: &ast::Statement,
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    includes: bool,
) -> Result<(), AdmissionFailure> {
    if let ast::Statement::ProgramPart(part) = statement {
        check_program_part(part, parsed)?;
        return Ok(());
    }
    if matches!(statement, ast::Statement::Const(_)) {
        return Ok(());
    }
    if matches!(
        statement,
        ast::Statement::Defined(_) | ast::Statement::Show(_) | ast::Statement::Project(_)
    ) {
        let body = match statement {
            ast::Statement::Show(show) => show.body(),
            ast::Statement::Project(project) => project.body(),
            _ => None,
        };
        if let Some(body) = body {
            for (index, element) in body.elements().enumerate() {
                if index >= options.max_body_elements {
                    return Err(AdmissionFailure::Limit {
                        resource: InputLimit::BodyElements,
                        limit: options.max_body_elements,
                        observed: index + 1,
                        location: parsed.location(element.syntax().text_range()).into(),
                    });
                }
            }
        }
        return Ok(());
    }
    if let ast::Statement::WeakConstraint(weak) = statement {
        check_weak(weak, parsed, options)?;
        return Ok(());
    }
    if matches!(statement, ast::Statement::Optimize(_)) {
        return Ok(());
    }
    if includes && matches!(statement, ast::Statement::Include(_)) {
        return Ok(());
    }
    let ast::Statement::Rule(rule) = statement else {
        return Err(unsupported(
            ProfileFeature::Statement,
            parsed.location(statement.syntax().text_range()),
        ));
    };
    match rule.head() {
        None
        | Some(ast::Head::Literal(_) | ast::Head::Aggregate(_) | ast::Head::Disjunction(_)) => {}
        Some(head) => {
            return Err(unsupported(
                ProfileFeature::Head,
                parsed.location(head.syntax().text_range()),
            ));
        }
    }
    if let Some(body) = rule.body() {
        for (index, element) in body.elements().enumerate() {
            let location = parsed.location(element.syntax().text_range());
            if index >= options.max_body_elements {
                return Err(AdmissionFailure::Limit {
                    resource: InputLimit::BodyElements,
                    limit: options.max_body_elements,
                    observed: index + 1,
                    location: location.into(),
                });
            }
            if !matches!(
                element,
                ast::BodyElement::Literal(_)
                    | ast::BodyElement::Aggregate(_)
                    | ast::BodyElement::ConditionalLiteral(_)
            ) {
                return Err(unsupported(ProfileFeature::BodyElement, location));
            }
        }
    }
    Ok(())
}

fn check_program_part(
    part: &ast::ProgramStatement,
    parsed: &Parse<ast::Program>,
) -> Result<(), AdmissionFailure> {
    // Raising merges repeated base sections and may erase empty parts.
    // Check every original delimiter before that representation change.
    let base = part.name().is_some_and(|name| name.text() == "base");
    let parameterless = part
        .parameters()
        .is_none_or(|parameters| parameters.names().next().is_none());
    if !base || !parameterless {
        return Err(unsupported(
            ProfileFeature::ProgramPart,
            parsed.location(part.syntax().text_range()),
        ));
    }
    Ok(())
}

fn check_traversal(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
) -> Result<usize, AdmissionFailure> {
    let mut observation_depth = None;
    let mut count = 0;
    let mut depth = 0;
    for event in parsed.syntax().preorder() {
        match event {
            WalkEvent::Enter(node) => {
                count += 1;
                depth += 1;
                if ast::ShowStatement::cast(node.clone()).is_some() {
                    observation_depth = Some(depth);
                }
                check_node(
                    &node,
                    parsed,
                    options,
                    observation_depth.is_some(),
                    count,
                    depth,
                )?;
            }
            WalkEvent::Leave(_) => {
                if observation_depth == Some(depth) {
                    observation_depth = None;
                }
                depth -= 1;
            }
        }
    }
    Ok(count)
}

fn check_node(
    node: &SyntaxNode,
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    observation: bool,
    count: usize,
    depth: usize,
) -> Result<(), AdmissionFailure> {
    let location = parsed.location(node.text_range());
    for (resource, observed, limit) in [
        (InputLimit::SyntaxNodes, count, options.max_syntax_nodes),
        (InputLimit::SyntaxDepth, depth, options.max_syntax_depth),
    ] {
        if observed > limit {
            return Err(AdmissionFailure::Limit {
                resource,
                limit,
                observed,
                location: location.into(),
            });
        }
    }
    if !observation
        && matches!(
            node.kind(),
            SyntaxKind::EXTERNAL_TERM | SyntaxKind::SPLICE_TERM
        )
    {
        return Err(unsupported(ProfileFeature::Term, location));
    }
    Ok(())
}

fn check_weak(
    weak: &ast::WeakConstraint,
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
) -> Result<(), AdmissionFailure> {
    if let Some(body) = weak.body() {
        for (index, element) in body.elements().enumerate() {
            let location = parsed.location(element.syntax().text_range());
            if index >= options.max_body_elements {
                return Err(AdmissionFailure::Limit {
                    resource: InputLimit::BodyElements,
                    limit: options.max_body_elements,
                    observed: index + 1,
                    location: location.into(),
                });
            }
            if !matches!(
                element,
                ast::BodyElement::Literal(_)
                    | ast::BodyElement::Aggregate(_)
                    | ast::BodyElement::ConditionalLiteral(_)
            ) {
                return Err(unsupported(ProfileFeature::Objective, location));
            }
        }
    }
    Ok(())
}
