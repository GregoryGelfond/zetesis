//! Borrowed semantic models and bounded JSON views; no solver or output stream.

use std::fmt;

use themelios_program::symbol::{Sign as SymbolSign, Symbol};
use zetesis_core::{Atom, Model, Sign, Value, ValueNode};
use zetesis_cpu::{Control, Stop};
use zetesis_objective::Score;

use super::{Error, Evaluation, Limits, ObservationProgram, Statistics};
use crate::OutputSelection;

/// A full supplied model beside its independent observation and objective channels.
/// Construction evaluates observations only; the caller establishes stability and
/// the score. No model clone, projection, deduplication or solver call occurs.
pub struct ModelView<'a> {
    model: &'a Model,
    selection: &'a OutputSelection,
    terms: Evaluation,
    score: Option<&'a Score>,
}
impl ObservationProgram {
    /// Evaluate a view over a supplied full model and optional already computed score.
    ///
    /// # Errors
    /// Returns the observation evaluator's typed refusal without a partial view.
    pub fn view<'a>(
        &self,
        model: &'a Model,
        selection: &'a OutputSelection,
        score: Option<&'a Score>,
        limits: Limits,
        control: &Control,
    ) -> Result<ModelView<'a>, Error> {
        let terms = if self.is_empty() {
            Evaluation {
                symbols: Vec::new(),
                statistics: Statistics::default(),
            }
        } else {
            self.evaluate(model, limits, control)?
        };
        Ok(ModelView {
            model,
            selection,
            terms,
            score,
        })
    }
}
impl ModelView<'_> {
    /// Complete semantic identity, unaffected by display directives.
    #[must_use]
    pub const fn model(&self) -> &Model {
        self.model
    }

    /// Selected original atoms in full-model order; no term-channel deduplication.
    pub fn shown_atoms(&self) -> impl Iterator<Item = &Atom> {
        self.model
            .atoms()
            .iter()
            .filter(|atom| self.selection.includes(atom))
    }

    /// Distinct enabled terms. Equal symbols in the atom channel remain separate.
    #[must_use]
    pub fn shown_terms(&self) -> &[Symbol] {
        self.terms.symbols()
    }

    /// The caller's already evaluated score; absence differs from an empty score.
    #[must_use]
    pub const fn score(&self) -> Option<&Score> {
        self.score
    }

    /// Observation work, independent of JSON view work.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.terms.statistics()
    }

    /// Derive one complete JSON model value, without writing external bytes.
    ///
    /// Terms use a flat preorder node sequence: constructor arities determine
    /// their children. Scalars have one node. Full atoms are typed name/sign/
    /// argument records; shown atom indices address that array and shown terms
    /// form their own channel. This preserves identity without parsing ASP text.
    /// Costs retain descending priority/value pairs, or `null` when absent.
    ///
    /// # Errors
    /// Refuses before exceeding the record, work or traversal-depth ceilings;
    /// allocation/control failures return no partial JSON value.
    pub fn json(&self, limits: ViewLimits, control: &Control) -> Result<String, ViewError> {
        let mut out = Buffer::new(limits, control);
        out.text("{\"full_model\":[")?;
        for (index, atom) in self.model.atoms().iter().enumerate() {
            if index != 0 {
                out.text(",")?;
            }
            out.text("{\"predicate\":")?;
            out.quoted(atom.predicate().name())?;
            out.text(",\"sign\":")?;
            out.quoted(sign(atom.predicate().sign()))?;
            out.text(",\"arguments\":[")?;
            for (index, value) in atom.values().iter().enumerate() {
                if index != 0 {
                    out.text(",")?;
                }
                out.value(value)?;
            }
            out.text("]}")?;
        }
        out.text("],\"shown\":{\"atom_indices\":[")?;
        let mut first = true;
        for (index, atom) in self.model.atoms().iter().enumerate() {
            let mut selected = !self.selection.is_explicit();
            for signature in self.selection.signatures() {
                out.step(
                    1 + atom.predicate().name().len() as u128 + signature.name().len() as u128,
                )?;
                if atom.predicate() == signature {
                    selected = true;
                    break;
                }
            }
            if selected {
                if !first {
                    out.text(",")?;
                }
                first = false;
                out.text(&index.to_string())?;
            }
        }
        out.text("],\"terms\":[")?;
        for (index, symbol) in self.shown_terms().iter().enumerate() {
            if index != 0 {
                out.text(",")?;
            }
            out.symbol(symbol)?;
        }
        out.text("]},\"costs\":")?;
        if let Some(score) = self.score {
            out.text("[")?;
            for (index, (priority, value)) in score.costs().iter().enumerate() {
                if index != 0 {
                    out.text(",")?;
                }
                out.text("{\"priority\":")?;
                out.text(&priority.to_string())?;
                out.text(",\"value\":")?;
                out.text(&value.to_string())?;
                out.text("}")?;
            }
            out.text("]")?;
        } else {
            out.text("null")?;
        }
        out.text("}")?;
        Ok(out.text)
    }
}

