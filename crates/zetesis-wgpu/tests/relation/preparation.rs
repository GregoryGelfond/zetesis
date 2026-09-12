//! Actual mapped-copy interruption and subsequent same-context GPU work.

use super::*;
use crate::GpuBackendPreference;
use zetesis_core::{Atom, Predicate, Value, relation::Limits};
use zetesis_cpu::Stop;

fn interrupted_copy(backend: GpuBackendPreference) {
    let mut executor = GpuRelationExecutor::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend,
            vendor_id: None,
        },
    )
    .unwrap();
    assert!(executor.info().is_hardware_gpu());
    assert_eq!(
        executor.info().backend(),
        match backend {
            GpuBackendPreference::Metal => "Metal",
            GpuBackendPreference::Vulkan => "Vulkan",
            _ => panic!("explicit physical API required"),
        }
    );
    eprintln!("settled preparation adapter={:?}", executor.info());
    let predicate = Predicate::new("row", 2).unwrap();
    let atoms = [[0, 10], [1, 11], [0, 12]]
        .map(|row| Atom::new(predicate.clone(), row.map(Value::Number).to_vec()).unwrap());
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let queries = [relation
        .query(&[(0, &Value::Number(0))], Limits::default())
        .unwrap()];
    let context = executor.context().clone();
    for stop in [Stop::Cancelled, Stop::Deadline] {
        // Poll 1 precedes allocation; 2/3 precede columns 0/1; poll 4 follows
        // both copies. Every refusal below therefore owns a mapped buffer.
        for stop_at in [2, 3, 4] {
            let mut polls = 0;
            let result = executor.prepare_with(&relation, RelationGpuLimits::default(), || {
                polls += 1;
                if polls == stop_at {
                    Err(GpuError::interrupted(stop))
                } else {
                    Ok(())
                }
            });
            let Err(failure) = result else {
                panic!("interrupted preparation returned a view");
            };
            assert!(matches!(failure, RelationGpuError::Stopped(actual) if actual == stop));
            assert_eq!(polls, stop_at);
            context.check_health().unwrap();
            drop(context.lease().unwrap());
            let mut prepared = executor
                .prepare(&relation, RelationGpuLimits::default(), &Control::default())
                .unwrap();
            let masks = prepared
                .filter(&queries, RelationGpuLimits::default(), &Control::default())
                .unwrap();
            assert_eq!(
                masks.selection(0, Limits::default()).unwrap().positions(),
                [0, 2]
            );
            assert_eq!(prepared.activity().submissions, 1);
            assert_eq!(prepared.activity().completed_queries, 1);
        }
    }
}

#[test]
#[ignore = "requires an actual physical Metal adapter"]
fn metal_interrupted_preparation_preserves_context() {
    interrupted_copy(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires an actual physical Vulkan adapter"]
fn vulkan_interrupted_preparation_preserves_context() {
    interrupted_copy(GpuBackendPreference::Vulkan);
}
