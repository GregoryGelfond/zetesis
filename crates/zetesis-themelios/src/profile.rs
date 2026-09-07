//! Pre-raise source restrictions and bounded iterative syntax traversal.

use themelios_syntax::ast::{self, HasGuards};
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::{AstNode, SyntaxKind, SyntaxNode, WalkEvent};

use crate::diagnostic::unsupported;
use crate::{AdmissionFailure, AdmissionOptions, InputLimit, ProfileFeature};

pub(crate) fn check(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
) -> Result<(), AdmissionFailure> {
    check_profile(parsed, options, false, false, false).map(|_| ())
}

pub(crate) fn check_extended(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
) -> Result<(), AdmissionFailure> {
    check_profile(parsed, options, true, false, false).map(|_| ())
}

pub(crate) fn check_bundle(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
) -> Result<usize, AdmissionFailure> {
    check_profile(parsed, options, true, true, false)
}

pub(crate) fn check_formula(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    includes: bool,
) -> Result<usize, AdmissionFailure> {
    check_profile(parsed, options, true, includes, true)
}

fn check_profile(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    extended: bool,
    includes: bool,
    formula: bool,
) -> Result<usize, AdmissionFailure> {
    let nodes = check_traversal(parsed, options, extended, formula)?;
    for statement in parsed.tree().statements() {
        if extended && matches!(statement, ast::Statement::Const(_)) {
            continue;
        }
        if extended
            && matches!(
                statement,
                ast::Statement::Defined(_) | ast::Statement::Show(_)
            )
        {
            if formula
                && let ast::Statement::Show(show) = &statement
                && let Some(body) = show.body()
            {
                for (index, element) in body.elements().enumerate() {
                    if index >= options.max_body_elements {
                        return Err(AdmissionFailure::Limit {
                            resource: InputLimit::BodyElements,
                            limit: options.max_body_elements,
                            observed: index + 1,
                            location: parsed.location(element.syntax().text_range()),
                        });
                    }
                }
            }
            crate::metadata::check_syntax(&statement, parsed, formula)?;
            continue;
        }
        if formula {
            if let ast::Statement::WeakConstraint(weak) = &statement {
                check_weak(weak, parsed, options)?;
                continue;
            }
            if matches!(statement, ast::Statement::Optimize(_)) {
                continue;
            }
        }
        if includes && matches!(statement, ast::Statement::Include(_)) {
            continue;
        }
        let ast::Statement::Rule(rule) = statement else {
            return Err(unsupported(
                ProfileFeature::Statement,
                parsed.location(statement.syntax().text_range()),
            ));
        };
        if formula {
            match rule.head() {
                None
                | Some(ast::Head::Literal(_) | ast::Head::Aggregate(ast::Aggregate::Set(_))) => {}
                Some(ast::Head::Disjunction(head)) => {
                    for element in head.elements() {
                        if matches!(element, ast::DisjunctionElement::ConditionalLiteral(_)) {
                            return Err(unsupported(
                                ProfileFeature::ConditionalDisjunction,
                                parsed.location(element.syntax().text_range()),
                            ));
                        }
                    }
                }
                Some(head) => {
                    return Err(unsupported(
                        ProfileFeature::Head,
                        parsed.location(head.syntax().text_range()),
                    ));
                }
            }
        } else {
            check_head(&rule, parsed)?;
        }
        if let Some(body) = rule.body() {
            for (index, element) in body.elements().enumerate() {
                let location = parsed.location(element.syntax().text_range());
                if index >= options.max_body_elements {
                    return Err(AdmissionFailure::Limit {
                        resource: InputLimit::BodyElements,
                        limit: options.max_body_elements,
                        observed: index + 1,
                        location,
                    });
                }
                if !(matches!(element, ast::BodyElement::Literal(_))
                    || formula
                        && matches!(
                            element,
                            ast::BodyElement::Aggregate(_)
                                | ast::BodyElement::ConditionalLiteral(_)
                        ))
                {
                    return Err(unsupported(ProfileFeature::BodyElement, location));
                }
            }
        }
    }
    Ok(nodes)
}

