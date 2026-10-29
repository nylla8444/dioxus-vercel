//! Writes a site in Vercel's Build Output API (v3) format:
//! https://vercel.com/docs/build-output-api

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::json;

/// dx names every asset it processes `<name>-dxh<hash>.<ext>`, so a file
/// matching this never changes and can be cached forever. Unhashed files,
/// such as those copied from the project's `public/` folder, don't match.
const HASHED_ASSET: &str = r"^/assets/.+-dxh[0-9a-f]+\.[A-Za-z0-9]+$";

pub struct Options {
    /// Serve index.html for paths that aren't files, for client-side routing.
    pub fallback: bool,
}

#[derive(Debug)]
pub struct Stats {
    pub files: usize,
    pub bytes: u64,
}

/// Replaces `out_dir` with the site in `public_dir` plus a `config.json`.
pub fn write(public_dir: &Path, out_dir: &Path, options: &Options) -> Result<Stats> {
    if !public_dir.join("index.html").is_file() {
        bail!(
            "no index.html in {}. Did `dx bundle --web --release` succeed?",
            public_dir.display()
        );
    }
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)
            .with_context(|| format!("could not clear {}", out_dir.display()))?;
    }

    let mut stats = Stats { files: 0, bytes: 0 };
    copy_dir(public_dir, &out_dir.join("static"), &mut stats)?;
    fs::write(out_dir.join("config.json"), config_json(options))?;
    Ok(stats)
}

fn config_json(options: &Options) -> String {
    let mut routes = vec![
        json!({
            "src": HASHED_ASSET,
            "headers": { "cache-control": "public, max-age=31536000, immutable" },
            "continue": true
        }),
        json!({ "handle": "filesystem" }),
    ];
    if options.fallback {
        routes.push(json!({ "src": "/(.*)", "dest": "/index.html" }));
    }
    let config = json!({ "version": 3, "routes": routes });
    serde_json::to_string_pretty(&config).expect("config is always valid JSON")
}

fn copy_dir(from: &Path, to: &Path, stats: &mut Stats) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest, stats)?;
        } else {
            stats.bytes += fs::copy(entry.path(), &dest)?;
            stats.files += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_asset_pattern_matches_only_dx_hashed_files() {
        let pattern = regex::Regex::new(HASHED_ASSET).unwrap();
        // Names taken from a real `dx bundle` of the test site.
        for hashed in [
            "/assets/main-dxhf4c8ffa68b6c80c7.css",
            "/assets/test-site_bg-dxh2fffb76feaa85f75.wasm",
            "/assets/test-site-dxhf65d85c53382285.js",
        ] {
            assert!(pattern.is_match(hashed), "{hashed} should be cached");
        }
        for unhashed in ["/assets/logo.png", "/index.html", "/favicon-dxh12.ico"] {
            assert!(
                !pattern.is_match(unhashed),
                "{unhashed} should not be cached"
            );
        }
    }

    #[test]
    fn config_falls_back_to_index_only_when_asked() {
        let with: serde_json::Value =
            serde_json::from_str(&config_json(&Options { fallback: true })).unwrap();
        let without: serde_json::Value =
            serde_json::from_str(&config_json(&Options { fallback: false })).unwrap();

        assert_eq!(with["version"], 3);
        assert_eq!(with["routes"][1]["handle"], "filesystem");
        assert_eq!(with["routes"][2]["dest"], "/index.html");
        assert_eq!(without["routes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn write_copies_the_site_and_replaces_old_output() {
        let tmp = std::env::temp_dir().join(format!("dioxus-vercel-test-{}", std::process::id()));
        let public = tmp.join("public");
        let out = tmp.join("output");
        fs::create_dir_all(public.join("assets")).unwrap();
        fs::write(public.join("index.html"), "<html></html>").unwrap();
        fs::write(public.join("assets").join("app-dxh00ff.js"), "js").unwrap();
        fs::create_dir_all(out.join("static")).unwrap();
        fs::write(out.join("static").join("stale.txt"), "old").unwrap();

        let stats = write(&public, &out, &Options { fallback: true }).unwrap();

        assert_eq!(stats.files, 2);
        assert!(out.join("static").join("index.html").is_file());
        assert!(
            out.join("static")
                .join("assets")
                .join("app-dxh00ff.js")
                .is_file()
        );
        assert!(!out.join("static").join("stale.txt").exists());
        assert!(out.join("config.json").is_file());
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn write_refuses_a_folder_without_index_html() {
        let tmp = std::env::temp_dir().join(format!("dioxus-vercel-empty-{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        let err = write(&tmp, &tmp.join("output"), &Options { fallback: true }).unwrap_err();
        assert!(err.to_string().contains("no index.html"));
        fs::remove_dir_all(&tmp).unwrap();
    }
}
