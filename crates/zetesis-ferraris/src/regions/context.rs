//! Class-indexed reading inputs and disjoint worker-learning capabilities.
//!
//! Only the candidate instantiation borrows learned rows. A frozen context owns
//! no such reference; its shared closure still uses the same queues and kernels.

use zetesis_algebra::{AnswerSetsSought, Class, Countermodels};

use super::{
    ChoiceHeads, FrozenSubject, GeneratorOrder, LearningOwner, LearningScratch, NO_CHOICES,
    OriginalSubject, Producers, Theory,
};

/// Private static policy for the two sealed class kinds. It adds no runtime tag
/// or public extension point, and does not decide whether an interpretation is
/// a member of either class.
pub(super) trait ReadingClass: Class {
    type Data<'a>: Copy;
    type Learning<'a>: LearningAccess;

    fn theory(data: Self::Data<'_>) -> &Theory;
    fn producers(data: Self::Data<'_>) -> Option<&Producers>;
    fn frozen(data: Self::Data<'_>) -> Option<&[bool]>;
    fn choices(data: Self::Data<'_>) -> &ChoiceHeads;
    fn generator_order(data: Self::Data<'_>) -> Option<&GeneratorOrder>;
    fn learning(data: Self::Data<'_>) -> Option<&LearningOwner>;
    fn borrow_learning(state: &mut LearningScratch) -> Self::Learning<'_>;
}

/// The actual per-call capability, not an owner or a second learned-row store.
pub(super) trait LearningAccess {
    fn state(&self) -> Option<&LearningScratch>;
    fn state_mut(&mut self) -> Option<&mut LearningScratch>;
}

impl LearningAccess for &mut LearningScratch {
    fn state(&self) -> Option<&LearningScratch> {
        Some(self)
    }
    fn state_mut(&mut self) -> Option<&mut LearningScratch> {
        Some(self)
    }
}

impl LearningAccess for () {
    fn state(&self) -> Option<&LearningScratch> {
        None
    }
    fn state_mut(&mut self) -> Option<&mut LearningScratch> {
        None
    }
}

/// What one narrowing reads, indexed by the class of its regional knowledge.
pub(super) struct Context<'a, C: ReadingClass> {
    data: C::Data<'a>,
}

impl<C: ReadingClass> Copy for Context<'_, C> {}
impl<C: ReadingClass> Clone for Context<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, C: ReadingClass> Context<'a, C> {
    pub(super) fn theory(self) -> &'a Theory {
        C::theory(self.data)
    }
    pub(super) fn producers(self) -> Option<&'a Producers> {
        C::producers(self.data)
    }
    pub(super) fn frozen(self) -> Option<&'a [bool]> {
        C::frozen(self.data)
    }
    pub(super) fn choices(self) -> &'a ChoiceHeads {
        C::choices(self.data)
    }
    pub(super) fn generator_order(self) -> Option<&'a GeneratorOrder> {
        C::generator_order(self.data)
    }
    pub(super) fn learning(self) -> Option<&'a LearningOwner> {
        C::learning(self.data)
    }
}

impl<'a> From<OriginalSubject<'a>> for Context<'a, AnswerSetsSought> {
    fn from(data: OriginalSubject<'a>) -> Self {
        Self { data }
    }
}

impl<'a> From<FrozenSubject<'a>> for Context<'a, Countermodels> {
    fn from(data: FrozenSubject<'a>) -> Self {
        Self { data }
    }
}

impl ReadingClass for AnswerSetsSought {
    type Data<'a> = OriginalSubject<'a>;
    type Learning<'a> = &'a mut LearningScratch;

    fn theory(data: Self::Data<'_>) -> &Theory {
        data.theory
    }
    fn producers(data: Self::Data<'_>) -> Option<&Producers> {
        data.producers
    }
    fn frozen(_: Self::Data<'_>) -> Option<&[bool]> {
        None
    }
    fn choices(data: Self::Data<'_>) -> &ChoiceHeads {
        data.choices
    }
    fn generator_order(data: Self::Data<'_>) -> Option<&GeneratorOrder> {
        data.generator_order
    }
    fn learning(data: Self::Data<'_>) -> Option<&LearningOwner> {
        data.learning
    }
    fn borrow_learning(state: &mut LearningScratch) -> Self::Learning<'_> {
        state
    }
}

impl ReadingClass for Countermodels {
    type Data<'a> = FrozenSubject<'a>;
    type Learning<'a> = ();

    fn theory(data: Self::Data<'_>) -> &Theory {
        data.theory
    }
    fn producers(_: Self::Data<'_>) -> Option<&Producers> {
        None
    }
    fn frozen(data: Self::Data<'_>) -> Option<&[bool]> {
        Some(data.truth)
    }
    fn choices(_: Self::Data<'_>) -> &ChoiceHeads {
        &NO_CHOICES
    }
    fn generator_order(_: Self::Data<'_>) -> Option<&GeneratorOrder> {
        None
    }
    fn learning(_: Self::Data<'_>) -> Option<&LearningOwner> {
        None
    }
    fn borrow_learning(_: &mut LearningScratch) -> Self::Learning<'_> {}
}
