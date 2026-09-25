//! Closed-value framing reads canonical preorder nodes without recursion.

use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::TermRef;

use super::{Encoding, Error};

impl Encoding {
    pub(super) fn value<'a>(&mut self, value: impl Into<TermRef<'a>>) -> Result<(), Error> {
        let value = value.into();
        match value.descriptor() {
            ValueNodeRef::Infimum => self.tag(0),
            ValueNodeRef::Number(number) => {
                self.tag(1)?;
                self.write(&number.to_be_bytes())
            }
            ValueNodeRef::String(text) => {
                self.tag(2)?;
                self.text(text)
            }
            ValueNodeRef::Symbol(text) => {
                self.tag(3)?;
                self.text(text)
            }
            ValueNodeRef::Function { .. } | ValueNodeRef::Tuple { .. } => {
                self.tag(4)?;
                self.count(value.expanded_nodes())?;
                for node in value.nodes() {
                    self.value_node(node)?;
                }
                Ok(())
            }
            ValueNodeRef::Supremum => self.tag(5),
        }
    }

    fn value_node(&mut self, node: ValueNodeRef<'_>) -> Result<(), Error> {
        match node {
            ValueNodeRef::Infimum => self.tag(0),
            ValueNodeRef::Number(number) => {
                self.tag(1)?;
                self.write(&number.to_be_bytes())
            }
            ValueNodeRef::String(text) => {
                self.tag(2)?;
                self.text(text)
            }
            ValueNodeRef::Symbol(text) => {
                self.tag(3)?;
                self.text(text)
            }
            ValueNodeRef::Function { name, sign, arity } => {
                self.tag(4)?;
                self.text(name)?;
                self.sign(sign)?;
                self.count(arity)
            }
            ValueNodeRef::Tuple { arity } => {
                self.tag(5)?;
                self.count(arity)
            }
            ValueNodeRef::Supremum => self.tag(6),
        }
    }
}
