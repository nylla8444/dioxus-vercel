//! `init`: sets a project up so Vercel builds it itself whenever the
//! connected Git repo gets a push.

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

#[derive(Debug)]
pub struct VercelJson {
    pub text: String,
    /// Things worth telling the user about the change.
    pub notes: Vec<String>,
}

/// Adds the build settings to `existing` vercel.json text (or a new file),
/// keeping every other setting. Settings someone else wrote are only
/// replaced with `force`.
pub fn vercel_json(
    existing: Option<&str>,
    package: Option<&str>,
    force: bool,
) -> Result<VercelJson> {
    let mut config: Map<String, Value> = match existing {
        Some(text) => {
            serde_json::from_str(text).context("vercel.json isn't a valid JSON object")?
        }
        None => Map::new(),
    };

    let install = format!(
        "cargo install dioxus-vercel --version {} --locked",
        env!("CARGO_PKG_VERSION")
    );
    let build = match package {
        Some(package) => format!("dioxus-vercel build --package {package}"),
        None => "dioxus-vercel build".to_owned(),
    };
    for (key, ours, prefix) in [
        ("installCommand", &install, "cargo install dioxus-vercel"),
        ("buildCommand", &build, "dioxus-vercel build"),
    ] {
        if let Some(current) = config.get(key).and_then(Value::as_str) {
            // An earlier `init` wrote it, possibly for another version.
            let from_init = current.starts_with(prefix);
            if current != ours && !from_init && !force {
                bail!("vercel.json already sets {key} to `{current}`. Pass --force to replace it.");
            }
        }
    }

    // null is Vercel's "Other" preset, so it doesn't build the app as Rust code.
    config.insert("framework".into(), Value::Null);
    config.insert("installCommand".into(), install.into());
    config.insert("buildCommand".into(), build.into());

    let mut notes = Vec::new();
    // init-ci turns Vercel's Git builds off; this route needs them on.
    // shift_remove keeps the other settings in order; remove would swap.
    if let Some(git) = config.get_mut("git").and_then(Value::as_object_mut) {
        if git.get("deploymentEnabled") == Some(&Value::Bool(false)) {
            git.shift_remove("deploymentEnabled");
            notes.push(
                "Removed \"deploymentEnabled\": false, which init-ci adds to stop Vercel's own builds."
                    .to_owned(),
            );
        }
        if git.is_empty() {
            config.shift_remove("git");
        }
    }

    let text = serde_json::to_string_pretty(&Value::Object(config))? + "\n";
    Ok(VercelJson { text, notes })
}

#[cfg(test)]
mod tests {
    use super::vercel_json;
    use serde_json::{Value, json};

    fn parse(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn writes_a_new_file() {
        let result = vercel_json(None, None, false).unwrap();
        let config = parse(&result.text);

        assert_eq!(config["framework"], Value::Null);
        assert_eq!(
            config["installCommand"],
            format!(
                "cargo install dioxus-vercel --version {} --locked",
                env!("CARGO_PKG_VERSION")
            )
        );
        assert_eq!(config["buildCommand"], "dioxus-vercel build");
        assert!(result.notes.is_empty());
        assert!(result.text.ends_with("}\n"));
    }

    #[test]
    fn passes_the_package_to_build() {
        let result = vercel_json(None, Some("web"), false).unwrap();
        assert_eq!(
            parse(&result.text)["buildCommand"],
            "dioxus-vercel build --package web"
        );
    }

    #[test]
    fn keeps_other_settings_and_their_order() {
        let existing =
            r#"{ "redirects": [{ "source": "/old", "destination": "/" }], "cleanUrls": true }"#;
        let result = vercel_json(Some(existing), None, false).unwrap();
        let config = parse(&result.text);

        assert_eq!(config["redirects"][0]["source"], "/old");
        assert_eq!(config["cleanUrls"], true);
        assert!(result.text.find("redirects").unwrap() < result.text.find("buildCommand").unwrap());
    }

    #[test]
    fn turns_vercel_builds_back_on_after_init_ci() {
        let existing = json!({ "git": { "deploymentEnabled": false } }).to_string();
        let result = vercel_json(Some(&existing), None, false).unwrap();

        assert!(parse(&result.text).get("git").is_none());
        assert_eq!(result.notes.len(), 1);
    }

    #[test]
    fn removing_the_git_setting_keeps_the_order() {
        let existing =
            json!({ "redirects": [], "git": { "deploymentEnabled": false } }).to_string();
        let text = vercel_json(Some(&existing), None, false).unwrap().text;
        let keys: Vec<String> = parse(&text).as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            ["redirects", "framework", "installCommand", "buildCommand"]
        );
    }

    #[test]
    fn refuses_to_replace_someone_elses_build_command() {
        let existing = r#"{ "buildCommand": "npm run build" }"#;
        let err = vercel_json(Some(existing), None, false).unwrap_err();
        assert!(err.to_string().contains("--force"));

        let forced = vercel_json(Some(existing), None, true).unwrap();
        assert_eq!(parse(&forced.text)["buildCommand"], "dioxus-vercel build");
    }

    #[test]
    fn updates_its_own_older_settings_without_force() {
        let existing =
            r#"{ "installCommand": "cargo install dioxus-vercel --version 0.0.1 --locked" }"#;
        let result = vercel_json(Some(existing), None, false).unwrap();
        assert!(
            parse(&result.text)["installCommand"]
                .as_str()
                .unwrap()
                .contains(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(vercel_json(Some("not json"), None, false).is_err());
        assert!(vercel_json(Some("[1, 2]"), None, false).is_err());
    }
}
