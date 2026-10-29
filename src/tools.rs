use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Finds a program on PATH. `which` also finds `.cmd` shims on Windows,
/// which is how npm installs the Vercel CLI.
pub fn find(program: &str, install_hint: &str) -> Result<PathBuf> {
    which::which(program).with_context(|| format!("`{program}` not found on PATH. {install_hint}"))
}

/// dx only works with the dioxus version it was released with, so a
/// mismatch is the usual cause of confusing bundle errors.
pub fn warn_on_dx_mismatch(dx: &Path, dioxus_version: &str) -> Result<()> {
    let output = Command::new(dx)
        .arg("--version")
        .output()
        .context("failed to run `dx --version`")?;
    // Prints e.g. "dioxus 0.7.10 (57d6794)".
    let text = String::from_utf8_lossy(&output.stdout);
    let dx_version = text.split_whitespace().nth(1).unwrap_or_default();
    if !same_minor(dx_version, dioxus_version) {
        eprintln!(
            "warning: dx is {dx_version} but the project uses dioxus {dioxus_version}. \
             Install the matching CLI with `cargo install dioxus-cli --version {dioxus_version} --locked`."
        );
    }
    Ok(())
}

pub fn dx_bundle(dx: &Path, root: &Path, package: Option<&str>) -> Result<()> {
    let mut args = vec!["bundle", "--web", "--release"];
    if let Some(package) = package {
        args.extend(["--package", package]);
    }
    run(dx, &args, root)
}

pub fn run(program: &Path, args: &[&str], dir: &Path) -> Result<()> {
    let name = program.file_stem().unwrap_or_default().to_string_lossy();
    println!("> {name} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("failed to start `{name}`"))?;
    if !status.success() {
        bail!("`{name} {}` failed ({status})", args.join(" "));
    }
    Ok(())
}

/// Compares versions by major.minor, which is the breaking part for 0.x crates.
fn same_minor(a: &str, b: &str) -> bool {
    !a.is_empty() && a.split('.').take(2).eq(b.split('.').take(2))
}

#[cfg(test)]
mod tests {
    use super::same_minor;

    #[test]
    fn matching_minor_versions() {
        assert!(same_minor("0.7.10", "0.7.10"));
        assert!(same_minor("0.7.3", "0.7.10"));
        assert!(same_minor("0.8.0-alpha.3", "0.8.1"));
    }

    #[test]
    fn mismatched_or_unknown_versions() {
        assert!(!same_minor("0.6.3", "0.7.10"));
        assert!(!same_minor("0.8.0-alpha.3", "0.7.10"));
        assert!(!same_minor("", "0.7.10"));
    }
}