fn check_traversal(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    extended: bool,
    formula: bool,
) -> Result<usize, AdmissionFailure> {
    let mut observation_depth = None;
    let mut count = 0;
    let mut depth = 0;
    for event in parsed.syntax().preorder() {
        match event {
            WalkEvent::Enter(node) => {
                count += 1;
                depth += 1;
                if formula && ast::ShowStatement::cast(node.clone()).is_some() {
                    observation_depth = Some(depth);
                }
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
                            location,
                        });
                    }
                }
                if extended {
                    if observation_depth.is_none()
                        && matches!(
                            node.kind(),
                            SyntaxKind::EXTERNAL_TERM | SyntaxKind::SPLICE_TERM
                        )
                    {
                        return Err(unsupported(ProfileFeature::Term, location));
                    }
                } else {
                    check_term_syntax(&node, parsed)?;
                }
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

fn check_term_syntax(
    node: &SyntaxNode,
    parsed: &Parse<ast::Program>,
) -> Result<(), AdmissionFailure> {
    let location = parsed.location(node.text_range());
    match node.kind() {
        SyntaxKind::BINARY_TERM
        | SyntaxKind::EXTERNAL_TERM
        | SyntaxKind::ABS_TERM
        | SyntaxKind::SPLICE_TERM => {
            return Err(unsupported(ProfileFeature::Term, location));
        }
        SyntaxKind::UNARY_TERM => {
            let unary = ast::UnaryTerm::cast(node.clone()).expect("the syntax kind was checked");
            let mut operators = unary.operators();
            let direct_minus = operators
                .next()
                .is_some_and(|operator| operator.kind() == SyntaxKind::MINUS)
                && operators.next().is_none();
            let signed_value = matches!(
                unary.operand(),
                Some(ast::Term::Constant(constant))
                    if matches!(constant.constant(), Some(ast::Constant::Number(_) | ast::Constant::Symbol(_)))
            ) || matches!(unary.operand(), Some(ast::Term::Function(_)));
            if !direct_minus || !signed_value {
                return Err(unsupported(ProfileFeature::Term, location));
            }
        }
        SyntaxKind::POOL => {
            let pool = ast::Pool::cast(node.clone()).expect("the syntax kind was checked");
            if pool.tuples().count() > 1 {
                return Err(unsupported(ProfileFeature::Term, location));
            }
        }
        SyntaxKind::ARGUMENTS => {
            let arguments =
                ast::Arguments::cast(node.clone()).expect("the syntax kind was checked");
            if arguments.alternatives().count() != 1 {
                return Err(unsupported(ProfileFeature::PooledArguments, location));
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_head(rule: &ast::Rule, parsed: &Parse<ast::Program>) -> Result<(), AdmissionFailure> {
    match rule.head() {
        None | Some(ast::Head::Literal(_)) => Ok(()),
        Some(ast::Head::Aggregate(ast::Aggregate::Set(choice))) => {
            let location = parsed.location(choice.syntax().text_range());
            if choice.left_guard().is_some() || choice.right_guard().is_some() {
                return Err(unsupported(ProfileFeature::BoundedChoice, location));
            }
            let mut elements = choice.elements();
            let Some(element) = elements.next() else {
                return Err(unsupported(ProfileFeature::ChoiceCardinality, location));
            };
            if elements.next().is_some() {
                return Err(unsupported(ProfileFeature::ChoiceCardinality, location));
            }
            if !matches!(element, ast::SetElement::Literal(_)) {
                return Err(unsupported(ProfileFeature::ConditionalChoice, location));
            }
            Ok(())
        }
        Some(head) => Err(unsupported(
            ProfileFeature::Head,
            parsed.location(head.syntax().text_range()),
        )),
    }
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
                    location,
                });
            }
            if !matches!(element, ast::BodyElement::Literal(_)) {
                return Err(unsupported(ProfileFeature::Objective, location));
            }
        }
    }
    Ok(())
}
