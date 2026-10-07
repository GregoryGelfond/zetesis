//! Resource policy is derived when used, including after configuration changes.

use super::Config;

#[test]
fn changed_memory_reaches_source_session_and_output() {
    let mut config = Config {
        memory: 64 * 1024 * 1024,
        ..Config::default()
    };
    let small_source = config.resources().formula_limits().max_support_bytes;
    let small_session = config.session().max_model_bytes;
    let small_output = config.output().max_atom_bytes;
    config.memory *= 2;
    assert!(config.resources().formula_limits().max_support_bytes > small_source);
    assert!(config.session().max_model_bytes > small_session);
    assert!(config.output().max_atom_bytes > small_output);
}

#[test]
fn ordinary_output_has_no_selected_work_ceiling() {
    let limits = Config::default().output();
    assert_eq!(limits.max_symbol_work, u64::MAX);
    assert_eq!(limits.observation.max_work, u64::MAX);
    assert_eq!(limits.observation.max_bindings, u64::MAX);
}
