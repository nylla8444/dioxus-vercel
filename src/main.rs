mod config;
mod output;
mod project;
mod tools;
mod vercel;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use crate::project::Project;
use crate::vercel::Vercel;

/// Build a Dioxus web app and deploy it to Vercel as a prebuilt static site.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Bundle the app with dx and write it to .vercel/output
    Build(BuildArgs),
    /// Build, then upload .vercel/output with `vercel deploy --prebuilt`
    Deploy {
        #[command(flatten)]
        build: BuildArgs,
        /// Deploy to production instead of a preview
        #[arg(long)]
        prod: bool,
        /// Vercel project to deploy to; it is created if it doesn't exist.
        /// Defaults to `project` in [package.metadata.dioxus-vercel].
        #[arg(long)]
        project: Option<String>,
        /// Remember --project in Cargo.toml so later deploys don't need it
        #[arg(long, requires = "project")]
        save: bool,
    },
}

#[derive(Args)]
struct BuildArgs {
    /// Path to the Dioxus project
    #[arg(long, default_value = ".")]
    path: PathBuf,
    /// Package to build, when the workspace has more than one
    #[arg(long, short)]
    package: Option<String>,
    /// Reuse the existing dx output instead of running `dx bundle`
    #[arg(long)]
    skip_bundle: bool,
    /// Return 404 for unknown paths instead of serving index.html.
    /// Use this for sites without client-side routing.
    #[arg(long)]
    no_fallback: bool,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Build(args) => {
            build(&args)?;
        }
        Command::Deploy {
            build: args,
            prod,
            project,
            save,
        } => {
            // Fail before the slow bundle step if the Vercel CLI is missing.
            let vercel = Vercel::find()?;
            let built = build(&args)?;
            let target = project.as_deref().or(built.vercel_project.as_deref());
            let created = vercel.deploy(&built.root, prod, target)?;

            if let Some(project) = &project {
                if save {
                    config::save_project(&built.manifest_path, project)?;
                    println!(
                        "Saved project `{project}` to {}",
                        built.manifest_path.display()
                    );
                } else if built.vercel_project.as_deref() != Some(project.as_str()) {
                    println!("Tip: add --save to remember `{project}` in Cargo.toml.");
                }
            }
            if !prod && !created {
                println!(
                    "Note: preview deployments need a Vercel login to view unless you turn off Deployment Protection."
                );
            }
        }
    }
    Ok(())
}

fn build(args: &BuildArgs) -> Result<Project> {
    let project = Project::load(&args.path, args.package.as_deref())?;

    if !args.skip_bundle {
        let dx = tools::find("dx", "Install it with `cargo install dioxus-cli --locked`.")?;
        tools::warn_on_dx_mismatch(&dx, &project.dioxus_version)?;
        tools::dx_bundle(&dx, &project.root, args.package.as_deref())?;
    }

    let out_dir = project.root.join(".vercel").join("output");
    let stats = output::write(
        &project.dx_public_dir(),
        &out_dir,
        &output::Options {
            fallback: !args.no_fallback,
        },
    )?;
    println!(
        "Wrote {} ({} files, {} KB)",
        out_dir.display(),
        stats.files,
        stats.bytes.div_ceil(1024)
    );
    Ok(project)
}
