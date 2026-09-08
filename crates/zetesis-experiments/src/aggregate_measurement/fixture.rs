use super::{Case, Configuration, Error, reserve};
use zetesis_core::Value;
use zetesis_cpu::Control;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison, Interpretation, Node, Theory,
    native_aggregate::{self as native, Bound, Group, Guard, Tuple},
};

const WEIGHTS: [i32; 5] = [-3, 2, 0, 7, -1];
const CONDITIONS: [usize; 8] = [2, 3, 4, 5, 6, 7, 8, 9];
const INTERPRETATIONS: usize = 4;
// Four tested interpretations plus one original-only observation.
const TESTED_CHOICES: usize = INTERPRETATIONS + 1;

pub(super) struct Fixture {
    pub(super) group: Group,
    pub(super) worlds: Vec<Interpretation>,
    pub(super) interpretations: Vec<(u8, Option<u8>)>,
}

pub(super) fn build(case: Case, control: &Control) -> Result<Fixture, Error> {
    control.poll().map_err(Error::Cpu)?;
    let theory = Theory::new(
        2,
        vec![
            Node::False,
            Node::Implies(0, 0),
            Node::Atom(0),
            Node::Atom(1),
            Node::Implies(2, 0),
            Node::Implies(4, 0),
            Node::Implies(3, 0),
            Node::And(2, 3),
            Node::Or(2, 3),
            Node::Implies(2, 3),
        ],
        vec![],
        AdmissionLimits::default(),
    )
    .map_err(Error::Admission)?;
    let mut tuples = reserve(case.tuples)?;
    for index in 0..case.tuples {
        control.poll().map_err(Error::Cpu)?;
        let mut key = reserve(2)?;
        if index != 0 {
            let index_value = i32::try_from(index).map_err(|_| Error::Accounting)?;
            key.extend([
                Value::Number(WEIGHTS[index % WEIGHTS.len()]),
                Value::Number(index_value),
            ]);
        }
        tuples.push(Tuple {
            key,
            condition: CONDITIONS[index % CONDITIONS.len()],
        });
    }
    let group = Group::new(
        &theory,
        case.function.into(),
        tuples,
        vec![Guard {
            comparison: AggregateComparison::Ge,
            bound: Bound::Integer(0),
        }],
        native::AdmissionLimits::default(),
        control,
    )
    .map_err(Error::Native)?;
    let mut worlds = reserve(INTERPRETATIONS)?;
    for bits in 0..INTERPRETATIONS {
        worlds.push(
            Interpretation::new(&theory, (0..2).filter(|atom| bits & (1 << atom) != 0))
                .map_err(Error::Admission)?,
        );
    }
    let mut interpretations = reserve(case.occurrences.get())?;
    for index in 0..case.occurrences.get() {
        interpretations.push((
            u8::try_from((index / TESTED_CHOICES) % INTERPRETATIONS)
                .map_err(|_| Error::Accounting)?,
            (index % TESTED_CHOICES != INTERPRETATIONS)
                .then(|| u8::try_from(index % TESTED_CHOICES).expect("two-bit interpretation")),
        ));
    }
    Ok(Fixture {
        group,
        worlds,
        interpretations,
    })
}

pub(super) fn acquire<'a>(
    fixture: &'a Fixture,
    configuration: &Configuration,
    control: &Control,
) -> Result<Vec<native::Eligibility<'a>>, Error> {
    let mut records = reserve(fixture.interpretations.len())?;
    for &(original, frozen) in &fixture.interpretations {
        records.push(
            fixture
                .group
                .eligibility(
                    &fixture.worlds[usize::from(original)],
                    frozen.map(|index| &fixture.worlds[usize::from(index)]),
                    native::EligibilityLimits {
                        max_work: configuration.max_acquisition_work,
                        max_bytes: configuration.max_acquisition_bytes,
                    },
                    control,
                )
                .map_err(Error::Native)?,
        );
    }
    Ok(records)
}
