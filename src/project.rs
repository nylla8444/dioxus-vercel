use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// The parts of `cargo metadata` output this tool reads.
#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    target_directory: PathBuf,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    version: String,
    manifest_path: PathBuf,
}

pub struct Project {
    /// Directory holding the package's Cargo.toml. `.vercel/` is written here.
    pub root: PathBuf,
    pub name: String,
    /// Cargo's target directory, which is shared by the whole workspace.
    pub target_dir: PathBuf,
    /// Resolved version of the `dioxus` crate.
    pub dioxus_version: String,
}

impl Project {
    pub fn load(path: &Path, package: Option<&str>) -> Result<Self> {
        let manifest = path.join("Cargo.toml");
        if !manifest.is_file() {
            bail!("no Cargo.toml in {}", path.display());
        }

        let output = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--manifest-path"])
            .arg(&manifest)
            .output()
            .context("failed to run `cargo metadata`")?;
        if !output.status.success() {
            bail!(
                "`cargo metadata` failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let metadata: Metadata = serde_json::from_slice(&output.stdout)
            .context("could not read `cargo metadata` output")?;

        let package = pick_package(&metadata, &manifest, package)?;
        let dioxus_version = metadata
            .packages
            .iter()
            .find(|p| p.name == "dioxus")
            .map(|p| p.version.clone())
            .with_context(|| format!("`{}` doesn't depend on dioxus", package.name))?;

        Ok(Self {
            root: package
                .manifest_path
                .parent()
                .context("manifest path has no parent")?
                .to_path_buf(),
            name: package.name.clone(),
            target_dir: metadata.target_directory.clone(),
            dioxus_version,
        })
    }

    /// Where `dx bundle --web --release` puts the finished site.
    pub fn dx_public_dir(&self) -> PathBuf {
        self.target_dir
            .join("dx")
            .join(&self.name)
            .join("release")
            .join("web")
            .join("public")
    }
}

fn pick_package<'a>(
    metadata: &'a Metadata,
    manifest: &Path,
    wanted: Option<&str>,
) -> Result<&'a Package> {
    let members: Vec<&Package> = metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .collect();

    if let Some(name) = wanted {
        return members
            .into_iter()
            .find(|p| p.name == name)
            .with_context(|| format!("no package named `{name}` in this workspace"));
    }

    // Prefer the package whose Cargo.toml was passed in, so a member's
    // directory works even when it sits inside a larger workspace.
    let manifest = fs::canonicalize(manifest)?;
    if let Some(package) = members
        .iter()
        .find(|p| fs::canonicalize(&p.manifest_path).is_ok_and(|m| m == manifest))
    {
        return Ok(package);
    }
    match members.as_slice() {
        [only] => Ok(only),
        _ => {
            let names: Vec<&str> = members.iter().map(|p| p.name.as_str()).collect();
            bail!(
                "this workspace has several packages; pick one with --package ({})",
                names.join(", ")
            )
        }
    }
}
