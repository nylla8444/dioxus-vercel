//! Calls to the Vercel CLI.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;

use crate::tools;

/// Presets Vercel picks when it sees Rust code. With one of these, a build
/// started from a connected Git repo tries to run the app as a server
/// function and fails.
const RUST_PRESETS: [&str; 3] = ["rust", "axum", "actix-web"];

pub struct Vercel {
    program: PathBuf,
}

pub struct Deployment {
    /// Whether this deploy created the project. Vercel always makes a new
    /// project's first deployment the production one, even without `--prod`.
    pub created: bool,
    /// The deployment's own URL, when it could be read from Vercel's output.
    pub url: Option<String>,
}

/// What `vercel project inspect --json` says about a project.
#[derive(Debug, PartialEq)]
enum ProjectState {
    Missing,
    Exists {
        name: String,
        framework: Option<String>,
    },
}

impl Vercel {
    pub fn find() -> Result<Self> {
        let program = tools::find(
            "vercel",
            "Install it with `npm i -g vercel`, then run `vercel login`.",
        )?;
        Ok(Self { program })
    }

    /// The CLI's major version, such as `65`.
    pub fn major_version(&self) -> Option<String> {
        let output = Command::new(&self.program).arg("--version").output().ok()?;
        // Prints just the version, e.g. "65.7.0", to stdout.
        let version = String::from_utf8_lossy(&output.stdout);
        let major = version.trim().split('.').next()?;
        (!major.is_empty() && major.bytes().all(|b| b.is_ascii_digit())).then(|| major.to_owned())
    }

    /// Uploads `dir/.vercel/output`. With `project`, deploys to that project
    /// and creates it if needed. Without it, the folder's `vercel link` or
    /// Vercel's own prompt picks the project.
    pub fn deploy(&self, dir: &Path, prod: bool, project: Option<&str>) -> Result<Deployment> {
        let before = self.inspect(dir, project);
        let mut created = false;

        let mut args = vec!["deploy", "--prebuilt"];
        if prod {
            args.push("--prod");
        }
        // With a project named, `--yes` stops every prompt, so stdout can be
        // captured to find the URL. Without one, Vercel asks questions there.
        let url = if let Some(project) = project {
            args.extend(["--yes", "--project", project, "--create-if-missing"]);
            parse_deploy_url(&tools::run_capturing(&self.program, &args, dir)?)
        } else {
            tools::run(&self.program, &args, dir)?;
            None
        };

        match (before, project) {
            (Some(ProjectState::Missing), Some(project)) => {
                created = true;
                tools::run(
                    &self.program,
                    &[
                        "project",
                        "update",
                        project,
                        "--framework",
                        "other",
                        "--yes",
                    ],
                    dir,
                )?;
                println!(
                    "Set the new project's Framework Preset to Other, so Vercel never tries to build it from Git."
                );
            }
            (
                Some(ProjectState::Exists {
                    name,
                    framework: Some(framework),
                }),
                _,
            ) if RUST_PRESETS.contains(&framework.as_str()) => {
                eprintln!(
                    "warning: Vercel project `{name}` uses the \"{framework}\" Framework Preset, so a build \
                     from a connected Git repo would fail. Switch it off with \
                     `vercel project update {name} --framework other`."
                );
            }
            _ => {}
        }
        Ok(Deployment { created, url })
    }

    /// Returns `None` when Vercel can't say, for example when no project
    /// was named and the folder isn't linked.
    fn inspect(&self, dir: &Path, project: Option<&str>) -> Option<ProjectState> {
        let mut command = Command::new(&self.program);
        command.args(["project", "inspect"]);
        if let Some(project) = project {
            command.arg(project);
        }
        let output = command.arg("--json").current_dir(dir).output().ok()?;
        parse_inspect(&output.stdout)
    }
}

fn parse_inspect(stdout: &[u8]) -> Option<ProjectState> {
    #[derive(Deserialize)]
    struct Inspect {
        name: Option<String>,
        framework: Option<String>,
        reason: Option<String>,
    }

    let inspect: Inspect = serde_json::from_slice(stdout).ok()?;
    if inspect.reason.as_deref() == Some("project_not_found") {
        return Some(ProjectState::Missing);
    }
    Some(ProjectState::Exists {
        name: inspect.name?,
        framework: inspect.framework,
    })
}

/// Finds the deployment URL in `vercel deploy` stdout. When stdout isn't a
/// terminal, the CLI prints a JSON report there; older versions print just
/// the URL.
fn parse_deploy_url(stdout: &str) -> Option<String> {
    if let Some(start) = stdout.find('{') {
        let mut values =
            serde_json::Deserializer::from_str(&stdout[start..]).into_iter::<serde_json::Value>();
        if let Some(Ok(report)) = values.next() {
            if let Some(url) = report["deployment"]["url"].as_str() {
                return Some(url.to_owned());
            }
        }
    }
    stdout
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("https://") && !line.contains(char::is_whitespace))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{ProjectState, parse_deploy_url, parse_inspect};

    #[test]
    fn deploy_url_from_the_json_report() {
        // Trimmed from real `vercel deploy --prebuilt` stdout (CLI 65.7).
        let stdout = r#"{
  "status": "ok",
  "deployment": {
    "id": "dpl_x",
    "url": "https://my-site-abc123-team.vercel.app",
    "productionUrl": null
  }
}"#;
        assert_eq!(
            parse_deploy_url(stdout).as_deref(),
            Some("https://my-site-abc123-team.vercel.app")
        );
    }

    #[test]
    fn deploy_url_from_plain_output() {
        let stdout = "Some log line\nhttps://my-site-abc123-team.vercel.app\n";
        assert_eq!(
            parse_deploy_url(stdout).as_deref(),
            Some("https://my-site-abc123-team.vercel.app")
        );
        assert_eq!(parse_deploy_url("nothing useful here"), None);
    }

    #[test]
    fn existing_project_with_a_preset() {
        // Trimmed from real `vercel project inspect --json` output (CLI 65.7).
        let json =
            br#"{ "id": "prj_x", "name": "my-site", "framework": "rust", "buildCommand": null }"#;
        assert_eq!(
            parse_inspect(json),
            Some(ProjectState::Exists {
                name: "my-site".into(),
                framework: Some("rust".into())
            })
        );
    }

    #[test]
    fn existing_project_without_a_preset() {
        let json = br#"{ "id": "prj_x", "name": "my-site", "framework": null }"#;
        assert_eq!(
            parse_inspect(json),
            Some(ProjectState::Exists {
                name: "my-site".into(),
                framework: None
            })
        );
    }

    #[test]
    fn missing_project() {
        let json = br#"{ "status": "error", "reason": "project_not_found", "message": "There is no project" }"#;
        assert_eq!(parse_inspect(json), Some(ProjectState::Missing));
    }

    #[test]
    fn other_errors_and_garbage_are_unknown() {
        assert_eq!(
            parse_inspect(br#"{ "status": "error", "reason": "not_linked" }"#),
            None
        );
        assert_eq!(parse_inspect(b"Error: something went wrong"), None);
    }
}
