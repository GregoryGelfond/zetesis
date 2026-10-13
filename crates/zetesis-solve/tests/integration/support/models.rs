//! Families of models, as the session propositions compare them.

use std::collections::{BTreeMap, BTreeSet};

use zetesis_core::Model;
use zetesis_objective::Score;
use zetesis_solve::WorldView;

/// A family of models, compared as a set.
pub(crate) type Family = BTreeSet<Model>;

/// A family of models with their objectives' costs, compared as a set.
pub(crate) type CostFamily = BTreeSet<(Model, Option<Vec<(i32, i64)>>)>;

/// A family of models with their scores, by model.
pub(crate) type ScoredFamily = BTreeMap<Model, Option<Score>>;

/// The answer sets of `view` with their costs; each answer set belongs to the
/// view's subject.
pub(crate) fn cost_family(view: &WorldView) -> CostFamily {
    view.answer_sets()
        .iter()
        .map(|answer| {
            assert!(answer.subject().same_instance(view.subject()));
            (
                answer.interpretation().clone(),
                answer.score().map(|score| score.costs().to_vec()),
            )
        })
        .collect()
}
