//! A hybrid grounding line names both halves of its constraint division.

use crate::presentation::Diagnostics;
use crate::{ColorMode, ExecutionObservation, ExecutionObserver, Grounder};

const DIVISION: zetesis_solve::StreamedConstraints = zetesis_solve::StreamedConstraints {
    templates: 1,
    instances: 2,
    retained_templates: 3,
    retained_instances: 4,
};
const CLAUSE: &str = "streamed constraints: 1 templates, 2 admitted instances; grounded in the core: 3 templates, 4 instances";

fn rendered(observation: ExecutionObservation<'_>) -> String {
    let mut output = Vec::new();
    Diagnostics::new(&mut output, ColorMode::Never)
        .observe(observation)
        .unwrap();
    String::from_utf8(output).unwrap()
}

#[test]
fn hybrid_grounding_names_streamed_and_grounded_constraints() {
    let text = rendered(ExecutionObservation::HybridGrounding {
        requested: Grounder::Lazy,
        streamed: DIVISION,
    });
    assert!(text.contains(CLAUSE), "{text}");
}

#[test]
fn a_hybrid_base_names_streamed_and_grounded_constraints() {
    let text = rendered(ExecutionObservation::TerminalDefinitions {
        requested: Grounder::Lazy,
        base: zetesis_themelios::BaseKind::Hybrid,
        deferred_templates: 1,
        streamed: Some(DIVISION),
    });
    assert!(text.contains(CLAUSE), "{text}");
}