/// Independent bounds for one pure model view, excluding allocator overhead.
#[derive(Clone, Copy, Debug)]
pub struct ViewLimits {
    /// Maximum retained UTF-8 JSON bytes; zero means zero.
    pub max_bytes: usize,
    /// Maximum charged output bytes, node visits and selection comparisons.
    pub max_work: u64,
    /// Maximum symbol traversal depth; bounds the explicit cursor stack.
    pub max_depth: usize,
}
impl Default for ViewLimits {
    fn default() -> Self {
        Self {
            max_bytes: 8_388_608,
            max_work: 100_000_000,
            max_depth: 128,
        }
    }
}

/// A pure view failed without returning partial JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// Complete view would exceed the configured retained-byte ceiling.
    Bytes,
    /// Charged traversal/encoding work would exceed its ceiling.
    Work,
    /// Explicit traversal stack would exceed the depth ceiling.
    Depth,
    /// Fallible storage reservation failed.
    Allocation,
    /// Caller cancellation or deadline stopped the view.
    Stopped(Stop),
}
impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "model JSON view refused: {self:?}")
    }
}
impl std::error::Error for ViewError {}

fn sign(value: Sign) -> &'static str {
    match value {
        Sign::Positive => "positive",
        Sign::Negative => "negative",
    }
}

