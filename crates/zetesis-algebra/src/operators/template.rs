//! Executable templates for Bind, Filter, and Project.
//!
//! Fields stay private. Other crates construct a template with the builder
//! methods and choose a [`Schedule`]; they do not name the generators.

/// How arithmetic tests are scheduled relative to Bind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Schedule {
    /// Enumerate the domain product and apply arithmetic only as Filter.
    #[default]
    FilterOnly,
    /// Push invertible arithmetic tests into Bind when they can generate a
    /// missing slot. Residual tests remain Filter. This is `PushdownBind`
    /// relative to the full product, not a different semantics.
    Generating,
}

/// A finite carrier for [`super::bind`], plus residual Filter and a head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    width: usize,
    schedule: Schedule,
    ranges: Vec<RangeBind>,
    dependent: Vec<DependentRange>,
    facts: Vec<FactBind>,
    arithmetic: Vec<Arithmetic>,
    comparisons: Vec<Comparison>,
    head: Vec<HeadTerm>,
}

/// A slot that is not a valid attribute of this template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateError {
    /// The mentioned slot is outside `0..width`.
    Slot,
    /// A head was required and none was supplied.
    MissingHead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RangeBind {
    pub slot: usize,
    pub lo: i32,
    pub hi: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DependentRange {
    pub slot: usize,
    pub lo: i32,
    pub hi_slot: usize,
    pub hi_delta: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FactBind {
    pub slot: usize,
    pub values: Vec<i32>,
}

/// An invertible (or residual) arithmetic test on already integer slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Arithmetic {
    /// `dividend \ divisor = 0` (remainder equals zero).
    RemainderZero { dividend: usize, divisor: usize },
    /// `dividend / divisor = quot`.
    Quotient {
        dividend: usize,
        divisor: usize,
        quot: usize,
    },
    /// `left + right = result`.
    Plus {
        left: usize,
        right: usize,
        result: usize,
    },
    /// `left * right = result`.
    Times {
        left: usize,
        right: usize,
        result: usize,
    },
    /// `|input| = result`.
    Abs { input: usize, result: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Comparison {
    Rel {
        left: Term,
        relation: Rel,
        right: Term,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Term {
    Slot(usize),
    Const(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rel {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HeadTerm {
    Slot(usize),
    Const(i32),
}

impl Template {
    /// An empty-body template of `width` slots, generating schedule off,
    /// with no ranges, tests, or head.
    #[must_use]
    pub fn new(width: usize) -> Self {
        Self {
            width,
            schedule: Schedule::FilterOnly,
            ranges: Vec::new(),
            dependent: Vec::new(),
            facts: Vec::new(),
            arithmetic: Vec::new(),
            comparisons: Vec::new(),
            head: Vec::new(),
        }
    }

    /// Number of binding slots.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// The current arithmetic schedule.
    #[must_use]
    pub fn schedule(&self) -> Schedule {
        self.schedule
    }

    /// Choose filter-only or generating Bind for arithmetic tests.
    #[must_use]
    pub fn with_schedule(mut self, schedule: Schedule) -> Self {
        self.schedule = schedule;
        self
    }

    /// Bind `slot` to every value in the inclusive range `lo..=hi`.
    ///
    /// An empty range (`lo > hi`) is legal and produces no bindings.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if `slot` is out of range.
    pub fn with_range(mut self, slot: usize, lo: i32, hi: i32) -> Result<Self, TemplateError> {
        self.check_slot(slot)?;
        self.ranges.push(RangeBind { slot, lo, hi });
        Ok(self)
    }

    /// Bind `slot` to `lo..=(parent + hi_delta)` once `parent` is bound.
    ///
    /// `Y = 2..X-1` is `with_dependent_range(y, 2, x, -1)`.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if either slot is out of range.
    pub fn with_dependent_range(
        mut self,
        slot: usize,
        lo: i32,
        parent: usize,
        hi_delta: i32,
    ) -> Result<Self, TemplateError> {
        self.check_slot(slot)?;
        self.check_slot(parent)?;
        self.dependent.push(DependentRange {
            slot,
            lo,
            hi_slot: parent,
            hi_delta,
        });
        Ok(self)
    }

    /// Bind `slot` to each value in `values`. Duplicates are kept as given
    /// and filtered by later uniquing only if the caller projects them.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if `slot` is out of range.
    pub fn with_facts(mut self, slot: usize, values: Vec<i32>) -> Result<Self, TemplateError> {
        self.check_slot(slot)?;
        self.facts.push(FactBind { slot, values });
        Ok(self)
    }

    /// A remainder test `dividend \ divisor = 0`.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if either slot is out of range.
    pub fn with_remainder(
        mut self,
        dividend: usize,
        divisor: usize,
    ) -> Result<Self, TemplateError> {
        self.check_slot(dividend)?;
        self.check_slot(divisor)?;
        self.arithmetic
            .push(Arithmetic::RemainderZero { dividend, divisor });
        Ok(self)
    }

    /// An integer quotient test `dividend / divisor = quot`.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_quotient(
        mut self,
        dividend: usize,
        divisor: usize,
        quot: usize,
    ) -> Result<Self, TemplateError> {
        self.check_slot(dividend)?;
        self.check_slot(divisor)?;
        self.check_slot(quot)?;
        self.arithmetic.push(Arithmetic::Quotient {
            dividend,
            divisor,
            quot,
        });
        Ok(self)
    }

    /// An addition test `left + right = result`.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_plus(
        mut self,
        left: usize,
        right: usize,
        result: usize,
    ) -> Result<Self, TemplateError> {
        self.check_slot(left)?;
        self.check_slot(right)?;
        self.check_slot(result)?;
        self.arithmetic.push(Arithmetic::Plus {
            left,
            right,
            result,
        });
        Ok(self)
    }

    /// A product test `left * right = result`.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_times(
        mut self,
        left: usize,
        right: usize,
        result: usize,
    ) -> Result<Self, TemplateError> {
        self.check_slot(left)?;
        self.check_slot(right)?;
        self.check_slot(result)?;
        self.arithmetic.push(Arithmetic::Times {
            left,
            right,
            result,
        });
        Ok(self)
    }

    /// An absolute-value test `|input| = result`.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_abs(mut self, input: usize, result: usize) -> Result<Self, TemplateError> {
        self.check_slot(input)?;
        self.check_slot(result)?;
        self.arithmetic.push(Arithmetic::Abs { input, result });
        Ok(self)
    }

    /// Residual `left == right` on slots.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_eq(self, left: usize, right: usize) -> Result<Self, TemplateError> {
        self.with_rel(Term::Slot(left), Rel::Eq, Term::Slot(right))
    }

    /// Residual `left != right` on slots.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_ne(self, left: usize, right: usize) -> Result<Self, TemplateError> {
        self.with_rel(Term::Slot(left), Rel::Ne, Term::Slot(right))
    }

    /// Residual `slot == value`.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if `slot` is out of range.
    pub fn with_eq_const(self, slot: usize, value: i32) -> Result<Self, TemplateError> {
        self.with_rel(Term::Slot(slot), Rel::Eq, Term::Const(value))
    }

    /// Residual `left < right` on slots.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_lt(self, left: usize, right: usize) -> Result<Self, TemplateError> {
        self.with_rel(Term::Slot(left), Rel::Lt, Term::Slot(right))
    }

    /// Residual `left <= right` on slots.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_le(self, left: usize, right: usize) -> Result<Self, TemplateError> {
        self.with_rel(Term::Slot(left), Rel::Le, Term::Slot(right))
    }

    /// Residual `left > right` on slots.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_gt(self, left: usize, right: usize) -> Result<Self, TemplateError> {
        self.with_rel(Term::Slot(left), Rel::Gt, Term::Slot(right))
    }

    /// Residual `left >= right` on slots.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_ge(self, left: usize, right: usize) -> Result<Self, TemplateError> {
        self.with_rel(Term::Slot(left), Rel::Ge, Term::Slot(right))
    }

    /// Project the listed slots, in order, as the atom key.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if a slot is out of range.
    pub fn with_head_slots(mut self, slots: &[usize]) -> Result<Self, TemplateError> {
        for &slot in slots {
            self.check_slot(slot)?;
            self.head.push(HeadTerm::Slot(slot));
        }
        Ok(self)
    }

    /// Project a single slot as a unary atom key.
    ///
    /// # Errors
    /// Returns [`TemplateError::Slot`] if `slot` is out of range.
    pub fn with_head_slot(self, slot: usize) -> Result<Self, TemplateError> {
        self.with_head_slots(&[slot])
    }

    /// Append a constant coordinate to the projected atom key.
    #[must_use]
    pub fn with_head_const(mut self, value: i32) -> Self {
        self.head.push(HeadTerm::Const(value));
        self
    }

    fn with_rel(mut self, left: Term, relation: Rel, right: Term) -> Result<Self, TemplateError> {
        self.check_term(left)?;
        self.check_term(right)?;
        self.comparisons.push(Comparison::Rel {
            left,
            relation,
            right,
        });
        Ok(self)
    }

    fn check_term(&self, term: Term) -> Result<(), TemplateError> {
        match term {
            Term::Slot(slot) => self.check_slot(slot),
            Term::Const(_) => Ok(()),
        }
    }

    fn check_slot(&self, slot: usize) -> Result<(), TemplateError> {
        if slot < self.width {
            Ok(())
        } else {
            Err(TemplateError::Slot)
        }
    }

    pub(super) fn ranges(&self) -> &[RangeBind] {
        &self.ranges
    }

    pub(super) fn dependent(&self) -> &[DependentRange] {
        &self.dependent
    }

    pub(super) fn facts(&self) -> &[FactBind] {
        &self.facts
    }

    pub(super) fn arithmetic(&self) -> &[Arithmetic] {
        &self.arithmetic
    }

    pub(super) fn comparisons(&self) -> &[Comparison] {
        &self.comparisons
    }

    pub(super) fn head(&self) -> &[HeadTerm] {
        &self.head
    }
}
