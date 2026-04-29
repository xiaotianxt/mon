use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

pub fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

pub fn config_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".mon"))
}

pub fn session_file(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return expand_tilde(path);
    }
    if let Some(path) = std::env::var_os("MON_SESSION_FILE") {
        return expand_tilde(PathBuf::from(path));
    }
    Ok(config_dir()?.join("session.json"))
}

pub fn tracking_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return expand_tilde(path);
    }
    Ok(home_dir()?.join("Desktop").join("rent-tracking"))
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
        let expanded = expand_tilde(PathBuf::from("~/Desktop/rent-tracking")).unwrap();
        assert!(expanded.ends_with("Desktop/rent-tracking"));
        assert!(expanded.is_absolute());
    }
}
