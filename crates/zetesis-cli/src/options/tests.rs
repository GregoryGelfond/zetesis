//! The default memory allowance and the host readings behind it.

use super::{allowance, memory_total};

#[test]
fn the_default_allowance_is_half_of_the_available_memory() {
    assert_eq!(allowance(Some(48 << 30)), 24 << 30);
}

#[test]
fn a_small_container_is_given_half_its_limit() {
    // A fixed floor would give a one-gibibyte container more than its limit.
    assert_eq!(allowance(Some(1 << 30)), 1 << 29);
}

#[test]
fn without_a_reading_the_default_is_the_reference() {
    assert_eq!(allowance(None), crate::SolveConfig::REFERENCE_MEMORY);
}

#[test]
fn meminfo_reports_its_total_in_bytes() {
    let meminfo = "MemTotal:       16314508 kB\nMemFree:          123456 kB\n";
    assert_eq!(memory_total(meminfo), Some(16_314_508 * 1024));
}

#[cfg(target_os = "macos")]
#[test]
fn the_kernel_reading_matches_the_system_sysctl() {
    // The system tool reads the same kernel value through its own call.
    let output = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .unwrap();
    let reported: u64 = std::str::from_utf8(&output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(super::read_host_memory(), Some(reported));
}