struct Buffer<'a> {
    text: String,
    limits: ViewLimits,
    control: &'a Control,
    work: u64,
}
impl<'a> Buffer<'a> {
    fn new(limits: ViewLimits, control: &'a Control) -> Self {
        Self {
            text: String::new(),
            limits,
            control,
            work: 0,
        }
    }
    fn step(&mut self, count: u128) -> Result<(), ViewError> {
        self.control.poll().map_err(ViewError::Stopped)?;
        let work = u128::from(self.work) + count;
        if work > u128::from(self.limits.max_work) {
            return Err(ViewError::Work);
        }
        self.work = u64::try_from(work).expect("checked u64 work ceiling");
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<(), ViewError> {
        self.step(text.len() as u128 + 1)?;
        let needed = self
            .text
            .len()
            .checked_add(text.len())
            .ok_or(ViewError::Bytes)?;
        if needed > self.limits.max_bytes {
            return Err(ViewError::Bytes);
        }
        if needed > self.text.capacity() {
            let capacity = needed
                .max(self.text.capacity().saturating_mul(2))
                .min(self.limits.max_bytes);
            self.text
                .try_reserve_exact(capacity - self.text.len())
                .map_err(|_| ViewError::Allocation)?;
        }
        self.text.push_str(text);
        Ok(())
    }
    fn quoted(&mut self, text: &str) -> Result<(), ViewError> {
        self.text("\"")?;
        for character in text.chars() {
            match character {
                '"' => self.text("\\\"")?,
                '\\' => self.text("\\\\")?,
                '\u{0000}'..='\u{001f}' => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    let byte = u8::try_from(u32::from(character)).expect("ASCII control character");
                    let escape = [
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[usize::from(byte >> 4)],
                        HEX[usize::from(byte & 15)],
                    ];
                    self.text(std::str::from_utf8(&escape).expect("ASCII hexadecimal escape"))?;
                }
                other => self.text(other.encode_utf8(&mut [0; 4]))?,
            }
        }
        self.text("\"")
    }
    fn leaf(
        &mut self,
        kind: &str,
        value: Option<&str>,
        number: Option<i32>,
    ) -> Result<(), ViewError> {
        self.text("{\"kind\":")?;
        self.quoted(kind)?;
        if let Some(value) = value {
            self.text(",\"value\":")?;
            self.quoted(value)?;
        }
        if let Some(value) = number {
            self.text(",\"value\":")?;
            self.text(&value.to_string())?;
        }
        self.text("}")
    }
    fn constructor(
        &mut self,
        name: Option<&str>,
        negative: bool,
        arity: usize,
    ) -> Result<(), ViewError> {
        if let Some(name) = name {
            if arity == 0 && !negative {
                return self.leaf("symbol", Some(name), None);
            }
            self.text("{\"kind\":\"function\",\"name\":")?;
            self.quoted(name)?;
            self.text(",\"sign\":")?;
            self.quoted(if negative { "negative" } else { "positive" })?;
        } else {
            self.text("{\"kind\":\"tuple\"")?;
        }
        self.text(",\"arity\":")?;
        self.text(&arity.to_string())?;
        self.text("}")
    }
    fn node(&mut self, node: &ValueNode) -> Result<(), ViewError> {
        match node {
            ValueNode::Infimum => self.leaf("infimum", None, None),
            ValueNode::Supremum => self.leaf("supremum", None, None),
            ValueNode::Number(value) => self.leaf("number", None, Some(*value)),
            ValueNode::String(value) => self.leaf("string", Some(value), None),
            ValueNode::Symbol(value) => self.leaf("symbol", Some(value), None),
            ValueNode::Function { name, sign, arity } => {
                self.constructor(Some(name), *sign == Sign::Negative, *arity)
            }
            ValueNode::Tuple { arity } => self.constructor(None, false, *arity),
        }
    }
    fn value(&mut self, value: &Value) -> Result<(), ViewError> {
        if self.limits.max_depth == 0 {
            return Err(ViewError::Depth);
        }
        self.text("[")?;
        match value {
            Value::Infimum => self.leaf("infimum", None, None)?,
            Value::Supremum => self.leaf("supremum", None, None)?,
            Value::Number(value) => self.leaf("number", None, Some(*value))?,
            Value::String(value) => self.leaf("string", Some(value), None)?,
            Value::Symbol(value) => self.leaf("symbol", Some(value), None)?,
            Value::Structured(value) => {
                if value.depth() > self.limits.max_depth {
                    return Err(ViewError::Depth);
                }
                for (index, node) in value.nodes().iter().enumerate() {
                    if index != 0 {
                        self.text(",")?;
                    }
                    self.node(node)?;
                }
            }
        }
        self.text("]")
    }
    fn symbol(&mut self, symbol: &Symbol) -> Result<(), ViewError> {
        let mut frames: Vec<(&[Symbol], usize)> = Vec::new();
        let mut current = symbol;
        self.text("[")?;
        loop {
            if frames.len() >= self.limits.max_depth {
                return Err(ViewError::Depth);
            }
            let children = match current {
                Symbol::Infimum => {
                    self.leaf("infimum", None, None)?;
                    None
                }
                Symbol::Supremum => {
                    self.leaf("supremum", None, None)?;
                    None
                }
                Symbol::Number(value) => {
                    self.leaf("number", None, Some(*value))?;
                    None
                }
                Symbol::String(value) => {
                    self.leaf("string", Some(value), None)?;
                    None
                }
                Symbol::Function {
                    name,
                    sign,
                    arguments,
                } => {
                    self.constructor(
                        Some(name.as_str()),
                        *sign == SymbolSign::Negative,
                        arguments.len(),
                    )?;
                    Some(arguments.as_slice())
                }
                Symbol::Tuple(arguments) => {
                    self.constructor(None, false, arguments.len())?;
                    Some(arguments.as_slice())
                }
            };
            if let Some(arguments) = children.filter(|args| !args.is_empty()) {
                frames
                    .try_reserve_exact(1)
                    .map_err(|_| ViewError::Allocation)?;
                frames.push((arguments, 1));
                current = &arguments[0];
                self.text(",")?;
                continue;
            }
            loop {
                let Some((arguments, next)) = frames.last_mut() else {
                    return self.text("]");
                };
                if let Some(argument) = arguments.get(*next) {
                    *next += 1;
                    current = argument;
                    self.text(",")?;
                    break;
                }
                frames.pop();
            }
        }
    }
}
