//! Semantic equality of compiled topology across independent vocabularies.
//! Occurrence coordinates are never compared as payload identities. This walks
//! the admitted topology and delegates typed-term comparison to borrowed core
//! views, without exporting or rebuilding any value.
use super::{
    AggregateKey, AggregateQuery, AtomKeyTemplate, Binder, Condition, Constructor, KeyTemplate,
    ObservationProgram, Operand, Pattern, Query, Read, Scalar, Template,
};
use std::sync::Arc;

struct Identity<'a, 'b> {
    left: Read<'a>,
    right: Read<'b>,
}
fn items<T>(left: &[T], right: &[T], mut equal: impl FnMut(&T, &T) -> bool) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| equal(a, b))
}
impl Identity<'_, '_> {
    fn scalar(&self, a: Scalar, b: Scalar) -> bool {
        self.left.scalar(a) == self.right.scalar(b)
    }
    fn shape(&self, a: Constructor, b: Constructor) -> bool {
        self.left.constructor(a) == self.right.constructor(b)
    }
    fn template(&self, a: &Template, b: &Template) -> bool {
        match (a, b) {
            (Template::Constant(a), Template::Constant(b)) => self.scalar(*a, *b),
            (Template::Variable(a), Template::Variable(b)) => a == b,
            (Template::Construct(a, ac), Template::Construct(b, bc)) => {
                self.shape(*a, *b) && items(ac, bc, |a, b| self.template(a, b))
            }
            (Template::Unary(a, av), Template::Unary(b, bv)) => a == b && self.template(av, bv),
            (Template::Binary(a, al, ar), Template::Binary(b, bl, br)) => {
                a == b && self.template(al, bl) && self.template(ar, br)
            }
            (Template::Absolute(a), Template::Absolute(b)) => self.template(a, b),
            (Template::Pool(a), Template::Pool(b)) => items(a, b, |a, b| self.template(a, b)),
            (Template::Interval(al, ar), Template::Interval(bl, br)) => {
                self.template(al, bl) && self.template(ar, br)
            }
            _ => false,
        }
    }
    fn operand(&self, a: &Operand, b: &Operand) -> bool {
        match (a, b) {
            (Operand::Constant(a), Operand::Constant(b)) => self.scalar(*a, *b),
            (Operand::Variable(a), Operand::Variable(b)) => a == b,
            (Operand::Any, Operand::Any) => true,
            (Operand::Construct(a, ac), Operand::Construct(b, bc)) => {
                self.shape(*a, *b) && items(ac, bc, |a, b| self.operand(a, b))
            }
            (Operand::Expression(a), Operand::Expression(b)) => self.template(a, b),
            (
                Operand::Inverse {
                    slot: a,
                    expression: av,
                },
                Operand::Inverse {
                    slot: b,
                    expression: bv,
                },
            ) => a == b && self.template(av, bv),
            _ => false,
        }
    }
    fn pattern(&self, a: &Pattern, b: &Pattern) -> bool {
        self.left.predicate(a.predicate) == self.right.predicate(b.predicate)
            && a.evaluated == b.evaluated
            && a.key == b.key
            && items(&a.terms, &b.terms, |a, b| self.operand(a, b))
    }
    fn key(&self, a: &KeyTemplate, b: &KeyTemplate) -> bool {
        match (a, b) {
            (KeyTemplate::Any, KeyTemplate::Any) => true,
            (KeyTemplate::Value(a), KeyTemplate::Value(b)) => self.template(a, b),
            (KeyTemplate::Construct(a, ac), KeyTemplate::Construct(b, bc)) => {
                self.shape(*a, *b) && items(ac, bc, |a, b| self.key(a, b))
            }
            (KeyTemplate::Pool(a), KeyTemplate::Pool(b)) => items(a, b, |a, b| self.key(a, b)),
            _ => false,
        }
    }
    fn atom_key(&self, a: &AtomKeyTemplate, b: &AtomKeyTemplate) -> bool {
        self.left.predicate(a.predicate) == self.right.predicate(b.predicate)
            && items(&a.arguments, &b.arguments, |a, b| self.key(a, b))
    }
    fn aggregate(&self, a: &AggregateQuery, b: &AggregateQuery) -> bool {
        a.function == b.function
            && items(&a.elements, &b.elements, |a, b| {
                self.aggregate_key(&a.key, &b.key) && self.query(&a.query, &b.query)
            })
    }
    fn aggregate_key(&self, a: &AggregateKey, b: &AggregateKey) -> bool {
        match (a, b) {
            (AggregateKey::Tuple(a), AggregateKey::Tuple(b)) => self.template(a, b),
            (
                AggregateKey::Atom {
                    negation: a,
                    slot: aslot,
                },
                AggregateKey::Atom {
                    negation: b,
                    slot: bslot,
                },
            ) => a == b && aslot == bslot,
            _ => false,
        }
    }
    fn condition(&self, a: &Condition, b: &Condition) -> bool {
        match (a, b) {
            (Condition::Atom(an, a), Condition::Atom(bn, b)) => {
                an == bn
                    && items(a, b, |a, b| {
                        self.pattern(&a.pattern, &b.pattern)
                            && self.query(&a.expansion, &b.expansion)
                    })
            }
            (Condition::AtomPatternValue(an, a), Condition::AtomPatternValue(bn, b)) => {
                an == bn && a == b
            }
            (Condition::Compare(an, a, ar), Condition::Compare(bn, b, br)) => {
                an == bn
                    && self.template(a, b)
                    && items(ar, br, |(a, av), (b, bv)| a == b && self.template(av, bv))
            }
            (Condition::Boolean(a), Condition::Boolean(b)) => a == b,
            (Condition::Conditional(a, ac), Condition::Conditional(b, bc)) => {
                self.query(a, b) && self.condition(ac, bc)
            }
            (Condition::Aggregate(an, a, ag), Condition::Aggregate(bn, b, bg)) => {
                an == bn
                    && self.aggregate(a, b)
                    && items(ag, bg, |a, b| {
                        a.relation == b.relation && self.template(&a.bound, &b.bound)
                    })
            }
            _ => false,
        }
    }
    fn binder(&self, a: &Binder, b: &Binder) -> bool {
        match (a, b) {
            (Binder::Atom(a), Binder::Atom(b)) => items(a, b, |a, b| self.pattern(a, b)),
            (Binder::Assign(a, av), Binder::Assign(b, bv)) => a == b && self.template(av, bv),
            (Binder::AtomKey(a, av), Binder::AtomKey(b, bv)) => a == b && self.atom_key(av, bv),
            (
                Binder::Match {
                    patterns: a,
                    value: av,
                    complete: ac,
                },
                Binder::Match {
                    patterns: b,
                    value: bv,
                    complete: bc,
                },
            ) => ac == bc && self.template(av, bv) && items(a, b, |a, b| self.operand(a, b)),
            (Binder::Aggregate(a, av), Binder::Aggregate(b, bv)) => {
                a == b && self.aggregate(av, bv)
            }
            (Binder::NumericMismatch(a), Binder::NumericMismatch(b)) => self.aggregate(a, b),
            _ => false,
        }
    }
    fn query(&self, a: &Query, b: &Query) -> bool {
        a.variables == b.variables
            && a.inputs == b.inputs
            && items(&a.binders, &b.binders, |a, b| self.binder(a, b))
            && items(&a.conditions, &b.conditions, |a, b| self.condition(a, b))
    }
}
impl PartialEq for ObservationProgram {
    fn eq(&self, other: &Self) -> bool {
        let (Some(a), Some(b)) = (&self.data, &other.data) else {
            return self.data.is_none() && other.data.is_none();
        };
        if Arc::ptr_eq(a, b) {
            return true;
        }
        let left = a
            .vocabulary
            .read_with(|| Ok::<_, std::convert::Infallible>(()))
            .expect("published immutable component prefix");
        let right = b
            .vocabulary
            .read_with(|| Ok::<_, std::convert::Infallible>(()))
            .expect("published immutable component prefix");
        let identity = Identity { left, right };
        items(&a.directives, &b.directives, |a, b| {
            a.origins == b.origins
                && identity.template(&a.term, &b.term)
                && identity.query(&a.query, &b.query)
        })
    }
}
impl Eq for ObservationProgram {}

