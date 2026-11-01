mod ci;
mod config;
mod init;
mod output;
mod project;
mod tools;
mod vercel;
mod vercel_build;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
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
    /// Set up vercel.json so Vercel builds and deploys the app itself on
    /// every push to a connected Git repo
    Init {
        #[command(flatten)]
        target: TargetArgs,
        /// Replace build settings someone else put in vercel.json
        #[arg(long)]
        force: bool,
    },
    /// Write a GitHub Actions workflow that deploys every push to main and
    /// every pull request
    InitCi {
        #[command(flatten)]
        target: TargetArgs,
        /// Vercel project to deploy to; saved to Cargo.toml for the workflow.
        /// Defaults to `project` in [package.metadata.dioxus-vercel].
        #[arg(long)]
        project: Option<String>,
        /// Replace an existing workflow file
        #[arg(long)]
        force: bool,
    },
}

#[derive(Args)]
struct TargetArgs {
    /// Path to the Dioxus project
    #[arg(long, default_value = ".")]
    path: PathBuf,
    /// Package to build, when the workspace has more than one
    #[arg(long, short)]
    package: Option<String>,
}

#[derive(Args)]
struct BuildArgs {
    #[command(flatten)]
    target: TargetArgs,
    /// Reuse the existing dx output instead of running `dx bundle`
    #[arg(long)]
    skip_bundle: bool,
    /// Return 404 for unknown paths instead of serving index.html.
    /// Use this for sites without client-side routing.
    #[arg(long)]
    no_fallback: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if vercel_build::running_on_vercel() {
        vercel_build::use_cache_folder()?;
    }
    match cli.command {
        Command::Build(args) => {
            let project = load(&args.target)?;
            build(&args, &project)?;
        }
        Command::Deploy {
            build: args,
            prod,
            project: project_flag,
            save,
        } => {
            // Fail before the slow bundle step if the Vercel CLI is missing.
            let vercel = Vercel::find()?;
            let project = load(&args.target)?;
            let target = project_flag
                .as_deref()
                .or(project.vercel_project.as_deref());
            if target.is_none() && ci::running_in_ci() {
                bail!(
                    "no Vercel project set. Without one, Vercel asks which project to use, \
                     which can't work in CI. Pass --project or set `project` in \
                     [package.metadata.dioxus-vercel] in Cargo.toml."
                );
            }

            build(&args, &project)?;
            let deployment = vercel.deploy(&project.root, prod, target)?;
            if let Some(url) = &deployment.url {
                println!("Deployed: {url}");
                ci::report_to_github(url)?;
            }

            if let Some(name) = &project_flag {
                if save {
                    config::save_project(&project.manifest_path, name)?;
                    println!(
                        "Saved project `{name}` to {}",
                        project.manifest_path.display()
                    );
                } else if project.vercel_project.as_deref() != Some(name.as_str()) {
                    println!("Tip: add --save to remember `{name}` in Cargo.toml.");
                }
            }
            if !prod && !deployment.created {
                println!(
                    "Note: preview deployments need a Vercel login to view unless you turn off Deployment Protection."
                );
            }
        }
        Command::Init { target, force } => init(&target, force)?,
        Command::InitCi {
            target,
            project,
            force,
        } => init_ci(&target, project.as_deref(), force)?,
    }
    Ok(())
}

fn load(target: &TargetArgs) -> Result<Project> {
    Project::load(&target.path, target.package.as_deref())
}

