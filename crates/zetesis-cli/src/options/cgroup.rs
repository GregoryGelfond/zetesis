//! The memory limit a Linux control group sets for the process, read from the
//! cgroup filesystem at its standard mount point: version 2's `memory.max`, or
//! the version-1 memory controller's `memory.limit_in_bytes`, at the process's
//! group and at each ancestor up to the mount's root, the least of them binding.
//! Inside a container the root is the container's own group, so its limit is
//! read there.

use std::path::{Component, Path};

/// The mount point of the unified version-2 hierarchy.
const UNIFIED: &str = "/sys/fs/cgroup";
/// The mount point of the version-1 memory controller.
const MEMORY_CONTROLLER: &str = "/sys/fs/cgroup/memory";

/// The least memory limit set on the process's control group or an ancestor,
/// from the text of `/proc/self/cgroup`, where `read` returns a file's text or
/// none when it cannot be read. None when no group on the path sets a limit.
/// A version-1 memory controller takes precedence over the unified hierarchy:
/// in a hybrid layout it is the controller that accounts the process's memory.
pub(super) fn memory_limit(groups: &str, read: impl Fn(&Path) -> Option<String>) -> Option<u64> {
    if let Some(group) = groups.lines().find_map(memory_controller_group) {
        return least_limit(
            Path::new(MEMORY_CONTROLLER),
            group,
            "memory.limit_in_bytes",
            &read,
        );
    }
    let group = groups.lines().find_map(|line| line.strip_prefix("0::"))?;
    least_limit(Path::new(UNIFIED), group, "memory.max", &read)
}

/// The group of a version-1 line, `ID:CONTROLLERS:GROUP`, whose controllers
/// include memory.
fn memory_controller_group(line: &str) -> Option<&str> {
    let mut fields = line.splitn(3, ':');
    let (_, controllers, group) = (fields.next()?, fields.next()?, fields.next()?);
    controllers
        .split(',')
        .any(|controller| controller == "memory")
        .then_some(group)
}

/// The least limit that `file` states at `group` under `root` and at each of
/// its ancestors up to `root` itself. A value that is not a number, such as
/// version 2's `max`, sets no limit; version 1's "unlimited" is a number past
/// any physical memory, so the caller's minimum leaves it no effect. A group
/// path that would leave `root` is not read.
fn least_limit(
    root: &Path,
    group: &str,
    file: &str,
    read: &impl Fn(&Path) -> Option<String>,
) -> Option<u64> {
    let relative = Path::new(group.trim_start_matches('/'));
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    let mut directory = root.join(relative);
    let mut least: Option<u64> = None;
    loop {
        let limit = read(&directory.join(file)).and_then(|text| text.trim().parse::<u64>().ok());
        least = match (least, limit) {
            (Some(least), Some(limit)) => Some(least.min(limit)),
            (least, limit) => least.or(limit),
        };
        if directory == root {
            return least;
        }
        directory.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::memory_limit;
    use std::collections::HashMap;
    use std::path::Path;

    /// A reader over the given files and nothing else.
    fn files<'a>(entries: &'a [(&str, &str)]) -> impl Fn(&Path) -> Option<String> + 'a {
        let map: HashMap<&Path, &str> = entries
            .iter()
            .map(|&(path, text)| (Path::new(path), text))
            .collect();
        move |path| map.get(path).map(|text| (*text).to_owned())
    }

    #[test]
    fn a_unified_group_limit_is_read_at_the_process_group() {
        let read = files(&[("/sys/fs/cgroup/app/memory.max", "1073741824\n")]);
        assert_eq!(memory_limit("0::/app\n", read), Some(1 << 30));
    }

    #[test]
    fn the_least_limit_on_the_path_to_the_root_binds() {
        let read = files(&[
            ("/sys/fs/cgroup/a/b/memory.max", "max\n"),
            ("/sys/fs/cgroup/a/memory.max", "536870912\n"),
            ("/sys/fs/cgroup/memory.max", "1073741824\n"),
        ]);
        assert_eq!(memory_limit("0::/a/b\n", read), Some(1 << 29));
    }

    #[test]
    fn a_namespaced_container_reads_its_own_root() {
        let read = files(&[("/sys/fs/cgroup/memory.max", "268435456\n")]);
        assert_eq!(memory_limit("0::/\n", read), Some(1 << 28));
    }

    #[test]
    fn max_everywhere_sets_no_limit() {
        let read = files(&[
            ("/sys/fs/cgroup/app/memory.max", "max\n"),
            ("/sys/fs/cgroup/memory.max", "max\n"),
        ]);
        assert_eq!(memory_limit("0::/app\n", read), None);
    }

    #[test]
    fn a_version1_memory_controller_takes_precedence() {
        let groups = "12:cpu,cpuacct:/docker/x\n4:memory:/docker/x\n0::/docker/x\n";
        let read = files(&[
            (
                "/sys/fs/cgroup/memory/docker/x/memory.limit_in_bytes",
                "2147483648\n",
            ),
            ("/sys/fs/cgroup/docker/x/memory.max", "1073741824\n"),
        ]);
        assert_eq!(memory_limit(groups, read), Some(1 << 31));
    }

    #[test]
    fn a_group_path_leaving_its_root_is_not_read() {
        let read = files(&[("/sys/fs/memory.max", "1\n")]);
        assert_eq!(memory_limit("0::/../memory.max\n", read), None);
    }

    #[test]
    fn without_a_group_line_there_is_no_limit() {
        assert_eq!(memory_limit("", files(&[])), None);
    }
}