#[cfg(test)]
mod tests {
    use super::*;
    use themelios_base::source::{Source, SourceId};
    use themelios_base::span::Location;
    use themelios_syntax::{dialect::Dialect, parse::parse};
    fn compile(text: &str) -> crate::SourceMetadata {
        let source = Source::new(SourceId::new(19), text.into()).unwrap();
        let parsed = parse(&source, Dialect::Clingo);
        assert!(parsed.diagnostics().is_empty());
        let raised = themelios_program::raise::raise(&parsed);
        assert!(raised.diagnostics().is_empty());
        crate::SourceMetadata::compile(
            raised.program(),
            crate::MetadataLimits::default(),
            Location {
                source: source.id(),
                span: source.span(),
            },
        )
        .unwrap()
    }
    #[test]
    fn compiled_scalar_and_selector_borrow_one_spelling() {
        let metadata = compile("#show shared/0. #show shared.");
        let read = metadata
            .observations()
            .read_with(|| Ok::<_, std::convert::Infallible>(()))
            .unwrap()
            .unwrap();
        let Template::Constant(scalar) = read.directives[0].term else {
            panic!("ground constant template")
        };
        let zetesis_core::ValueNodeRef::Symbol(name) =
            read.metadata.scalar(scalar).unwrap().descriptor()
        else {
            panic!("positive named constant")
        };
        let predicate = metadata.output().signatures().at(0).unwrap();
        assert_eq!(name, predicate.name());
        assert_eq!(name.as_ptr(), predicate.name().as_ptr());
    }
}
