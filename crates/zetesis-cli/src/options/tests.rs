//! The default memory allowance and the host readings behind it.

use super::{allowance, available_memory, memory_total};
use std::collections::HashMap;
use std::path::Path;

/// A reader over the given files and nothing else, shared with the cgroup
/// tests.
pub(super) fn files<'a>(entries: &'a [(&str, &str)]) -> impl Fn(&Path) -> Option<String> + 'a {
    let map: HashMap<&Path, &str> = entries
        .iter()
        .map(|&(path, text)| (Path::new(path), text))
        .collect();
    move |path| map.get(path).map(|text| (*text).to_owned())
}

const MEMINFO: (&str, &str) = ("/proc/meminfo", "MemTotal:        4194304 kB\n");
const IN_GROUP: (&str, &str) = ("/proc/self/cgroup", "0::/app\n");

#[test]
fn the_default_allowance_is_half_of_the_available_memory() {
    assert_eq!(allowance(Some(48 << 30)), 24 << 30);
}

#[test]
fn a_small_reading_is_halved_without_a_floor() {
    // A fixed two-gibibyte floor would give a one-gibibyte container more than
    // its limit; the container's limit reaches the reading by the tests below.
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

#[test]
fn a_group_limit_below_physical_memory_binds() {
    let read = files(&[
        MEMINFO,
        IN_GROUP,
        ("/sys/fs/cgroup/app/memory.max", "1073741824\n"),
    ]);
    assert_eq!(available_memory(read), Some(1 << 30));
}

#[test]
fn a_version1_unlimited_value_leaves_physical_memory() {
    let read = files(&[
        MEMINFO,
        ("/proc/self/cgroup", "4:memory:/\n"),
        (
            "/sys/fs/cgroup/memory/memory.limit_in_bytes",
            "9223372036854771712\n",
        ),
    ]);
    assert_eq!(available_memory(read), Some(4 << 30));
}

#[test]
fn without_a_group_file_the_reading_is_physical_memory() {
    assert_eq!(available_memory(files(&[MEMINFO])), Some(4 << 30));
}

#[test]
fn without_meminfo_a_group_limit_is_no_reading() {
    let read = files(&[IN_GROUP, ("/sys/fs/cgroup/app/memory.max", "1073741824\n")]);
    assert_eq!(available_memory(read), None);
}
