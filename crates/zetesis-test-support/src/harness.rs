//! The names the test harness gives a crate's tests, for a test that reruns
//! one of its crate's tests in a child process.

/// The test `function` of the module whose path is `module`, as the test
/// harness names it: the module path below the test crate's root, then the
/// function. A test passes its own `module_path!()`.
#[must_use]
pub fn test_name(module: &str, function: &str) -> String {
    match module.split_once("::") {
        Some((_, below)) => format!("{below}::{function}"),
        None => function.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_is_named_by_its_module_below_the_crate_root() {
        assert_eq!(
            test_name("integration::root_bundles", "resolves"),
            "root_bundles::resolves"
        );
    }

    #[test]
    fn a_test_at_the_crate_root_is_named_by_its_function() {
        assert_eq!(test_name("integration", "resolves"), "resolves");
    }
}
