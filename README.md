# dioxus-vercel

Deploy a [Dioxus](https://dioxuslabs.com) web app to [Vercel](https://vercel.com). There are three ways, easiest first:

| Way | Setup | On every push |
|---|---|---|
| [**Let Vercel build it**](#let-vercel-build-it) (`init`) | Add one `vercel.json`, import the repo on Vercel | Vercel builds and deploys the app (about 1 minute after the first build); pull requests get previews |
| [**GitHub Actions**](#deploy-from-github-actions) (`init-ci`) | Add a workflow and a Vercel token secret | GitHub builds and deploys it (about 1 minute once cached) |
| [**From your machine**](#deploy-from-your-machine) (`deploy`) | Install dx and the Vercel CLI | Nothing; you run `dioxus-vercel deploy` (about 20 seconds after the first build) |

Every way ends the same: the built app goes to Vercel in its prebuilt format ([Build Output API v3](https://vercel.com/docs/build-output-api)) and is served from Vercel's CDN.

## Contents

- [Status](#status)
- [Install](#install)
- [Let Vercel build it](#let-vercel-build-it)
- [Deploy from GitHub Actions](#deploy-from-github-actions)
- [Deploy from your machine](#deploy-from-your-machine)
- [What it writes](#what-it-writes)
- [How fast is it?](#how-fast-is-it)
- [Things to know](#things-to-know)
- [License](#license)

## Status

dioxus-vercel handles **client-side Dioxus web apps**. It has been tested with Dioxus 0.7.10, dx 0.7.10 and Vercel CLI 65.7 on Windows 11, on Vercel's build machines and on GitHub Actions.

Not supported yet:
- static site generation (`dx bundle --ssg`)
- fullstack apps with server functions

## Install

```sh
cargo install dioxus-vercel --locked
```

Or install the latest code from GitHub:

```sh
cargo install --git https://github.com/nylla8444/dioxus-vercel --locked dioxus-vercel
```

## Let Vercel build it

From your Dioxus project folder:

```sh
dioxus-vercel init
git add vercel.json
git commit -m "Build on Vercel"
git push
```

Then on vercel.com, choose **Add New > Project**, import the repository and click **Deploy**. If the app is in a subfolder of the repository, set **Root Directory** to that folder. From then on, Vercel builds and deploys every push and gives every pull request a preview. No token or secret is needed.

If you have the Vercel CLI and are logged in, `init` can do the vercel.com part too:

```sh
dioxus-vercel init --project my-site
```

That also creates the Vercel project if needed, sets its Root Directory, and connects it to your repository's `origin` remote, so only the commit and push are left. Vercel's GitHub app needs access to the repository; if it doesn't have it yet, give it access at [github.com/apps/vercel](https://github.com/apps/vercel).

`init` writes this `vercel.json`, or adds it to yours and keeps your other settings. You can also copy it in by hand, without installing anything:

```json
{
  "framework": null,
  "installCommand": "cargo install dioxus-vercel --version 0.3.2 --locked",
  "buildCommand": "dioxus-vercel build"
}
```

What happens on Vercel:

- The install command builds dioxus-vercel with the Rust that Vercel's build machines already have.
- `dioxus-vercel build` notices that it's running on Vercel and sets up the rest: stable Rust with the WebAssembly target (or the toolchain from your `rust-toolchain` file), and a dx for your `dioxus` version.
- DioxusLabs' Linux builds of dx need glibc 2.35 or newer, and Vercel's machines have 2.34. So dioxus-vercel downloads a dx built on Amazon Linux 2023 by this repo's [build-dx workflow](.github/workflows/build-dx.yml), and checks its checksum.
- The first build takes about 3 minutes. dioxus-vercel keeps Rust's build output in `node_modules/.cache`, which Vercel saves and restores for Git-connected projects, so later builds take under a minute.

This repo builds that dx automatically for every Dioxus version from 0.7.0 on, plus pre-releases newer than the latest stable version, usually within a day of a release ([the dx releases](https://github.com/nylla8444/dioxus-vercel/releases)). If your version has no build yet, the build compiles dx itself, which took over 15 minutes in our tests.

| `init` option | What it does |
|---|---|
| `--path <dir>` | Project folder (default: the current folder) |
| `-p, --package <name>` | Which package to build in a workspace with several |
| `--project <name>` | Also create the Vercel project and connect it to the repository (needs the Vercel CLI) |
| `--force` | Replace build settings someone else put in `vercel.json` |

## Deploy from GitHub Actions

If you'd rather build on GitHub's machines, which are faster once their cache is warm, `init-ci` writes a GitHub Actions workflow instead:

```sh
dioxus-vercel init-ci --project my-site
```

It writes two files and saves the project name to `Cargo.toml`:

- `.github/workflows/dioxus-vercel.yml`, at the root of your git repository. A push to `main` deploys to production. A pull request deploys a preview and gets a comment with its link.
- `vercel.json` next to your `Cargo.toml`, with `"git": { "deploymentEnabled": false }`, so Vercel doesn't also try to build every push. An existing `vercel.json` is left alone.

The workflow keeps Rust's build output between runs with [rust-cache](https://github.com/Swatinem/rust-cache) and installs the prebuilt `dx` that matches your `dioxus` version.

To finish:

1. Create a token at [vercel.com/account/tokens](https://vercel.com/account/tokens). For Scope, pick the team that owns the project and **All Projects**, so the workflow can create the project on its first run. Set an expiration, so a leaked token stops working on its own. (`vercel tokens create` doesn't work here: a session from `vercel login` can't create tokens.)
2. On GitHub, add it as a repository secret named `VERCEL_TOKEN` (Settings > Secrets and variables > Actions).
3. Commit and push the workflow, `vercel.json` and `Cargo.toml`.

| `init-ci` option | What it does |
|---|---|
| `--path <dir>` | Project folder (default: the current folder) |
| `-p, --package <name>` | Which package to deploy in a workspace with several |
| `--project <name>` | Vercel project to deploy to; saved to `Cargo.toml` |
| `--force` | Replace an existing workflow file |

Pull requests from forks don't get repository secrets, so the workflow skips them. Use either `init` or `init-ci`, not both, or every push deploys twice.

This repo deploys its own [example app](examples/hello) this way.

## Deploy from your machine

You need:

- Rust with the WebAssembly target:
  ```sh
  rustup target add wasm32-unknown-unknown
  ```
- The Dioxus CLI, on the **same version** as your `dioxus` dependency. dioxus-vercel warns you if they don't match.
  ```sh
  cargo install dioxus-cli --version 0.7.10 --locked
  ```
- The Vercel CLI, logged in:
  ```sh
  npm i -g vercel
  vercel login
  ```

Then, from your Dioxus project folder:

```sh
dioxus-vercel deploy --project my-site --save   # first deploy; remembers the project
dioxus-vercel deploy                            # preview deployment
dioxus-vercel deploy --prod                     # production deployment
dioxus-vercel build                             # only write .vercel/output
```

`--save` adds this to your `Cargo.toml`, and you can also write it by hand:

```toml
[package.metadata.dioxus-vercel]
project = "my-site"
```

| Option | What it does |
|---|---|
| `--path <dir>` | Project folder (default: the current folder) |
| `-p, --package <name>` | Which package to build in a workspace with several |
| `--skip-bundle` | Reuse the last `dx bundle` output |
| `--no-fallback` | Return 404 for unknown paths instead of serving `index.html` |
| `--prod` | Deploy to production instead of a preview (`deploy` only) |
| `--project <name>` | Vercel project to deploy to; created if it doesn't exist (`deploy` only) |
| `--save` | Remember `--project` in `Cargo.toml` (`deploy` only) |

The project is picked from `--project`, then `Cargo.toml`, then a folder linked with `vercel link`. If none of those are set, Vercel asks.

When dioxus-vercel creates a project, it sets the project's Framework Preset to "Other". Otherwise Vercel would pick its "Rust" preset from `Cargo.toml` and `src/main.rs`, and a build from a connected Git repo would try to run your app as a server function. For existing projects with a Rust preset, it prints a warning with the command that fixes it.

## What it writes

`.vercel/output/static/` holds dx's output, and `.vercel/output/config.json` sets three routes:

1. Files under `/assets` with a dx content hash in their name (like `main-dxhf4c8ffa68b6c80c7.css`) are cached for a year. Their name changes whenever their content does, so the cache is never stale.
2. Real files are served as they are.
3. Any other path serves `index.html`, so client-side routes like `/about` work when the page is reloaded. Pass `--no-fallback` to get a 404 for those paths instead.

On a real deploy, Vercel also serves the `.wasm` file with the right content type and Brotli compression: a 742 KB test app downloads as 278 KB.

## How fast is it?

These times were measured in November 2026 on two small two-page apps: this repo's [example app](examples/hello) and the [init demo](https://github.com/nylla8444/dioxus-vercel-init-demo). A bigger app takes longer to compile, but the differences between the approaches stay.

**Builds on Vercel's own machines** (a Git repository connected to Vercel):

| Approach | First build | Later builds |
|---|---|---|
| Without dioxus-vercel, using DioxusLabs' prebuilt `dx` | ❌ Fails: `dx` needs glibc 2.35 or newer, and Vercel's machines have 2.34 | ❌ Fails |
| Without dioxus-vercel, compiling `dx` during the build | ❌ Still compiling `dx` after 17 minutes, when we stopped it | — |
| **With dioxus-vercel (`init`)** | **2 min 56 s** | **57 s to 1 min**, because Vercel restores the cache |

In a later build, about 22 seconds go to installing dioxus-vercel, 13 to setting up Rust and 20 to compiling your own code. `dx` and your dependencies come from the cache.

**Getting `dx`**, which every build needs:

| How | Time |
|---|---|
| Compile it on Vercel's build machine (2 CPUs) | Over 17 minutes (stopped) |
| Compile it on GitHub Actions | 6 to 13 minutes, across 15 Dioxus versions |
| Compile it on a Windows laptop | 8 minutes |
| **Download dioxus-vercel's build for Vercel** | **1 second** |

**dioxus-vercel's other routes:**

| Way | First run | Later runs |
|---|---|---|
| GitHub Actions (`init-ci`) | 2 min 6 s | 62 to 75 s, with the Rust cache |
| From your machine (`deploy`) | About 6 minutes, compiling everything once | About 20 seconds |

For comparison, another Rust and WebAssembly project [reported](https://github.com/Jlore-Studios/JackiOh/issues/997) Vercel builds of about 4 minutes, and 12 after adding a C++ module, because Vercel kept none of its Rust build output. We haven't timed other setups, such as Vercel's container runtime or a hand-written GitHub Actions workflow.

## Things to know

- **A project's first deploy goes to production.** That's how Vercel works. Later deploys are previews unless you pass `--prod`.
- **Previews need a Vercel login to view**, because Deployment Protection is on by default.
- **Connecting a Git repo to Vercel only works with `init`'s `vercel.json`.** Without it, Vercel can't build a Dioxus app.
- **Unknown paths return status 200**, because they get `index.html` (see routes above).
- **Building on Windows is fine.** The WebAssembly output is the same on every OS, so no cross-compiling is needed.

## License

[MIT](LICENSE)
