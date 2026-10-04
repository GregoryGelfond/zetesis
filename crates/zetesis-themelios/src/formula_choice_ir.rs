//! Choice keys distinguish counted entries after local pool expansion.

use themelios_program::program::{Choice, Identity};

use crate::FormulaFailure;
use crate::formula_ir::{Compiler, Element, HeadElementKey, LocalFamily, Variables};

impl Compiler<'_> {
    pub(super) fn choice_elements(
        &mut self,
        choice: &Choice,
        source: Option<&Choice>,
        variables: &Variables,
    ) -> Result<Vec<Element>, FormulaFailure> {
        let mut elements = Vec::new();
        // The whole-rule pool rewrite reconstructs local head alternatives.
        // Compile each original counted entry's local product independently.
        // Every product alternative receives its occurrence key before local
        // variable substitutions; equal alternatives remain distinct entries.
        for (index, element) in source.unwrap_or(choice).elements().enumerate() {
            let family = LocalFamily(index);
            let identity = element.get().identity();
            if identity == Identity::ByOccurrence {
                self.dependency_projection = true;
            }
            for literal in self.literal_alternatives(element.get().literal())? {
                for alternative in self.condition_alternatives(element.get().condition())? {
                    let mut local = variables.clone();
                    self.head_global_literal(&literal, &mut local)?;
                    let mut condition = self.condition(&alternative, &mut local)?;
                    let (head, body_variables) =
                        self.element_head(&literal, &mut local, &mut condition)?;
                    let key = match identity {
                        Identity::ByContent => HeadElementKey::Atom,
                        Identity::ByOccurrence => HeadElementKey::Occurrence(elements.len()),
                    };
                    self.variable_limit(&local)?;
                    local.safety(self.location)?;
                    elements.push(Element {
                        family,
                        key,
                        head,
                        condition,
                        body_variables,
                        variables: local.count,
                    });
                }
            }
        }
        Ok(elements)
    }
}

#[cfg(test)]
mod tests;
