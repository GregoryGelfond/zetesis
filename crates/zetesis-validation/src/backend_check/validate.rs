use super::{Backend, Decision, Family, run::Fixture};
use crate::{
    answers::{self, native_json},
    performance::matrix::{DeviceWork, Observation, Procedure},
    process, selected,
};

pub(super) fn capture(
    capture: &process::Capture,
    backend: Backend,
    fixture: &Fixture,
) -> Result<(Family, Observation), (Decision, String)> {
    match capture.stop() {
        process::Stop::Cancelled => {
            return Err((Decision::Cancelled, "backend check cancelled".into()));
        }
        process::Stop::Deadline => {
            return Err((Decision::Timeout, "process deadline reached".into()));
        }
        process::Stop::OutputLimit => {
            return Err((
                Decision::CaptureLimit,
                "combined output ceiling reached".into(),
            ));
        }
        process::Stop::Failure => {
            return Err((Decision::ProcessFailure, "process capture failed".into()));
        }
        process::Stop::Completed => (),
    }
    if capture.failure().is_some() || capture.cleanup_failure().is_some() {
        return Err((Decision::ProcessFailure, "capture or cleanup failed".into()));
    }
    match capture.exit().and_then(|exit| exit.code) {
        Some(0) => (),
        Some(3) => {
            return Err((
                Decision::Incomplete,
                "native search or publication was interrupted".into(),
            ));
        }
        exit => {
            return Err((
                Decision::ProcessFailure,
                format!("native exit {exit:?}; inspect retained stderr"),
            ));
        }
    }
    let family =
        family(capture.stdout()).map_err(|error| (Decision::InvalidReport, error.to_string()))?;
    if family != fixture.expected() {
        return Err((
            Decision::FamilyMismatch,
            "full selected models or priority/cost vector differ from the fixed contract".into(),
        ));
    }
    let document: serde_json::Value = serde_json::from_slice(capture.stdout())
        .map_err(|error| (Decision::InvalidReport, error.to_string()))?;
    if document["statistics"]["search"]["work"]
        .as_u64()
        .is_none_or(|work| work == 0)
    {
        return Err((
            Decision::InvalidExecution,
            "positive formula search work was not reported".into(),
        ));
    }
    let observation = Observation::from_statistics(
        &document,
        capture.stderr(),
        selected::NativeExecution {
            backend,
            oracle: fixture.oracle,
            ..selected::NativeExecution::default()
        },
    )
    .map_err(|error| (Decision::InvalidExecution, error))?;
    execution(&observation, fixture.oracle).map_err(|error| (Decision::InvalidExecution, error))?;
    Ok((family, observation))
}

fn family(bytes: &[u8]) -> Result<Family, answers::Error> {
    let decoded = native_json::parse(
        bytes,
        native_json::Limits {
            report: answers::Limits::for_bytes(bytes.len()),
            ..native_json::Limits::default()
        },
    )?;
    Ok(Family {
        models: decoded.full_model_symbols(bytes.len())?,
        costs: decoded.costs().map(<[_]>::to_vec),
    })
}

fn execution(observation: &Observation, oracle: selected::Oracle) -> Result<(), String> {
    let expected = if oracle == selected::Oracle::Countermodel {
        Procedure::Countermodel
    } else {
        Procedure::TightSupport
    };
    if observation.execution.procedure != expected {
        return Err("fixture did not exercise its required formula procedure".into());
    }
    let ran = match observation.execution.device {
        DeviceWork::Cpu => observation.execution.backend == Backend::Cpu,
        DeviceWork::Formula {
            batches,
            candidates,
            work,
            ..
        }
        | DeviceWork::TightSupport {
            batches,
            candidates,
            work,
            ..
        } => batches > 0 && candidates > 0 && work > 0,
        _ => false,
    };
    if ran {
        Ok(())
    } else {
        Err("required device work was not observed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::family;
    use serde_json::json;

    fn document() -> serde_json::Value {
        json!({"schema":2,"format":"zetesis","models":[
            {"number":1,"model":{"atoms":[{"predicate":"hidden","sign":"positive","arguments":[]}],"full_model":[0],"shown":{"atom_indices":[],"terms":[]},"costs":null}}
        ],"outcome":{"status":"satisfiable","completion":"exhausted","coverage":"exhausted","published_models":1,"verified_models":1,"checked":1,"interruption":null,"optimization":null,"error":null},"statistics":null})
    }
    #[test]
    fn hidden_atoms_are_part_of_the_checked_family() {
        let result = family(&serde_json::to_vec(&document()).unwrap()).unwrap();
        assert_eq!(result.models, [vec!["hidden"]]);
    }
    #[test]
    fn incomplete_output_cannot_establish_a_family() {
        let mut value = document();
        value["outcome"]["coverage"] = "partial".into();
        assert!(family(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn repeated_full_models_remain_detectable() {
        let mut value = document();
        let mut second = value["models"][0].clone();
        second["number"] = 2.into();
        second["model"]["atoms"] = json!([]);
        value["models"].as_array_mut().unwrap().push(second);
        for field in ["published_models", "verified_models", "checked"] {
            value["outcome"][field] = 2.into();
        }
        let result = family(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(result.models, [vec!["hidden"], vec!["hidden"]]);
    }
}
