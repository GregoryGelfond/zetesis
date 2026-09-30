//! The observations a formula program shows, rendered over every one of its
//! atoms.

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_reference_support::formula;
use zetesis_themelios::observation::Limits;

/// The words of the observations `source` shows in the model holding every
/// one of its atoms.
pub fn shown(source: &str) -> Vec<String> {
    let input = formula(source);
    let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
    input
        .metadata()
        .observations()
        .render(
            &model,
            input.metadata().output(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .text()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}
