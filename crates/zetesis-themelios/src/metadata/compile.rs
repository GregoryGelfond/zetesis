//! Validated metadata compilation from the canonical owned program.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, BodyElement, LiteralInner, Program, Project, Show, Statement,
};
use themelios_program::provenance::Origin;
use themelios_program::symbol::Symbol;
use themelios_program::term::{Term, Variable};

use super::SourceMetadata;
use crate::formula_ir::CompilationOptions;
use crate::{ExpansionFailure, ExpansionLimits, FormulaFailure, ProgramSite, StatementId};

/// Independent input bounds for metadata compilation, before cloning or resolving
/// constants. These measure the borrowed canonical program, not original syntax.
#[derive(Clone, Copy, Debug)]
pub struct MetadataLimits {
    /// All canonical statements scanned, including ignored logical statements.
    pub max_statements: usize,
    /// Cumulative relevant directive, body-element, term and symbol nodes.
    pub max_nodes: usize,
    /// Root-inclusive term/symbol depth, additionally capped at 64.
    pub max_depth: usize,
    /// Cumulative relevant UTF-8 names, variables and strings in the borrowed
    /// canonical input. Logical occurrence accounting repeats per parsed origin,
    /// while canonical spelling is shared; constant expansion has
    /// its own copied-payload limits. Excludes caller-owned program capacity,
    /// provenance annotations, allocator overhead and RSS.
    pub max_text_bytes: usize,
    /// Relevant statement origins inspected, including non-parsed origins.
    pub max_origins: usize,
    /// Existing checked constant normalization and metadata-occurrence ceilings.
    pub expansion: ExpansionLimits,
    /// Existing safe observation-template compilation ceilings.
    pub observations: crate::observation::AdmissionLimits,
    /// Named canonical metadata storage, independent of logical source counts.
    pub storage: super::MetadataStorageLimits,
}
impl Default for MetadataLimits {
    fn default() -> Self {
        Self {
            max_statements: 262_144,
            max_nodes: 65_536,
            max_depth: 64,
            max_text_bytes: 1_048_576,
            max_origins: 16_384,
            expansion: ExpansionLimits::default(),
            observations: crate::observation::AdmissionLimits::default(),
            storage: super::MetadataStorageLimits::default(),
        }
    }
}

/// A resource inspected before metadata compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataResource {
    /// Canonical statements, including ignored logical statements.
    Statements,
    /// Relevant structural/term/symbol nodes.
    Nodes,
    /// Term/symbol depth.
    Depth,
    /// Relevant UTF-8 text.
    TextBytes,
    /// Relevant statement origins.
    Origins,
}

/// A shared-program form outside the metadata compilation contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataFeature {
    /// Only the implicit, unparameterized base part is meaningful here.
    ProgramPart,
    /// A term lies outside the checked metadata subset; constants must also be ground.
    Term,
    /// Null bytes cannot be represented by the source symbol vocabulary.
    NullText,
}

