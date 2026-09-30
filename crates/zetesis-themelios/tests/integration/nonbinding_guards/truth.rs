//! Direct formulas for a one-atom count, independent of aggregate lowering.

#[derive(Clone, Copy)]
pub(super) struct Truth {
    pub whole: bool,
    pub frozen: bool,
}

impl Truth {
    pub fn constant(value: bool) -> Self {
        Self {
            whole: value,
            frozen: value,
        }
    }

    pub fn atom(outer: bool, inner: bool) -> Self {
        Self {
            whole: outer,
            frozen: outer && inner,
        }
    }

    pub fn negate(self) -> Self {
        self.implies(Self::constant(false))
    }

    pub fn and(self, other: Self) -> Self {
        Self {
            whole: self.whole && other.whole,
            frozen: self.frozen && other.frozen,
        }
    }

    pub fn or(self, other: Self) -> Self {
        Self {
            whole: self.whole || other.whole,
            frozen: self.frozen || other.frozen,
        }
    }

    pub fn implies(self, other: Self) -> Self {
        let whole = !self.whole || other.whole;
        Self {
            whole,
            frozen: whole && (!self.frozen || other.frozen),
        }
    }
}

fn compare(left: i32, relation: &str, right: i32) -> bool {
    match relation {
        "=" => left == right,
        "!=" => left != right,
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        ">=" => left >= right,
        _ => panic!("the fixture supplies one of the six relations"),
    }
}

/// For the singleton count, the four numeric truth tables denote exactly
/// false, true, p, or not p. Default negation is applied afterwards.
pub(super) fn singleton(p: Truth, bound: i32, relation: &str, negations: usize) -> Truth {
    let mut result = match (compare(bound, relation, 0), compare(bound, relation, 1)) {
        (false, false) => Truth::constant(false),
        (true, true) => Truth::constant(true),
        (false, true) => p,
        (true, false) => p.negate(),
    };
    for _ in 0..negations {
        result = result.negate();
    }
    result
}
