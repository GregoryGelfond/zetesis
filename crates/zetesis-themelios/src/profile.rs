//! Pre-raise source restrictions and bounded iterative syntax traversal.

use themelios_syntax::ast::{self, AstToken, HasGuards};
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
    let [result] = check_profiles(parsed, options, extended, includes, [formula]);
    result
}

/// Both verdicts share one syntax traversal and one statement iteration. A
/// refusal remains local to its profile and retains that profile's first error.
pub(crate) fn check_automatic(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    includes: bool,
) -> [Result<usize, AdmissionFailure>; 2] {
    check_profiles(parsed, options, true, includes, [false, true])
}

fn check_profiles<const N: usize>(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    extended: bool,
    includes: bool,
    formulas: [bool; N],
) -> [Result<usize, AdmissionFailure>; N] {
    let mut results = check_traversal(parsed, options, extended, formulas);
    for statement in parsed.tree().statements() {
        for (formula, result) in formulas.into_iter().zip(&mut results) {
            if result.is_ok()
                && let Err(error) =
                    check_statement(&statement, parsed, options, extended, includes, formula)
            {
                *result = Err(error);
            }
        }
        if results.iter().all(Result::is_err) {
            break;
        }
    }
    results
}

fn check_statement(
    statement: &ast::Statement,
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    extended: bool,
    includes: bool,
    formula: bool,
) -> Result<(), AdmissionFailure> {
    if let ast::Statement::ProgramPart(part) = statement {
        check_program_part(part, parsed)?;
        return Ok(());
    }
    if extended && matches!(statement, ast::Statement::Const(_)) {
        return Ok(());
    }
    if extended
        && matches!(
            statement,
            ast::Statement::Defined(_) | ast::Statement::Show(_) | ast::Statement::Project(_)
        )
    {
        let body = match statement {
            ast::Statement::Show(show) => show.body(),
            ast::Statement::Project(project) => project.body(),
            _ => None,
        };
        if formula && let Some(body) = body {
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
        crate::metadata::check_syntax(statement, parsed, formula)?;
        return Ok(());
    }
    if formula {
        if let ast::Statement::WeakConstraint(weak) = statement {
            check_weak(weak, parsed, options)?;
            return Ok(());
        }
        if matches!(statement, ast::Statement::Optimize(_)) {
            return Ok(());
        }
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
    if formula {
        match rule.head() {
            None
            | Some(ast::Head::Literal(_) | ast::Head::Aggregate(_) | ast::Head::Disjunction(_)) => {
            }
            Some(head) => {
                return Err(unsupported(
                    ProfileFeature::Head,
                    parsed.location(head.syntax().text_range()),
                ));
            }
        }
    } else {
        check_head(rule, parsed)?;
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
            if !(matches!(element, ast::BodyElement::Literal(_))
                || formula
                    && matches!(
                        element,
                        ast::BodyElement::Aggregate(_) | ast::BodyElement::ConditionalLiteral(_)
                    ))
            {
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

fn check_traversal<const N: usize>(
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    extended: bool,
    formulas: [bool; N],
) -> [Result<usize, AdmissionFailure>; N] {
    let mut results = std::array::from_fn(|_| Ok(0));
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
                for (formula, result) in formulas.into_iter().zip(&mut results) {
                    if result.is_ok() {
                        *result = check_node(
                            &node,
                            parsed,
                            options,
                            extended,
                            formula && observation_depth.is_some(),
                            count,
                            depth,
                        )
                        .map(|()| count);
                    }
                }
                if results.iter().all(Result::is_err) {
                    break;
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
    results
}

fn check_node(
    node: &SyntaxNode,
    parsed: &Parse<ast::Program>,
    options: AdmissionOptions,
    extended: bool,
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
    if extended {
        if !observation
            && matches!(
                node.kind(),
                SyntaxKind::EXTERNAL_TERM | SyntaxKind::SPLICE_TERM
            )
        {
            return Err(unsupported(ProfileFeature::Term, location));
        }
    } else {
        check_term_syntax(node, parsed)?;
    }
    Ok(())
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
