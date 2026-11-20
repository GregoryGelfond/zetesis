use super::*;
use crate::{GpuDeviceSource, GpuErrorKind};
use zetesis_backend::GpuApi;

const METAL: GpuSelection = GpuSelection { api: GpuApi::Metal };

fn refused() -> GpuError {
    GpuError::new(
        GpuErrorKind::AdapterUnavailable,
        "no Metal adapter was found",
    )
}

fn finished() -> State {
    State::Running(std::thread::spawn(|| Err(refused())))
}

#[test]
fn a_discovery_serves_one_consumer() {
    let shared = GpuDiscovery::with_state(METAL, finished());
    assert!(shared.clone().take().is_some());
    assert!(shared.take().is_none());
}

#[test]
fn a_panicked_discovery_leaves_consumers_to_enumerate() {
    let panicked = GpuDiscovery::with_state(
        METAL,
        State::Running(std::thread::spawn(
            || -> Result<Vec<Candidate>, GpuError> { panic!("discovery thread failure") },
        )),
    );
    assert!(panicked.take().is_none());
}

#[test]
fn an_unstarted_discovery_leaves_consumers_to_enumerate() {
    assert!(
        GpuDiscovery::with_state(METAL, State::Spent)
            .take()
            .is_none()
    );
}

#[test]
fn a_prefetched_enumeration_failure_is_the_consumer_failure() {
    let refusing = GpuDiscovery::with_state(METAL, finished());
    let Err(error) = pollster::block_on(refusing.context(
        GpuOptions::default(),
        "test device",
        |_| Ok(()),
        false,
    )) else {
        panic!("a refused enumeration opened a context");
    };
    assert!(refusing.served());
    assert_eq!(error.kind(), GpuErrorKind::AdapterUnavailable);
    assert_eq!(error.to_string(), refused().to_string());
    for propagation in [
        crate::GpuPropagation::Dense,
        crate::GpuPropagation::ChangedDependencies,
    ] {
        let refusing = GpuDiscovery::with_state(METAL, finished());
        let result = crate::GpuFormulaOracle::from_source_with_kernel(
            GpuDeviceSource::Discovered {
                discovery: &refusing,
                options: GpuOptions::default(),
                timing: true,
            },
            crate::GpuKernelOptions {
                propagation,
                ..crate::GpuKernelOptions::default()
            },
        );
        let error = result
            .err()
            .expect("refused discovery cannot create a kernel");
        assert!(refusing.served());
        assert_eq!(error.kind(), GpuErrorKind::AdapterUnavailable);
        assert_eq!(error.to_string(), refused().to_string());
    }
}

#[test]
fn dropping_an_unconsumed_discovery_joins_its_thread() {
    let (sender, receiver) = std::sync::mpsc::channel();
    let pending = GpuDiscovery::with_state(
        METAL,
        State::Running(std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            sender.send(()).unwrap();
            Err(refused())
        })),
    );
    drop(pending);
    // The drop returned only after the thread finished its work.
    assert!(receiver.try_recv().is_ok());
}

/// One solve-path primitive's constructor, reporting the adapter it chose.
type Construct = fn(GpuDeviceSource<'_>) -> crate::GpuInfo;

// Each solve-path primitive constructed from a discovery selects over its
// prefetched enumeration and chooses the adapter its own independent
// constructor chooses, each under its own capability check.
fn discovered_adapter(api: GpuApi) {
    let selection = GpuSelection { api };
    let options = GpuOptions::default();
    let primitives: [(Construct, bool); 3] = [
        (
            |source| {
                let oracle = crate::GpuFormulaOracle::from_source(source).unwrap();
                assert_eq!(
                    oracle.compiled_profile().kernel_options(),
                    crate::GpuKernelOptions::default()
                );
                oracle.info().clone()
            },
            false,
        ),
        (
            |source| {
                crate::GpuTightOracle::from_source(source)
                    .unwrap()
                    .info()
                    .clone()
            },
            false,
        ),
        (explicit_formula_adapter, true),
    ];
    for (construct, timing) in primitives {
        let independent = construct(GpuDeviceSource::Selected {
            options,
            selection,
            timing,
        });
        let discovery = GpuDiscovery::spawn(selection);
        let discovered = construct(GpuDeviceSource::Discovered {
            discovery: &discovery,
            options,
            timing,
        });
        assert!(discovery.served());
        assert_eq!(discovered, independent);
    }
}

// Exercise the source-aware options through both selection and prefetched
// discovery. The requested timestamps belong to the exact opened context.
fn explicit_formula_adapter(source: GpuDeviceSource<'_>) -> crate::GpuInfo {
    let options = crate::GpuKernelOptions {
        propagation: crate::GpuPropagation::ChangedDependencies,
        ..crate::GpuKernelOptions::default()
    };
    let oracle = crate::GpuFormulaOracle::from_source_with_kernel(source, options).unwrap();
    assert_eq!(oracle.compiled_profile().kernel_options(), options);
    assert_eq!(oracle.node_kernel(), crate::BinaryNodeKernel::default());
    assert_eq!(
        oracle.context().compute_timing(),
        if oracle
            .context()
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            zetesis_backend::ComputeTimingStatus::Enabled
        } else {
            zetesis_backend::ComputeTimingStatus::Unsupported
        }
    );
    oracle.info().clone()
}

#[test]
#[ignore = "requires Metal: discovered primitives choose the independent adapter"]
fn metal_discovered_primitives_choose_the_independent_adapter() {
    discovered_adapter(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: discovered primitives choose the independent adapter"]
fn vulkan_discovered_primitives_choose_the_independent_adapter() {
    discovered_adapter(GpuApi::Vulkan);
}
