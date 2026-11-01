# dioxus-vercel

Deploy a [Dioxus](https://dioxuslabs.com) web app to [Vercel](https://vercel.com) from Windows, macOS or Linux, without installing Rust on Vercel.

It runs `dx bundle --web --release` on your machine, writes the result in Vercel's prebuilt format ([Build Output API v3](https://vercel.com/docs/build-output-api)), and uploads it with `vercel deploy --prebuilt`. Nothing is compiled on Vercel: your files go straight to its CDN.

## Status

dioxus-vercel handles **client-side Dioxus web apps**, deployed from your machine or from GitHub Actions. It has been tested with Dioxus 0.7.10, dx 0.7.10 and Vercel CLI 65.7 on Windows 11.

Not supported yet:
- static site generation (`dx bundle --ssg`)
- fullstack apps with server functions

## Requirements

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

## Install

```sh
cargo install dioxus-vercel --locked
```

Or install the latest code from GitHub:

```sh
cargo install --git https://github.com/nylla8444/dioxus-vercel --locked
```

## Usage

From your Dioxus project folder:

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

## Deploy from GitHub Actions

Vercel can't build Dioxus apps, so connecting your repo to Vercel won't give you automatic deploys. `init-ci` sets them up with GitHub Actions instead:

```sh
dioxus-vercel init-ci --project my-site
```

It writes two files and saves the project name to `Cargo.toml`:

- `.github/workflows/dioxus-vercel.yml`, at the root of your git repository. A push to `main` deploys to production. A pull request deploys a preview and gets a comment with its link.
- `vercel.json` next to your `Cargo.toml`, with `"git": { "deploymentEnabled": false }`, so Vercel doesn't also try to build every push. An existing `vercel.json` is left alone.

The workflow builds on GitHub's machines, keeps Rust's build output between runs with [rust-cache](https://github.com/Swatinem/rust-cache), and installs the prebuilt `dx` that matches your `dioxus` version.

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

Pull requests from forks don't get repository secrets, so the workflow skips them.

This repo deploys its own [example app](examples/hello) this way.

## What it writes

`.vercel/output/static/` holds dx's output, and `.vercel/output/config.json` sets three routes:

1. Files under `/assets` with a dx content hash in their name (like `main-dxhf4c8ffa68b6c80c7.css`) are cached for a year. Their name changes whenever their content does, so the cache is never stale.
2. Real files are served as they are.
3. Any other path serves `index.html`, so client-side routes like `/about` work when the page is reloaded. Pass `--no-fallback` to get a 404 for those paths instead.

On a real deploy, Vercel also serves the `.wasm` file with the right content type and Brotli compression: a 742 KB test app downloads as 278 KB.

## Things to know

- **A project's first deploy goes to production.** That's how Vercel works. Later deploys are previews unless you pass `--prod`.
- **Previews need a Vercel login to view**, because Deployment Protection is on by default.
- **Connecting the Git repo to Vercel won't deploy your app.** Vercel can't build Dioxus apps. Use `init-ci` for automatic deploys instead.
- **Unknown paths return status 200**, because they get `index.html` (see routes above).
- **Building on Windows is fine.** The WebAssembly output is the same on every OS, so no cross-compiling is needed.

## License

[MIT](LICENSE)
