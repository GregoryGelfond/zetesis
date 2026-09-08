//! Portable selection/label evidence; no physical adapter is constructed.

use clap::Parser;
use zetesis_experiments::{Backend, CommandOptions, Experiment};

fn backend(options: &CommandOptions) -> Backend {
    match &options.command {
        None => options.static_options.backend,
        Some(Experiment::Formula(value) | Experiment::FormulaProjection(value)) => value.backend,
        Some(Experiment::Lazy(value)) => value.backend,
        Some(Experiment::Tight(value)) => value.backend,
        Some(Experiment::Aggregate(value)) => value.backend,
        Some(Experiment::Grounding(_)) => panic!("grounding measurements are CPU only"),
    }
}

#[test]
fn vulkan_is_accepted_by_every_device_experiment() {
    for command in [
        None,
        Some("formula"),
        Some("formula-projection"),
        Some("lazy"),
        Some("tight"),
        Some("aggregate"),
    ] {
        let mut arguments = vec!["zetesis-bench"];
        arguments.extend(command);
        arguments.extend(["--backend", "vulkan"]);
        let parsed = CommandOptions::try_parse_from(arguments).unwrap();
        assert_eq!(backend(&parsed), Backend::Vulkan);
    }
}

#[test]
fn default_experiment_backend_remains_metal() {
    for command in [
        None,
        Some("formula"),
        Some("formula-projection"),
        Some("lazy"),
        Some("tight"),
        Some("aggregate"),
    ] {
        let mut arguments = vec!["zetesis-bench"];
        arguments.extend(command);
        let parsed = CommandOptions::try_parse_from(arguments).unwrap();
        assert_eq!(backend(&parsed), Backend::Metal);
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
