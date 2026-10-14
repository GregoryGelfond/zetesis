//! Bounded declared closure and actual cwd-first include resolution.

use std::path::{Component, Path, PathBuf};

use super::{BundleFile, Error, examples};

pub(super) fn validate(entry: &str, files: &[BundleFile]) -> Result<(), Error> {
    examples::Files::relative(entry).map_err(Error::Corpus)?;
    if files.is_empty() || files.len() > 256 {
        return Err(Error::Configuration(
            "bundle requires 1..=256 source members",
        ));
    }
    let mut edges = 0usize;
    for (position, file) in files.iter().enumerate() {
        examples::Files::relative(&file.path).map_err(Error::Corpus)?;
        if files[..position]
            .iter()
            .any(|prior| prior.path == file.path)
            || file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::Configuration(
                "bundle member is repeated or its digest is invalid",
            ));
        }
        edges = super::super::add(edges, file.includes.len())?;
        if edges > 1024 {
            return Err(Error::Configuration(
                "bundle exceeds 1024 include occurrences",
            ));
        }
        for include in &file.includes {
            examples::Files::relative(&include.target).map_err(Error::Corpus)?;
            if target(&file.path, &include.spelling)? != Path::new(&include.target) {
                return Err(Error::Configuration(
                    "bundle include differs from its declared relative target",
                ));
            }
        }
    }
    let mut active = Vec::new();
    let mut heights = vec![0; files.len()];
    visit(entry, files, &mut active, &mut heights)?;
    if heights.contains(&0) {
        return Err(Error::Configuration(
            "bundle has unreachable source members",
        ));
    }
    Ok(())
}

// Every recursive call consumes one of 32 path positions; active membership
// forbids cycles. Repeated edges still resolve, while completed DAG branches
// reuse their established subtree height, preserving the longest-path bound.
fn visit(
    name: &str,
    files: &[BundleFile],
    active: &mut Vec<usize>,
    heights: &mut [usize],
) -> Result<usize, Error> {
    let index = files
        .iter()
        .position(|file| file.path == name)
        .ok_or(Error::Configuration(
            "bundle target is not a declared member",
        ))?;
    if active.contains(&index) {
        return Err(Error::Configuration("bundle include cycle"));
    }
    if active.len() >= 32 {
        return Err(Error::Configuration("bundle include depth exceeds 32"));
    }
    if heights[index] > 0 {
        return Ok(heights[index]);
    }
    active.push(index);
    let mut height = 1;
    for include in &files[index].includes {
        height = height.max(1 + visit(&include.target, files, active, heights)?);
    }
    active.pop();
    if height > 32 {
        return Err(Error::Configuration("bundle include depth exceeds 32"));
    }
    heights[index] = height;
    Ok(height)
}

fn target(including: &str, spelling: &str) -> Result<PathBuf, Error> {
    let path = Path::new(spelling);
    if spelling.is_empty()
        || spelling.contains(['\\', ':'])
        || spelling.split('/').any(str::is_empty)
    {
        return Err(Error::Configuration(
            "bundle include spelling is not portable",
        ));
    }
    let mut target = Path::new(including)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();
    for part in path.components() {
        match part {
            Component::Normal(value) => target.push(value),
            Component::CurDir => {}
            Component::ParentDir if target.pop() => {}
            _ => return Err(Error::Configuration("bundle include escapes its root")),
        }
    }
    Ok(target)
}

pub(super) fn resolve(root: &Path, files: &[BundleFile]) -> Result<(), Error> {
    let root = examples::Files::canonical(root).map_err(Error::Corpus)?;
    for file in files {
        let including = root.join(&file.path);
        let parent = including
            .parent()
            .ok_or(Error::Configuration("bundle member has no parent"))?;
        for include in &file.includes {
            let expected =
                examples::Files::confined(&root, &include.target).map_err(Error::Corpus)?;
            let local = examples::Files::canonical(&parent.join(&include.spelling))
                .map_err(Error::Corpus)?;
            if local != expected {
                return Err(Error::Configuration(
                    "bundle include aliases an undeclared target",
                ));
            }
            let cwd = root.join(&include.spelling);
            match cwd.canonicalize() {
                Ok(actual) if actual != expected => {
                    return Err(Error::Configuration(
                        "cwd include shadows the declared bundle target",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(crate::performance::io(&cwd, error)),
            }
        }
    }
    Ok(())
}