/// Metadata compilation refused; no partially compiled policy is returned.
#[derive(Debug)]
pub enum MetadataError {
    /// A borrowed-input ceiling was exceeded before cloning the relevant input.
    Limit {
        /// Counted resource.
        resource: MetadataResource,
        /// Inclusive configured ceiling.
        limit: usize,
        /// Required count.
        observed: u128,
        /// Logical statement identity and any real parsed or caller-supplied source evidence.
        location: ProgramSite,
    },
    /// A form cannot be interpreted by this metadata-only door.
    Unsupported {
        /// Refused form.
        feature: MetadataFeature,
        /// Logical statement identity and any real parsed or caller-supplied source evidence.
        location: ProgramSite,
    },
    /// Existing checked constant normalization or metadata-occurrence limits refused.
    Expansion(ExpansionFailure),
    /// The existing safe observation compiler or signature construction refused.
    Compilation(FormulaFailure),
}
impl std::fmt::Display for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit {
                resource,
                limit,
                observed,
                ..
            } => write!(f, "metadata {resource:?} count {observed} exceeds {limit}"),
            Self::Unsupported { feature, .. } => write!(f, "metadata refuses {feature:?}"),
            Self::Expansion(error) => error.fmt(f),
            Self::Compilation(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for MetadataError {}

struct Preflight {
    limits: MetadataLimits,
    location: ProgramSite,
    nodes: usize,
    bytes: usize,
    origins: usize,
}

#[derive(Default)]
struct ParsedOrigins {
    first: Option<Location>,
    count: usize,
}

impl Preflight {
    fn check(
        &self,
        resource: MetadataResource,
        observed: u128,
        limit: usize,
    ) -> Result<(), MetadataError> {
        if observed > limit as u128 {
            Err(MetadataError::Limit {
                resource,
                observed,
                limit,
                location: self.location,
            })
        } else {
            Ok(())
        }
    }
    fn unsupported(&self, feature: MetadataFeature) -> MetadataError {
        MetadataError::Unsupported {
            feature,
            location: self.location,
        }
    }
    fn origins<'a>(
        &mut self,
        origins: impl Iterator<Item = &'a Origin>,
    ) -> Result<ParsedOrigins, MetadataError> {
        let mut parsed = ParsedOrigins::default();
        for origin in origins {
            self.check(
                MetadataResource::Origins,
                self.origins as u128 + 1,
                self.limits.max_origins,
            )?;
            self.origins += 1;
            if let Origin::Parsed(location) = origin {
                parsed.count += 1;
                if parsed.first.is_none() {
                    parsed.first = Some(*location);
                }
            }
        }
        Ok(parsed)
    }
    fn node(&mut self, depth: usize) -> Result<(), MetadataError> {
        self.check(
            MetadataResource::Depth,
            depth as u128,
            self.limits.max_depth.min(64),
        )?;
        self.check(
            MetadataResource::Nodes,
            self.nodes as u128 + 1,
            self.limits.max_nodes,
        )?;
        self.nodes += 1;
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<(), MetadataError> {
        self.check(
            MetadataResource::TextBytes,
            self.bytes as u128 + text.len() as u128,
            self.limits.max_text_bytes,
        )?;
        self.bytes += text.len();
        if text.contains('\0') {
            return Err(self.unsupported(MetadataFeature::NullText));
        }
        Ok(())
    }
    // Recursion is checked before descent and cannot exceed 64, even for a
    // directly constructed owned program with no preceding syntax admission.
    fn term(&mut self, term: &Term, depth: usize, constant: bool) -> Result<(), MetadataError> {
        self.node(depth)?;
        match term {
            Term::Symbolic(symbol) => self.symbol(symbol, depth)?,
            Term::Variable(variable) => {
                if constant {
                    return Err(self.unsupported(MetadataFeature::Term));
                }
                if let Variable::Named(name) = variable {
                    self.text(name.as_str())?;
                }
            }
            Term::Function { name, arguments } => {
                self.text(name.as_str())?;
                for term in arguments {
                    self.term(term, depth + 1, constant)?;
                }
            }
            Term::Tuple(arguments) => {
                for term in arguments {
                    self.term(term, depth + 1, constant)?;
                }
            }
            Term::UnaryOperation { argument, .. } | Term::Absolute(argument) => {
                self.term(argument, depth + 1, constant)?;
            }
            Term::BinaryOperation { left, right, .. } => {
                self.term(left, depth + 1, constant)?;
                self.term(right, depth + 1, constant)?;
            }
            // The observation compiler rejects these too. Refuse here before a
            // general constant resolver could interpret a malformed constructor.
            Term::Pool(_) | Term::Interval { .. } | Term::External { .. } => {
                return Err(self.unsupported(MetadataFeature::Term));
            }
        }
        Ok(())
    }
    fn symbol(&mut self, symbol: &Symbol, depth: usize) -> Result<(), MetadataError> {
        self.node(depth)?;
        match symbol {
            Symbol::String(text) => self.text(text)?,
            Symbol::Function {
                name, arguments, ..
            } => {
                self.text(name.as_str())?;
                for argument in arguments {
                    self.symbol(argument, depth + 1)?;
                }
            }
            Symbol::Tuple(arguments) => {
                for argument in arguments {
                    self.symbol(argument, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn show(&mut self, show: &Show) -> Result<(), MetadataError> {
        match show {
            Show::All => {}
            Show::Signature(signature) => self.text(signature.name.as_str())?,
            Show::Term(term) => self.term(term, 1, false)?,
            Show::TermBody { term, body } => {
                self.term(term, 1, false)?;
                for element in body.get().elements() {
                    self.node(1)?;
                    // Unsupported body forms are refused by the shared compiler
                    // without cloning them; only accepted literals need preflight.
                    if let BodyElement::Literal(literal) = element.get() {
                        match &literal.inner {
                            LiteralInner::Atom(atom) => {
                                self.text(atom.get().name.as_str())?;
                                if let Arguments::Single(arguments) = &atom.get().arguments {
                                    for term in arguments {
                                        self.term(term, 1, false)?;
                                    }
                                }
                            }
                            LiteralInner::Comparison(comparison) => {
                                self.term(comparison.get().first(), 1, false)?;
                                for (_, term) in comparison.get().steps() {
                                    self.term(term, 1, false)?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl SourceMetadata {
    /// Compile display metadata from a shared owned program without parsing,
    /// grounding, solving or external effects. Logical rules and other non-metadata
    /// statements are counted but not interpreted. Only the unparameterized base
    /// part is accepted. This does not admit the program for logical execution.
    ///
    /// Constants and observations use the same checked resolver/compiler as source
    /// admission, including positive-binding safety and constant-cycle checks.
    /// Constant uniqueness follows canonical declarations; extra provenance on
    /// one declaration does not create another. Original parsed directive origins
    /// are preserved; constructed directives affect the policy but invent no
    /// parsed evidence. `fallback` supplies optional real source evidence when
    /// no parsed origin exists; pass [`ProgramSite::program`] for
    /// source-independent diagnostics.
    ///
    /// # Cost
    /// Input preflight is bounded by statement/node/text/origin ceilings before
    /// copied metadata or constant resolution. Compilation has the separate
    /// expansion and observation limits. No caller-owned program is cloned.
    ///
    /// # Errors
    /// Returns an input, constant or safe-observation refusal with its logical
    /// statement identity and any real source evidence, without a partial policy.
    pub fn compile(
        program: &Program,
        limits: MetadataLimits,
        fallback: impl Into<ProgramSite>,
    ) -> Result<Self, MetadataError> {
        let fallback = fallback.into();
        validate(program, limits, fallback)?;
        let mut metadata = super::Builder::new(limits.storage);
        super::collect_profile_at(program, &mut metadata, true, fallback)
            .map_err(|error| MetadataError::Compilation(error.into()))?;
        let mut budget = crate::expansion::Budget::new(limits.expansion, 0);
        if !program.statements().any(|entry| {
            matches!(
                entry.get(),
                Statement::Show(Show::Term(_) | Show::TermBody { .. })
            )
        }) {
            crate::extended::resolve(program, &mut budget, fallback)
                .map_err(MetadataError::Expansion)?;
        }
        let observations = limits.observations;
        let options = CompilationOptions {
            max_body_elements: observations.max_body_elements as usize,
            core_limits: zetesis_core::AdmissionLimits {
                max_variables_per_template: observations.max_variables as usize,
                max_predicate_arity: observations.max_arity as usize,
                ..Default::default()
            },
        };
        metadata
            .compile_observations(program, options, observations, &mut budget, fallback)
            .map_err(MetadataError::Compilation)?;
        metadata
            .finish(fallback)
            .map_err(|error| MetadataError::Compilation(error.into()))
    }
}

fn validate(
    program: &Program,
    limits: MetadataLimits,
    fallback: ProgramSite,
) -> Result<(), MetadataError> {
    let mut input = Preflight {
        limits,
        location: fallback,
        nodes: 0,
        bytes: 0,
        origins: 0,
    };
    for part in program.parts() {
        if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
            return Err(input.unsupported(MetadataFeature::ProgramPart));
        }
    }
    let mut constants = BTreeMap::new();
    let mut metadata = 0_u128;
    for (index, carrier) in program.statements().enumerate() {
        input.location = fallback.with_statement(StatementId::new(index));
        input.check(
            MetadataResource::Statements,
            index as u128 + 1,
            limits.max_statements,
        )?;
        if !matches!(
            carrier.get(),
            Statement::Const(_)
                | Statement::Show(_)
                | Statement::Defined(_)
                | Statement::Project(_)
        ) {
            continue;
        }
        input.node(1)?;
        let parsed = input.origins(carrier.provenance().origins())?;
        input.location = parsed.first.map_or(input.location, |location| {
            input.location.with_location(location)
        });
        match carrier.get() {
            Statement::Const(constant) => {
                if constant.policy.is_some() {
                    return Err(MetadataError::Expansion(ExpansionFailure::ConstantPolicy {
                        location: input.location,
                    }));
                }
                input.text(constant.name.as_str())?;
                crate::expansion::check(
                    crate::ExpansionResource::Constants,
                    constants.len() as u128 + 1,
                    limits.expansion.max_constants,
                    input.location,
                )
                .map_err(MetadataError::Expansion)?;
                if let Some(first) = constants.insert(constant.name.as_str(), input.location) {
                    return Err(MetadataError::Expansion(
                        ExpansionFailure::DuplicateConstant {
                            name: constant.name.as_str().into(),
                            first,
                            duplicate: input.location,
                        },
                    ));
                }
                input.term(&constant.value, 1, true)?;
            }
            Statement::Show(show) => {
                metadata += parsed.count.max(1) as u128;
                input.show(show)?;
            }
            Statement::Project(project) => {
                metadata += parsed.count.max(1) as u128;
                match project {
                    Project::Signature(signature) => input.text(signature.name.as_str())?,
                    Project::Atom { atom, .. } => {
                        // This door records declarations; body safety and complete
                        // finite grounding belong to formula preparation.
                        input.text(atom.get().name.as_str())?;
                        for arguments in atom.get().alternatives() {
                            for term in arguments {
                                input.term(term, 1, false)?;
                            }
                        }
                    }
                }
            }
            Statement::Defined(defined) => {
                metadata += parsed.count.max(1) as u128;
                input.text(defined.signature.name.as_str())?;
            }
            _ => unreachable!("relevant statement checked"),
        }
        crate::expansion::check(
            crate::ExpansionResource::MetadataStatements,
            metadata,
            limits.expansion.max_metadata_statements,
            input.location,
        )
        .map_err(MetadataError::Expansion)?;
    }
    Ok(())
}
