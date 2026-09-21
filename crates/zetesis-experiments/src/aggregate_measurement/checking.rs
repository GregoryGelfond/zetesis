use super::{Activity, Error, Outcome, Value, reserve, view::Evaluation};
use rayon::prelude::*;
use zetesis_core::Value as Term;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::native_aggregate as native;
use zetesis_wgpu::{AggregateGpuEvaluation, AggregateGpuReduction, AggregateGpuValue};

pub(super) fn cpu(
    records: &[native::Eligibility<'_>],
    max_work: u64,
    pool: Option<&rayon::ThreadPool>,
    cancellation: &Cancellation,
    activity: &mut Activity,
) -> Result<Vec<Outcome>, Error> {
    let mut results = reserve(records.len())?;
    results.resize_with(records.len(), || None);
    activity.cpu_attempts = records.len();
    let limits = native::ReductionLimits { max_work };
    if let Some(pool) = pool {
        pool.install(|| {
            results
                .par_iter_mut()
                .zip(records.par_iter())
                .for_each(|(slot, record)| *slot = Some(record.reduce(limits, cancellation)));
        });
    } else {
        for (slot, record) in results.iter_mut().zip(records) {
            *slot = Some(record.reduce(limits, cancellation));
        }
    }
    let mut failure = None;
    for result in &results {
        let work = match result
            .as_ref()
            .expect("every indexed occurrence was joined")
        {
            Ok(value) => {
                activity.cpu_completed += 1;
                value.statistics().work
            }
            Err(error) => {
                failure.get_or_insert(*error);
                error.statistics().work
            }
        };
        activity.cpu_work = activity
            .cpu_work
            .checked_add(work)
            .ok_or(Error::Accounting)?;
    }
    if let Some(error) = failure {
        return Err(Error::Native(error));
    }
    let mut outcomes = reserve(records.len())?;
    for value in results {
        outcomes.push(native_outcome(
            value.expect("joined occurrence").map_err(Error::Native)?,
        )?);
    }
    Ok(outcomes)
}

fn native_outcome(value: native::Reduction<'_>) -> Result<Outcome, Error> {
    Ok(Outcome {
        original: native_evaluation(value.original())?,
        frozen: value.frozen().map(native_evaluation).transpose()?,
        reduct_truth: value.reduct_truth(),
    })
}
fn native_evaluation(value: native::Evaluation<'_>) -> Result<Evaluation, Error> {
    let measure = match value.value() {
        native::Value::Integer(value) => Value::Integer(value),
        native::Value::Term(Term::Infimum) => Value::Infimum,
        native::Value::Term(Term::Supremum) => Value::Supremum,
        native::Value::Term(_) => return Err(Error::Parity),
    };
    Ok(Evaluation {
        value: measure,
        holds: value.holds(),
    })
}

pub(super) fn device(values: &[AggregateGpuReduction]) -> Result<Vec<Outcome>, Error> {
    let mut outcomes = reserve(values.len())?;
    outcomes.extend(values.iter().map(|value| Outcome {
        original: device_evaluation(value.original()),
        frozen: value.frozen().map(device_evaluation),
        reduct_truth: value.reduct_truth(),
    }));
    Ok(outcomes)
}
fn device_evaluation(value: AggregateGpuEvaluation) -> Evaluation {
    Evaluation {
        value: match value.value() {
            AggregateGpuValue::Integer(value) => Value::Integer(i128::from(value)),
            AggregateGpuValue::Infimum => Value::Infimum,
            AggregateGpuValue::Supremum => Value::Supremum,
        },
        holds: value.holds(),
    }
}
