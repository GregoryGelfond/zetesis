//! The vocabulary's spellings, resolution and command-line and record forms.

use super::*;

#[test]
fn every_value_reads_back_from_its_spelling() {
    for backend in Backend::ALL {
        assert_eq!(backend.label().parse::<Backend>(), Ok(backend));
    }
}

#[test]
fn the_values_are_cpu_gpu_metal_and_vulkan_in_that_order() {
    let labels: Vec<_> = Backend::ALL.into_iter().map(Backend::label).collect();
    assert_eq!(labels, ["cpu", "gpu", "metal", "vulkan"]);
}

#[test]
fn the_default_backend_is_the_cpu() {
    assert_eq!(Backend::default(), Backend::Cpu);
}

#[test]
fn a_retired_spelling_names_what_to_use_instead() {
    for (spelling, replacement) in [
        ("auto", "CPU is the default"),
        ("dx12", "use gpu, metal or vulkan"),
        ("gl", "use gpu, metal or vulkan"),
        ("nvidia", "use gpu or vulkan"),
    ] {
        let error = spelling.parse::<Backend>().unwrap_err();
        assert_eq!(error.spelling(), spelling);
        assert!(
            error
                .retired()
                .is_some_and(|reason| reason.contains(replacement))
        );
        assert!(error.to_string().contains("is no longer a backend"));
    }
}

#[test]
fn an_unknown_spelling_lists_the_values() {
    let error = "tpu".parse::<Backend>().unwrap_err();
    assert_eq!(error.retired(), None);
    assert_eq!(
        error.to_string(),
        "`tpu` is not a backend; use cpu, gpu, metal or vulkan"
    );
}

#[test]
fn spellings_are_case_sensitive() {
    assert!("Metal".parse::<Backend>().is_err());
}

#[test]
fn the_native_api_is_metal_on_apple_targets_and_vulkan_elsewhere() {
    let expected = if cfg!(target_vendor = "apple") {
        GpuApi::Metal
    } else {
        GpuApi::Vulkan
    };
    assert_eq!(GpuApi::native(), expected);
}

#[test]
fn a_gpu_request_without_an_api_resolves_to_the_native_one() {
    assert_eq!(Backend::Gpu(None).resolved_api(), Some(GpuApi::native()));
}

#[test]
fn a_named_api_resolves_to_itself() {
    for api in [GpuApi::Metal, GpuApi::Vulkan] {
        assert_eq!(Backend::Gpu(Some(api)).resolved_api(), Some(api));
    }
}

#[test]
fn an_api_has_a_spelling_and_a_proper_name() {
    assert_eq!(
        [GpuApi::Metal, GpuApi::Vulkan].map(|api| (api.label(), api.name())),
        [("metal", "Metal"), ("vulkan", "Vulkan")]
    );
}

#[test]
fn the_cpu_resolves_to_no_api() {
    assert_eq!(Backend::Cpu.resolved_api(), None);
    assert!(!Backend::Cpu.is_gpu());
}

#[test]
fn default_threads_are_the_host_parallelism_at_most_four() {
    let host = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
    assert_eq!(default_threads().get(), host.min(4));
}

#[test]
fn auto_threads_mean_the_default() {
    assert_eq!(parse_threads("auto"), Ok(default_threads()));
}

#[test]
fn a_positive_thread_count_is_read_as_given() {
    assert_eq!(parse_threads("7").map(NonZeroUsize::get), Ok(7));
}

#[test]
fn other_thread_spellings_are_refused() {
    for value in ["0", "-1", "four", ""] {
        assert!(parse_threads(value).is_err(), "{value}");
    }
}

#[cfg(feature = "clap")]
mod command_line {
    use super::*;
    use clap::{Arg, Command};

    fn command() -> Command {
        Command::new("zetesis").arg(
            Arg::new("backend")
                .long("backend")
                .value_parser(BackendParser)
                .default_value("cpu"),
        )
    }

    #[test]
    fn help_lists_the_four_values_with_their_meaning() {
        let values: Vec<_> = clap::builder::TypedValueParser::possible_values(&BackendParser)
            .unwrap()
            .map(|value| {
                (
                    value.get_name().to_owned(),
                    value.get_help().map(ToString::to_string),
                )
            })
            .collect();
        let expected: Vec<_> = Backend::ALL
            .into_iter()
            .map(|backend| (backend.label().to_owned(), Some(backend.help().to_owned())))
            .collect();
        assert_eq!(values, expected);
    }

    #[test]
    fn a_command_line_value_parses_to_its_backend() {
        let matches = command().try_get_matches_from(["zetesis", "--backend", "vulkan"]);
        let backend = matches.unwrap().get_one::<Backend>("backend").copied();
        assert_eq!(backend, Some(Backend::Gpu(Some(GpuApi::Vulkan))));
    }

    #[test]
    fn an_ignore_case_argument_reads_any_case() {
        let matches = command()
            .mut_arg("backend", |argument| argument.ignore_case(true))
            .try_get_matches_from(["zetesis", "--backend", "METAL"]);
        let backend = matches.unwrap().get_one::<Backend>("backend").copied();
        assert_eq!(backend, Some(Backend::Gpu(Some(GpuApi::Metal))));
    }

    #[test]
    fn a_case_sensitive_argument_refuses_another_case() {
        let error = command()
            .try_get_matches_from(["zetesis", "--backend", "METAL"])
            .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn a_retired_command_line_value_is_explained() {
        let error = command()
            .try_get_matches_from(["zetesis", "--backend", "dx12"])
            .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
        assert!(error.to_string().contains("`dx12` is no longer a backend"));
    }

    #[test]
    fn a_retired_value_is_explained_in_any_case_when_case_is_ignored() {
        let error = command()
            .mut_arg("backend", |argument| argument.ignore_case(true))
            .try_get_matches_from(["zetesis", "--backend", "AUTO"])
            .unwrap_err();
        assert!(error.to_string().contains("`AUTO` is no longer a backend"));
    }

    #[test]
    fn an_unknown_command_line_value_lists_the_possible_values() {
        let error = command()
            .try_get_matches_from(["zetesis", "--backend", "tpu"])
            .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
        assert!(
            error
                .to_string()
                .contains("[possible values: cpu, gpu, metal, vulkan]")
        );
    }

    #[test]
    fn a_near_miss_gets_a_suggestion() {
        let error = command()
            .try_get_matches_from(["zetesis", "--backend", "meta"])
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("tip: a similar value exists: 'metal'")
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn records_store_the_stable_spelling() {
    for backend in Backend::ALL {
        assert_eq!(
            serde_json::to_string(&backend).unwrap(),
            format!("\"{}\"", backend.label())
        );
    }
}
