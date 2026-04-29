use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

use crate::cli::InstallArgs;
use crate::paths;

pub fn install(args: InstallArgs) -> Result<()> {
    let current = std::env::current_exe().context("failed to locate current executable")?;
    let bin_dir = match args.bin_dir {
        Some(path) => paths::expand_tilde(path)?,
        None => paths::home_dir()?.join(".local").join("bin"),
    };
    fs::create_dir_all(&bin_dir)
        .with_context(|| format!("failed to create {}", bin_dir.display()))?;

    let target: PathBuf = bin_dir.join("mon");
    if target.exists() && !args.force {
        anyhow::bail!(
            "{} already exists; rerun with --force to replace it",
            target.display()
        );
    }

    fs::copy(&current, &target).with_context(|| {
        format!(
            "failed to copy {} to {}",
            current.display(),
            target.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
            .with_context(|| format!("failed to chmod {}", target.display()))?;
    }

    println!("installed: {}", target.display());
    Ok(())
}
