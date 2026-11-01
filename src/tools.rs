use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Finds a program on PATH. `which` also finds `.cmd` shims on Windows,
/// which is how npm installs the Vercel CLI.
pub fn find(program: &str, install_hint: &str) -> Result<PathBuf> {
    which::which(program).with_context(|| format!("`{program}` not found on PATH. {install_hint}"))
}

/// dx only works with the dioxus version it was released with, so a
/// mismatch is the usual cause of confusing bundle errors. Fails if dx can't
/// run at all, for example when it needs a newer glibc than the system has.
pub fn warn_on_dx_mismatch(dx: &Path, dioxus_version: &str) -> Result<()> {
    let output = Command::new(dx)
        .arg("--version")
        .output()
        .context("failed to run `dx --version`")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(dx_version) = output
        .status
        .success()
        .then(|| dx_version(&stdout))
        .flatten()
    else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "dx is installed but can't run:\n{}\nIf it mentions GLIBC, this dx was built for a \
             newer Linux; build it here with `cargo install dioxus-cli --version {dioxus_version} --locked`.",
            stderr.trim()
        );
    };
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

/// Like [`run`], but also returns what the program printed to stdout.
/// Stdin is closed, so only use this for commands that never prompt.
pub fn run_capturing(program: &Path, args: &[&str], dir: &Path) -> Result<String> {
    let name = program.file_stem().unwrap_or_default().to_string_lossy();
    println!("> {name} {}", args.join(" "));
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to start `{name}`"))?;
    io::stdout().write_all(&output.stdout)?;
    // The Vercel CLI prints a bare URL without a newline; end the line so
    // the next message doesn't run into it.
    if !output.stdout.is_empty() && !output.stdout.ends_with(b"\n") {
        println!();
    }
    if !output.status.success() {
        bail!("`{name} {}` failed ({})", args.join(" "), output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Compares versions by major.minor, which is the breaking part for 0.x crates.
fn same_minor(a: &str, b: &str) -> bool {
    !a.is_empty() && a.split('.').take(2).eq(b.split('.').take(2))
}

/// The version from `dx --version`, which prints e.g. "dioxus 0.7.10 (57d6794)".
fn dx_version(stdout: &str) -> Option<&str> {
    let mut words = stdout.split_whitespace();
    (words.next()? == "dioxus").then_some(())?;
    words.next()
}

#[cfg(test)]
mod tests {
    use super::{dx_version, same_minor};

    #[test]
    fn reads_the_dx_version() {
        assert_eq!(dx_version("dioxus 0.7.10 (57d6794)\n"), Some("0.7.10"));
        assert_eq!(dx_version(""), None);
        assert_eq!(dx_version("something else 1.0"), None);
    }

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
