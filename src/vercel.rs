//! Calls to the Vercel CLI.
//!
//! The CLI changes its output when it detects an AI coding agent (through
//! variables such as `AI_AGENT`): it prints JSON reports instead of text.
//! People and CI see the text form, so both are handled here.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde_json::Value;

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

/// What `vercel project inspect` says about a project.
#[derive(Debug, PartialEq)]
enum ProjectState {
    Missing,
    Exists {
        name: String,
        framework: Option<String>,
    },
    /// The CLI couldn't say, with its message.
    Unknown(String),
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
            (ProjectState::Missing, Some(project)) => {
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
                ProjectState::Exists {
                    name,
                    framework: Some(framework),
                },
                _,
            ) if RUST_PRESETS.contains(&framework.as_str()) => {
                eprintln!(
                    "warning: Vercel project `{name}` uses the \"{framework}\" Framework Preset, so a build \
                     from a connected Git repo would fail. Switch it off with \
                     `vercel project update {name} --framework other`."
                );
            }
            // Without a project name, an unlinked folder is expected to be unknown.
            (ProjectState::Unknown(reason), Some(project)) => {
                eprintln!(
                    "note: couldn't check the Framework Preset of `{project}` ({reason}). \
                     If Vercel set it to \"rust\", switch it off with \
                     `vercel project update {project} --framework other`."
                );
            }
            _ => {}
        }
        Ok(Deployment { created, url })
    }

    fn inspect(&self, dir: &Path, project: Option<&str>) -> ProjectState {
        let mut command = Command::new(&self.program);
        command.args(["project", "inspect"]);
        if let Some(project) = project {
            command.arg(project);
        }
        match command.arg("--json").current_dir(dir).output() {
            Ok(output) => parse_inspect(output.status.success(), &output.stdout, &output.stderr),
            Err(err) => ProjectState::Unknown(err.to_string()),
        }
    }
}

fn parse_inspect(success: bool, stdout: &[u8], stderr: &[u8]) -> ProjectState {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);

    if let Some(report) = first_json(&stdout) {
        if report["reason"] == "project_not_found" {
            return ProjectState::Missing;
        }
        if let (true, Some(name)) = (success, report["name"].as_str()) {
            return ProjectState::Exists {
                name: name.to_owned(),
                framework: report["framework"].as_str().map(str::to_owned),
            };
        }
    }
    // The text form of the same error.
    if !success && stderr.contains("There is no project for") {
        return ProjectState::Missing;
    }

    // Skip notices like `<claude-code-hint ... />` and prefer an `Error:` line.
    let lines: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('<'))
        .collect();
    let reason = lines
        .iter()
        .find(|line| line.starts_with("Error"))
        .or(lines.first())
        .copied()
        .unwrap_or("no output");
    ProjectState::Unknown(reason.to_owned())
}

/// Finds the deployment URL in `vercel deploy` stdout: a JSON report for
/// agents, otherwise just the URL.
fn parse_deploy_url(stdout: &str) -> Option<String> {
    if let Some(url) = first_json(stdout)
        .and_then(|report| report["deployment"]["url"].as_str().map(str::to_owned))
    {
        return Some(url);
    }
    stdout
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("https://") && !line.contains(char::is_whitespace))
        .map(str::to_owned)
}

/// The first JSON value in `text`, ignoring anything printed before it.
fn first_json(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()?
        .ok()
}

#[cfg(test)]
mod tests {
    use super::{ProjectState, parse_deploy_url, parse_inspect};

    #[test]
    fn deploy_url_from_the_agent_json_report() {
        // Trimmed from real `vercel deploy --prebuilt` stdout under an AI agent (CLI 65.7).
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
        // Outside an agent (CLI 65.9 in GitHub Actions), stdout is only the
        // URL, without a trailing newline.
        let stdout = "https://my-site-abc123-team.vercel.app";
        assert_eq!(
            parse_deploy_url(stdout).as_deref(),
            Some("https://my-site-abc123-team.vercel.app")
        );
        assert_eq!(parse_deploy_url("nothing useful here"), None);
    }

    #[test]
    fn existing_project_with_a_preset() {
        // Trimmed from real `vercel project inspect --json` output; this form
        // is the same with or without an agent.
        let json =
            br#"{ "id": "prj_x", "name": "my-site", "owner": { "slug": "team" }, "framework": "rust" }"#;
        assert_eq!(
            parse_inspect(true, json, b""),
            ProjectState::Exists {
                name: "my-site".into(),
                framework: Some("rust".into())
            }
        );
    }

    #[test]
    fn existing_project_without_a_preset() {
        let json = br#"{ "id": "prj_x", "name": "my-site", "framework": null }"#;
        assert_eq!(
            parse_inspect(true, json, b""),
            ProjectState::Exists {
                name: "my-site".into(),
                framework: None
            }
        );
    }

    #[test]
    fn missing_project_as_agent_json() {
        let stdout = br#"{ "status": "error", "reason": "project_not_found", "message": "There is no project" }"#;
        let stderr = b"<claude-code-hint v=\"1\" type=\"plugin\" />\n";
        assert_eq!(parse_inspect(false, stdout, stderr), ProjectState::Missing);
    }

    #[test]
    fn missing_project_as_text() {
        // Real output outside an agent (CLI 65.7 and 65.9): empty stdout, exit code 1.
        let stderr = b"Error: There is no project for \"my-site\"\n";
        assert_eq!(parse_inspect(false, b"", stderr), ProjectState::Missing);
    }

    #[test]
    fn other_errors_are_unknown_with_the_cli_message() {
        let stderr = b"<claude-code-hint v=\"1\" />\nVercel CLI 65.9.0\nError: The specified token is not valid.\n";
        assert_eq!(
            parse_inspect(false, b"", stderr),
            ProjectState::Unknown("Error: The specified token is not valid.".into())
        );
        assert_eq!(
            parse_inspect(
                false,
                br#"{ "status": "error", "reason": "not_linked" }"#,
                b""
            ),
            ProjectState::Unknown(r#"{ "status": "error", "reason": "not_linked" }"#.into())
        );
        assert_eq!(
            parse_inspect(false, b"", b""),
            ProjectState::Unknown("no output".into())
        );
    }
}
