use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

pub fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

pub fn expand_tilde(path: PathBuf) -> Result<PathBuf> {
    let raw = path.to_string_lossy();
    if raw == "~" {
        return home_dir();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_relative_paths() {
        let expanded = expand_tilde(PathBuf::from("~/Library/Application Support/mon")).unwrap();
        assert!(expanded.ends_with("Library/Application Support/mon"));
        assert!(expanded.is_absolute());
    }
}
