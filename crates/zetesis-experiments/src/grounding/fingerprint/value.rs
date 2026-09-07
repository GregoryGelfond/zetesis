//! Closed-value framing reads canonical preorder nodes without recursion.

use zetesis_core::{Value, ValueNode};

use super::{Encoding, Error};

impl Encoding {
    pub(super) fn value(&mut self, value: &Value) -> Result<(), Error> {
        match value {
            Value::Infimum => self.tag(0),
            Value::Number(number) => {
                self.tag(1)?;
                self.write(&number.to_be_bytes())
            }
            Value::String(text) => {
                self.tag(2)?;
                self.text(text)
            }
            Value::Symbol(text) => {
                self.tag(3)?;
                self.text(text)
            }
            Value::Structured(value) => {
                self.tag(4)?;
                self.count(value.nodes().len())?;
                for node in value.nodes() {
                    self.value_node(node)?;
                }
                Ok(())
            }
            Value::Supremum => self.tag(5),
        }
    }

    fn value_node(&mut self, node: &ValueNode) -> Result<(), Error> {
        match node {
            ValueNode::Infimum => self.tag(0),
            ValueNode::Number(number) => {
                self.tag(1)?;
                self.write(&number.to_be_bytes())
            }
            ValueNode::String(text) => {
                self.tag(2)?;
                self.text(text)
            }
            ValueNode::Symbol(text) => {
                self.tag(3)?;
                self.text(text)
            }
            ValueNode::Function { name, sign, arity } => {
                self.tag(4)?;
                self.text(name)?;
                self.sign(*sign)?;
                self.count(*arity)
            }
            ValueNode::Tuple { arity } => {
                self.tag(5)?;
                self.count(*arity)
            }
            ValueNode::Supremum => self.tag(6),
        }
    }
}
