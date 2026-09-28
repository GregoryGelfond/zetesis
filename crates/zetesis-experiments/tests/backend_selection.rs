//! Portable selection/label evidence; no physical adapter is constructed.

use clap::Parser;
use zetesis_backend::GpuApi;
use zetesis_experiments::{Backend, CommandOptions, Experiment};

fn backend(options: &CommandOptions) -> Backend {
    match &options.command {
        None => options.static_options.backend,
        Some(Experiment::Formula(value) | Experiment::FormulaProjection(value)) => value.backend,
        Some(Experiment::Lazy(value)) => value.backend,
        Some(Experiment::Tight(value)) => value.backend,
        Some(Experiment::Aggregate(value)) => value.backend,
        Some(Experiment::Grounding(_) | Experiment::Table(_) | Experiment::Feedback(_)) => {
            panic!("this experiment has no device backend")
        }
        Some(Experiment::Relation(value)) => value.backend,
    }
}

const DEVICE_EXPERIMENTS: [Option<&str>; 7] = [
    None,
    Some("formula"),
    Some("formula-projection"),
    Some("lazy"),
    Some("tight"),
    Some("aggregate"),
    Some("relation"),
];

fn parse(command: Option<&str>, flags: &[&'static str]) -> Result<CommandOptions, clap::Error> {
    let mut arguments = vec!["zetesis-bench"];
    arguments.extend(command);
    arguments.extend(flags);
    CommandOptions::try_parse_from(arguments)
}

#[test]
fn every_backend_value_is_accepted_by_every_device_experiment() {
    for command in DEVICE_EXPERIMENTS {
        for expected in Backend::ALL {
            let parsed = parse(command, &["--backend", expected.label()]).unwrap();
            assert_eq!(backend(&parsed), expected, "{command:?}");
        }
    }
}

#[test]
fn the_default_experiment_backend_is_the_cpu() {
    for command in DEVICE_EXPERIMENTS {
        assert_eq!(
            backend(&parse(command, &[]).unwrap()),
            Backend::Cpu,
            "{command:?}"
        );
    }
}

#[test]
fn no_experiment_accepts_a_device_flag() {
    for command in DEVICE_EXPERIMENTS {
        assert!(
            parse(command, &["--device", "vulkan"]).is_err(),
            "{command:?}"
        );
    }
}

#[test]
fn vulkan_route_labels_are_distinct_from_metal() {
    use zetesis_experiments::{lazy_measurement, tight_measurement};
    for (route, expected) in [
        (lazy_measurement::Route::VulkanUnion, "vulkan-union"),
        (lazy_measurement::Route::VulkanWorlds, "vulkan-worlds"),
        (lazy_measurement::Route::MetalUnion, "metal-union"),
        (lazy_measurement::Route::MetalWorlds, "metal-worlds"),
    ] {
        assert_eq!(serde_json::to_value(route).unwrap(), expected);
    }
    for (route, expected) in [
        (tight_measurement::Route::VulkanFresh, "vulkan-fresh"),
        (tight_measurement::Route::VulkanResident, "vulkan-resident"),
        (tight_measurement::Route::MetalFresh, "metal-fresh"),
        (tight_measurement::Route::MetalResident, "metal-resident"),
    ] {
        assert_eq!(serde_json::to_value(route).unwrap(), expected);
    }
}

#[test]
fn configurations_retain_the_vulkan_request() {
    for command in ["lazy", "tight", "aggregate"] {
        let parsed =
            CommandOptions::try_parse_from(["zetesis-bench", command, "--backend", "vulkan"])
                .unwrap();
        let configuration = match parsed.command.unwrap() {
            Experiment::Lazy(options) => {
                serde_json::to_value(options.configuration().unwrap()).unwrap()
            }
            Experiment::Tight(options) => {
                serde_json::to_value(options.configuration().unwrap()).unwrap()
            }
            Experiment::Aggregate(options) => {
                serde_json::to_value(options.configuration().unwrap()).unwrap()
            }
            _ => panic!("expected an event-based experiment"),
        };
        assert_eq!(configuration["backend"], "vulkan");
    }
}

#[test]
fn a_gpu_request_records_the_native_api_it_runs_on() {
    let parsed =
        CommandOptions::try_parse_from(["zetesis-bench", "lazy", "--backend", "gpu"]).unwrap();
    let Some(Experiment::Lazy(options)) = parsed.command else {
        panic!("expected the lazy experiment");
    };
    let configuration = serde_json::to_value(options.configuration().unwrap()).unwrap();
    assert_eq!(configuration["backend"], GpuApi::native().label());
}
