//! Projection declarations reuse bounded source-rule compilation without becoming
//! logical producers. Their temporary dependency projections are discarded;
//! declarations contribute no statements to the retained source analysis.

use themelios_base::span::Location;
use themelios_program::program::{
    DefaultNegation, Literal, LiteralInner, Project, Rule, Statement,
};
use themelios_program::provenance::WithProvenance;

use crate::formula_ir::{Compiler, RuleIr};
use crate::{FormulaFailure, ProjectSelection};

impl Compiler<'_> {
    pub(super) fn project_statement(
        &mut self,
        statement: &WithProvenance<Statement>,
        origins: &[Location],
        projection_nodes: &mut u128,
        declarations: &mut Vec<RuleIr>,
        selection: &mut ProjectSelection,
    ) -> Result<bool, FormulaFailure> {
        let Statement::Project(project) = statement.get() else {
            return Ok(false);
        };
        match project {
            Project::Signature(signature) => {
                selection.signature(crate::metadata::predicate(signature, self.location)?);
            }
            Project::Atom { atom, body } => {
                selection.atom();
                let literal = Literal {
                    negation: DefaultNegation::None,
                    inner: LiteralInner::Atom(atom.clone()),
                };
                let rule = WithProvenance::new(
                    Statement::Rule(Rule::new(literal, body.get().clone())),
                    statement.provenance().clone(),
                );
                let mut discarded = Vec::new();
                let dependency_projection = self.dependency_projection;
                // Declaration constants belong to this scope, not the logical
                // rule domain. Aggregate IDs remain unique across both owners.
                let domain = std::mem::take(&mut self.domain);
                let result = self.source_rules(
                    &rule,
                    origins,
                    projection_nodes,
                    declarations,
                    &mut discarded,
                );
                self.domain = domain;
                self.dependency_projection = dependency_projection;
                result?;
            }
        }
        Ok(true)
    }
}