fn build(args: &BuildArgs, project: &Project) -> Result<()> {
    if !args.skip_bundle {
        let dx = if vercel_build::running_on_vercel() {
            vercel_build::prepare(project)?
        } else {
            tools::find("dx", "Install it with `cargo install dioxus-cli --locked`.")?
        };
        tools::warn_on_dx_mismatch(&dx, &project.dioxus_version)?;
        // dx doesn't delete files from earlier bundles, so without this, old
        // hashed assets would pile up and ship with every deploy.
        let public = project.dx_public_dir();
        if public.exists() {
            std::fs::remove_dir_all(&public)
                .with_context(|| format!("could not clear {}", public.display()))?;
        }
        tools::dx_bundle(&dx, &project.root, args.target.package.as_deref())?;
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
    Ok(())
}

fn init(target: &TargetArgs, force: bool) -> Result<()> {
    let project = load(target)?;
    let path = project.root.join("vercel.json");
    let existing = path
        .exists()
        .then(|| std::fs::read_to_string(&path))
        .transpose()
        .with_context(|| format!("could not read {}", path.display()))?;
    let updated = init::vercel_json(existing.as_deref(), target.package.as_deref(), force)?;
    if existing.as_deref() == Some(updated.text.as_str()) {
        println!("{} is already set up.", path.display());
    } else {
        std::fs::write(&path, &updated.text)
            .with_context(|| format!("could not write {}", path.display()))?;
        println!("Wrote {}", path.display());
    }
    for note in &updated.notes {
        println!("{note}");
    }

    // Vercel reads vercel.json from the project's Root Directory setting.
    let repo = ci::repo_root(&project.root);
    let root_directory = match &repo {
        Some(repo) => ci::workdir(repo, &project.root)?,
        None => ".".to_owned(),
    };
    if let Some(repo) = &repo {
        let workflow = ci::workflow_path(repo);
        if workflow.exists() {
            println!(
                "You also have the GitHub Actions workflow from init-ci, so every push would \
                 deploy twice. Delete {} if you switch to this setup.",
                workflow.display()
            );
        }
    }
    let root_hint = if root_directory == "." {
        String::new()
    } else {
        format!(", set Root Directory to `{root_directory}`")
    };
    println!(
        "\nTo finish:\n\
         1. Commit and push vercel.json.\n\
         2. On vercel.com, choose Add New > Project, import this repository{root_hint}, \
         and click Deploy.\n\
         Vercel then builds and deploys every push itself, and gives every pull request a preview."
    );
    Ok(())
}

fn init_ci(target: &TargetArgs, project_flag: Option<&str>, force: bool) -> Result<()> {
    let project = load(target)?;

    // The workflow reads the project from Cargo.toml, so it has to be saved there.
    let name = match (project_flag, project.vercel_project.as_deref()) {
        (Some(name), saved) => {
            if saved != Some(name) {
                config::save_project(&project.manifest_path, name)?;
                println!(
                    "Saved project `{name}` to {}",
                    project.manifest_path.display()
                );
            }
            name
        }
        (None, Some(saved)) => saved,
        (None, None) => bail!(
            "init-ci needs a Vercel project. Pass --project <name>; it's saved to \
             Cargo.toml so the workflow can use it."
        ),
    };

    let repo = ci::repo_root(&project.root).with_context(|| {
        format!(
            "{} isn't inside a git repository. Run `git init` first.",
            project.root.display()
        )
    })?;
    let vercel_version = Vercel::find()
        .ok()
        .and_then(|vercel| vercel.major_version())
        .unwrap_or_else(|| "latest".to_owned());

    let workflow_path = ci::workflow_path(&repo);
    let workflow = ci::workflow(&ci::WorkflowOptions {
        workdir: &ci::workdir(&repo, &project.root)?,
        package: target.package.as_deref(),
        dioxus_version: &project.dioxus_version,
        vercel_version: &vercel_version,
    });
    ci::write_new(&workflow_path, &workflow, force)?;
    println!("Wrote {}", workflow_path.display());

    let vercel_json = project.root.join("vercel.json");
    if vercel_json.exists() {
        let existing = std::fs::read_to_string(&vercel_json).unwrap_or_default();
        if !existing.contains("deploymentEnabled") {
            println!(
                "Left {} as it is. Add \"git\": {{ \"deploymentEnabled\": false }} to it, \
                 or Vercel will also try to build every push itself.",
                vercel_json.display()
            );
        }
    } else {
        ci::write_new(&vercel_json, ci::VERCEL_JSON, false)?;
        println!(
            "Wrote {} (stops Vercel from building pushes itself)",
            vercel_json.display()
        );
    }

    println!(
        "\nThe workflow deploys to the Vercel project `{name}`. To finish:\n\
         1. Create a token at https://vercel.com/account/tokens. For Scope, pick the \
         team that owns the project and All Projects, and set an expiration.\n\
         2. On GitHub, add it as a repository secret named VERCEL_TOKEN \
         (Settings > Secrets and variables > Actions).\n\
         3. Commit and push the workflow, vercel.json and Cargo.toml."
    );
    Ok(())
}
