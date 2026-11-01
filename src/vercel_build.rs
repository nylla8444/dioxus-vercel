//! Setup for builds on Vercel's own build machines, the route `init` sets up.
//!
//! Vercel's machines run Amazon Linux 2023 with glibc 2.34 and come with
//! rustup and an older Rust. DioxusLabs' Linux builds of dx need glibc 2.35
//! or newer, so dx comes from this repo's own Amazon Linux builds instead
//! (see .github/workflows/build-dx.yml).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::project::Project;
use crate::tools;

/// Where build-dx.yml publishes dx: one `dx-<version>` release per version.
const DX_RELEASES: &str = "https://github.com/nylla8444/dioxus-vercel/releases/download";

/// Vercel sets `VERCEL=1` during builds; they always run on Linux.
pub fn running_on_vercel() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("VERCEL").is_some_and(|value| value == "1")
}

/// Points Cargo's build output at `node_modules/.cache`, a folder Vercel
/// keeps between builds, so later builds can reuse compiled dependencies.
/// Vercel runs the build command from the project's Root Directory, which
/// is where it looks for `node_modules`. Call this before starting threads.
pub fn use_cache_folder() -> Result<()> {
    if std::env::var_os("CARGO_TARGET_DIR").is_some() {
        return Ok(());
    }
    let dir = std::env::current_dir()?
        .join("node_modules")
        .join(".cache")
        .join("dioxus-vercel")
        .join("target");
    // SAFETY: main calls this first, while the program has only one thread.
    unsafe { std::env::set_var("CARGO_TARGET_DIR", &dir) };
    println!(
        "Keeping Rust's build output in {} so Vercel can cache it",
        dir.display()
    );
    Ok(())
}

/// Gets Rust ready to build WebAssembly and returns a dx that runs here.
pub fn prepare(project: &Project) -> Result<PathBuf> {
    prepare_rust(&project.root)?;
    prepare_dx(
        &project.dioxus_version,
        &project.target_dir.join("dioxus-vercel"),
    )
}

/// Vercel's preinstalled Rust may be older than the project's dependencies
/// need, and it has no WebAssembly target. A rust-toolchain file, if the
/// project has one, decides the toolchain instead.
fn prepare_rust(root: &Path) -> Result<()> {
    let rustup = tools::find("rustup", "Vercel's build machines normally have it.")?;
    if root
        .ancestors()
        .any(|dir| dir.join("rust-toolchain.toml").exists() || dir.join("rust-toolchain").exists())
    {
        return tools::run(&rustup, &["target", "add", "wasm32-unknown-unknown"], root);
    }
    tools::run(
        &rustup,
        &[
            "toolchain",
            "install",
            "stable",
            "--profile",
            "minimal",
            "--target",
            "wasm32-unknown-unknown",
        ],
        root,
    )?;
    tools::run(&rustup, &["default", "stable"], root)
}

/// Downloads dx `version` built for Amazon Linux 2023 into `dir`, checking
/// its checksum. Without a build for that version, compiles dx instead.
fn prepare_dx(version: &str, dir: &Path) -> Result<PathBuf> {
    let install_root = dir.join(format!("dx-{version}"));
    let dx = install_root.join("bin").join("dx");
    if dx.is_file() {
        return Ok(dx);
    }

    let name = format!("dx-{version}-x86_64-amazonlinux2023");
    let archive = format!("{name}.tar.gz");
    let url = format!("{DX_RELEASES}/dx-{version}/{archive}");
    let download = dir.join("download");
    fs::create_dir_all(&download)?;

    println!("Downloading dx {version} built for Vercel's build machines");
    let found = Command::new("curl")
        .args(["-fsSL", "-o", &archive, &url])
        .current_dir(&download)
        .status()
        .context("failed to run curl")?
        .success();
    if !found {
        eprintln!(
            "warning: there's no dx {version} built for Vercel yet, so it will be compiled, \
             which takes 12 to 15 minutes on Vercel. Ask for a build at \
             https://github.com/nylla8444/dioxus-vercel/issues"
        );
        let cargo = tools::find("cargo", "Vercel's build machines normally have it.")?;
        let root = install_root.to_string_lossy();
        tools::run(
            &cargo,
            &[
                "install",
                "dioxus-cli",
                "--version",
                version,
                "--locked",
                "--root",
                &root,
            ],
            dir,
        )?;
        return Ok(dx);
    }

    let checksum = format!("{archive}.sha256");
    let sh = |script: &str| -> Result<()> {
        let status = Command::new("sh")
            .args(["-c", script])
            .current_dir(&download)
            .status()
            .context("failed to run sh")?;
        if !status.success() {
            bail!("`{script}` failed ({status})");
        }
        Ok(())
    };
    sh(&format!("curl -fsSL -o '{checksum}' '{url}.sha256'"))?;
    sh(&format!("sha256sum -c '{checksum}'"))?;
    sh(&format!("tar -xzf '{archive}'"))?;

    fs::create_dir_all(install_root.join("bin"))?;
    fs::rename(download.join(&name).join("dx"), &dx)
        .with_context(|| format!("could not move dx into {}", dx.display()))?;
    Ok(dx)
}
