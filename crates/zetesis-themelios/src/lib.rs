//! Faithful source boundaries for zetesis, built on pinned themelios parsing.
//! Strict S0 and extended scalar admission compile relational templates without
//! grounding. The separate formula APIs compute a bounded possible-positive
//! relation and ground complete joins into normal, conditional-choice, and finite-aggregate Ferraris
//! formulas. Lifted minimization templates remain separate and score only supplied
//! verified stable models. No source route invokes a solver or external engine.
//!
//! Original source bytes and parsed origins survive every successful boundary;
//! bundle failures retain the complete source catalog. Independent source,
//! syntax, scalar expansion, finite substitution, and formula storage limits
//! refuse rather than return recovered or truncated programs. Finite count/sum
//! assignment cursors may generate values absent from the source while retaining
//! exact reduct equality formulas. Scalar equality and interval bindings follow
//! dependency-ordered plans; evaluated normal heads use fresh value slots and
//! bounded streamed cursors. Pinned upstream analysis reads a separately bounded
//! pool-free owned projection; its facts are retained without claiming
//! its narrower safety reading proves the clingo assignment extension.
//! Syntax walks and
//! term normalization are iterative. The formula route preserves recursive
//! eligibility and uses constraints for choice bounds; its optional necessary
//! support guards remain double-negated so they do not constrain reduct subsets.
//! Source acceptance is not a parser-to-Lean or Rust implementation proof.
#![forbid(unsafe_code)]

mod diagnostic;
mod profile;
mod compile;
mod coherence;
mod bundle;
mod bundle_admission;
mod expansion;
mod extended;
mod fact_expansion;
mod metadata;
mod formula;
mod formula_ir;
mod formula_aggregate_ir;
mod formula_analysis;
mod formula_pool;
mod formula_assignment_ir;
mod formula_assignment;
mod formula_objective_dependencies;
mod formula_ground;
mod formula_factor;
mod formula_guard;
mod formula_conditional;
mod formula_conditional_ir;
mod formula_conditional_head_ir;
mod formula_support;
mod formula_binding_plan;
mod formula_binding_guard;
mod formula_binding_ir;
mod formula_choice_ir;
mod formula_head_ir;
mod formula_binding_cursor;
mod formula_projection_ir;
mod formula_weak;
pub mod objective_bound;
pub mod observation;

use themelios_base::source::{Source, SourceId};
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::raise::raise;
use themelios_syntax::dialect::Dialect;
use themelios_syntax::parse::parse;
use zetesis_core::{AdmissionLimits, Program};

pub use bundle::{
    BundleError, BundleIoOperation, BundleLimits, BundleResource, BundleRoot, BundleSource,
    IncludeEdge, IncludeResolution, SourceBundle,
};
pub use bundle_admission::{
    AdmittedBundle, BundleAdmissionError, BundleAdmissionFailure, BundleAdmissionOptions,
    admit_bundle_extended,
};
pub use diagnostic::{AdmissionFailure, InputLimit, ProfileFeature};
pub use expansion::{ExpansionFailure, ExpansionLimits, ExpansionResource};
pub use extended::admit_extended;
pub use formula::{
    AdmittedFormula, AdmittedFormulaBundle, FormulaBundleFailure, FormulaFailure, FormulaLimits,
    FormulaResource, admit_bundle_formula, admit_formula,
};
pub use metadata::{LocatedDirective, OutputSelection, SourceDirective, SourceMetadata};

/// Explicit host admission ceilings. Zero means that no resource of that kind
/// may be consumed; limits never mean unlimited.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionOptions {
    /// Identity attached to every source location.
    pub source_id: SourceId,
    /// Maximum UTF-8 bytes accepted before parsing.
    pub max_source_bytes: usize,
    /// Maximum syntax nodes visited before raising.
    pub max_syntax_nodes: usize,
    /// Maximum syntax-node nesting, including every nested term.
    pub max_syntax_depth: usize,
    /// Maximum source body elements per rule, before set deduplication.
    pub max_body_elements: usize,
    /// Limits checked independently by the core template admission door.
    pub core_limits: AdmissionLimits,
}

impl Default for AdmissionOptions {
    fn default() -> Self {
        Self {
            source_id: SourceId::new(0),
            max_source_bytes: 1_048_576,
            max_syntax_nodes: 262_144,
            max_syntax_depth: 128,
            max_body_elements: 1_024,
            core_limits: AdmissionLimits::default(),
        }
    }
}

/// An admitted program and the source evidence retained by its boundary.
/// Template origins follow the core program's unchanged template order.
#[derive(Debug)]
pub struct Admitted {
    program: Program,
    source: Source,
    template_origins: Vec<Vec<Location>>,
    metadata: SourceMetadata,
}

impl Admitted {
    /// The checked finite relational program.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Original bytes under the source identity used by every origin.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Parsed origins per template. Equal source rules may have merged several
    /// origins during themelios canonicalization.
    #[must_use]
    pub fn template_origins(&self) -> &[Vec<Location>] {
        &self.template_origins
    }

    /// Original source declarations and display selection. Display metadata
    /// never changes the independently admitted program or model identity.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.metadata
    }

    /// Consume the boundary value when source evidence is no longer required.
    #[must_use]
    pub fn into_program(self) -> Program {
        self.program
    }
}

/// Admit UTF-8 source under S0 using the clingo dialect. Refuses resource-limit
/// violations, every parser/raiser diagnostic, unsupported source forms, and
/// core validation failures. No recovered or truncated program is returned.
/// Work before core admission is bounded by source bytes and syntax ceilings.
///
/// # Errors
/// Returns the typed source, syntax, profile, construction, or core admission
/// refusal at the first failing stage, preserving that stage's diagnostics.
pub fn admit(text: String, options: AdmissionOptions) -> Result<Admitted, AdmissionFailure> {
    let start = Location {
        source: options.source_id,
        span: Span::empty(ByteOffset::new(0)),
    };
    if text.len() > options.max_source_bytes {
        return Err(AdmissionFailure::Limit {
            resource: InputLimit::SourceBytes,
            limit: options.max_source_bytes,
            observed: text.len(),
            location: start,
        });
    }
    let source =
        Source::new(options.source_id, text).map_err(|error| AdmissionFailure::Source {
            error,
            location: start,
        })?;
    let parsed = parse(&source, Dialect::Clingo);
    if !parsed.diagnostics().is_empty() {
        return Err(AdmissionFailure::Syntax(parsed.diagnostics().to_vec()));
    }
    profile::check(&parsed, options)?;
    let raised = raise(&parsed);
    if !raised.diagnostics().is_empty() {
        return Err(AdmissionFailure::Raise(raised.diagnostics().to_vec()));
    }
    let source_location = Location {
        source: source.id(),
        span: source.span(),
    };
    let (mut templates, mut template_origins) =
        compile::program(raised.program(), source_location)?;
    coherence::append(
        &mut templates,
        &mut template_origins,
        options.core_limits,
        source_location,
        |_, _, _| Ok::<(), AdmissionFailure>(()),
    )?;
    let program = Program::new(templates, options.core_limits).map_err(|error| {
        let location = error
            .template_index()
            .and_then(|index| template_origins.get(index))
            .and_then(|origins| origins.first())
            .copied()
            .unwrap_or(source_location);
        AdmissionFailure::Core { error, location }
    })?;
    Ok(Admitted {
        program,
        source,
        template_origins,
        metadata: SourceMetadata::default(),
    })
}

mod structural_value;
