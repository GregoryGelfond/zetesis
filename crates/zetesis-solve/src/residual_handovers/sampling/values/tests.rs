use super::{Totals, Value};
use crate::ResidualQuantiles;

#[test]
fn unrepresentable_observation_count_withholds_whole_population_claims() {
    let mut total = Totals {
        eligible: Some(u64::MAX),
        known: Some(u64::MAX),
        sum: Some(0),
        maximum: Some(0),
    };
    total.observe(Value::Known(0));
    let metric = total.finish(&mut [0], |&v| Some(v));
    assert_eq!((metric.known, metric.eligible), (None, None));
    assert_eq!(metric.quantiles, ResidualQuantiles::Unavailable);
    assert_eq!((metric.sum, metric.maximum), (None, None));
}
