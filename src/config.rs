//! Writes dioxus-vercel settings into `[package.metadata.dioxus-vercel]`.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, Item, Table, value};

/// Saves the Vercel project name to the package's Cargo.toml, keeping the
/// rest of the file (formatting, comments, order) as it was.
pub fn save_project(manifest: &Path, project: &str) -> Result<()> {
    let text = fs::read_to_string(manifest)
        .with_context(|| format!("could not read {}", manifest.display()))?;
    let updated = with_project(&text, project)?;
    if updated != text {
        fs::write(manifest, updated)
            .with_context(|| format!("could not write {}", manifest.display()))?;
    }
    Ok(())
}

fn with_project(manifest: &str, project: &str) -> Result<String> {
    let mut doc: DocumentMut = manifest.parse().context("could not parse Cargo.toml")?;
    let package = doc
        .get_mut("package")
        .and_then(Item::as_table_mut)
        .context("Cargo.toml has no [package] table")?;
    // An implicit table has no header of its own, so this adds a single
    // `[package.metadata.dioxus-vercel]` section instead of an empty
    // `[package.metadata]` one above it.
    let metadata = package
        .entry("metadata")
        .or_insert(implicit_table())
        .as_table_mut()
        .context("[package.metadata] in Cargo.toml is not a table")?;
    let settings = metadata
        .entry("dioxus-vercel")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .context("[package.metadata.dioxus-vercel] in Cargo.toml is not a table")?;
    settings["project"] = value(project);

    let updated = doc.to_string();
    // toml_edit writes the lines it adds with \n, so a CRLF file would end up
    // with mixed line endings.
    if manifest.contains("\r\n") {
        return Ok(updated.replace("\r\n", "\n").replace('\n', "\r\n"));
    }
    Ok(updated)
}

fn implicit_table() -> Item {
    let mut table = Table::new();
    table.set_implicit(true);
    Item::Table(table)
}

#[cfg(test)]
mod tests {
    use super::with_project;

    const MANIFEST: &str = r#"[package]
name = "my-site"
version = "0.1.0"

[dependencies]
# pinned to match dx
dioxus = "=0.7.10"
"#;

    #[test]
    fn adds_one_section_and_keeps_the_rest() {
        let updated = with_project(MANIFEST, "my-site").unwrap();

        // toml_edit puts the new section right after [package].
        let section = "[package.metadata.dioxus-vercel]\nproject = \"my-site\"\n\n";
        assert!(updated.contains(section), "section missing:\n{updated}");
        assert_eq!(
            updated.replace(section, ""),
            MANIFEST,
            "existing content changed:\n{updated}"
        );
        assert!(
            !updated.contains("[package.metadata]\n"),
            "empty table added:\n{updated}"
        );
    }

    #[test]
    fn replaces_an_existing_project() {
        let saved = with_project(MANIFEST, "old-name").unwrap();
        let updated = with_project(&saved, "new-name").unwrap();

        assert!(updated.contains("project = \"new-name\""));
        assert!(!updated.contains("old-name"));
        assert!(updated.contains("# pinned to match dx"));
    }

    #[test]
    fn keeps_other_tools_metadata() {
        let manifest = format!("{MANIFEST}\n[package.metadata.docs.rs]\nall-features = true\n");
        let updated = with_project(&manifest, "my-site").unwrap();

        assert!(updated.contains("[package.metadata.docs.rs]\nall-features = true"));
        assert!(updated.contains("[package.metadata.dioxus-vercel]\nproject = \"my-site\""));
    }

    #[test]
    fn keeps_crlf_line_endings() {
        let crlf = MANIFEST.replace('\n', "\r\n");
        let updated = with_project(&crlf, "my-site").unwrap();

        assert!(updated.contains("project = \"my-site\"\r\n"));
        assert!(
            !updated.replace("\r\n", "").contains('\n'),
            "mixed line endings: {updated:?}"
        );
    }

    #[test]
    fn needs_a_package_table() {
        let err = with_project("[workspace]\nmembers = []\n", "x").unwrap_err();
        assert!(err.to_string().contains("no [package] table"));
    }
}
