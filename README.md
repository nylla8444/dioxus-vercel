# dioxus-vercel

Deploy a [Dioxus](https://dioxuslabs.com) web app to [Vercel](https://vercel.com) from Windows, macOS or Linux, without installing Rust on Vercel.

It runs `dx bundle --web --release` on your machine, writes the result in Vercel's prebuilt format ([Build Output API v3](https://vercel.com/docs/build-output-api)), and uploads it with `vercel deploy --prebuilt`. Nothing is compiled on Vercel: your files go straight to its CDN.

## Status

Version 0.1 handles **client-side Dioxus web apps**. It has been tested with Dioxus 0.7.10, dx 0.7.10 and Vercel CLI 65.7 on Windows 11.

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
cargo install --git https://github.com/nylla8444/dioxus-vercel
```

## Usage

From your Dioxus project folder:

```sh
dioxus-vercel deploy --project my-site          # preview deployment
dioxus-vercel deploy --project my-site --prod   # production deployment
dioxus-vercel build                             # only write .vercel/output
```

| Option | What it does |
|---|---|
| `--path <dir>` | Project folder (default: the current folder) |
| `-p, --package <name>` | Which package to build in a workspace with several |
| `--skip-bundle` | Reuse the last `dx bundle` output |
| `--no-fallback` | Return 404 for unknown paths instead of serving `index.html` |
| `--prod` | Deploy to production instead of a preview (`deploy` only) |
| `--project <name>` | Vercel project to deploy to; created if it doesn't exist (`deploy` only) |

## What it writes

`.vercel/output/static/` holds dx's output, and `.vercel/output/config.json` sets three routes:

1. Files under `/assets` with a dx content hash in their name (like `main-dxhf4c8ffa68b6c80c7.css`) are cached for a year. Their name changes whenever their content does, so the cache is never stale.
2. Real files are served as they are.
3. Any other path serves `index.html`, so client-side routes like `/about` work when the page is reloaded. Pass `--no-fallback` to get a 404 for those paths instead.

On a real deploy, Vercel also serves the `.wasm` file with the right content type and Brotli compression: a 742 KB test app downloads as 278 KB.

## Things to know

- **A project's first deploy goes to production.** That's how Vercel works. Later deploys are previews unless you pass `--prod`.
- **Previews need a Vercel login to view**, because Deployment Protection is on by default.
- **`--project` isn't remembered.** Pass it every time, or run `vercel link` once in your project folder. dioxus-vercel only replaces `.vercel/output`, so the link is kept.
- **Don't connect the Git repo to Vercel** without changing the project's Framework Preset to "Other". Vercel detects `Cargo.toml` and `src/main.rs` as its "Rust" preset and would try to build your app as a server function.
- **Unknown paths return status 200**, because they get `index.html` (see routes above).
- **Building on Windows is fine.** The WebAssembly output is the same on every OS, so no cross-compiling is needed.
