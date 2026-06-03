# psudo

See our paper [psudo: Exploring Multi-Channel Biomedical Image Data with Spatially and Perceptually Optimized Pseudocoloring](https://www.biorxiv.org/content/10.1101/2024.04.11.589087v1) for more information.
## Publication

This package is developed following the research conducted by Simon Warchol, Jakob Troidl, Jeremy Muhlich, Robert Krueger, John Hoffer, Tica Lin, Johanna Beyer, Elena Glassman, Peter Sorger, and Hanspeter Pfister. For a detailed explanation of the methodologies and their applications, please refer to the original paper linked above.

### Affiliations
- Harvard John A. Paulson School of Engineering and Applied Sciences
- Harvard Medical School
- New York University Tandon School of Engineering

## Use this repository (web app + palette lab)

```bash
pnpm install
pnpm run wasm-build   # first time / after Rust changes (needs rustup + wasm-pack)
pnpm dev
```

Open the viewer at `/` or the WASM test UI at **`/lab`**.

## Use as an npm package in another app

Install the published WASM package and import `optimize` (and related helpers) from JavaScript/TypeScript:

```bash
npm install psudo
```

```javascript
import * as psudo from "psudo";
// or: import { optimize, calculate_palette_loss } from "psudo";

const optimized = await psudo.optimize(
  colors,           // Uint16Array — flat RGB 0–255 per channel
  locked,           // Uint16Array — 1 = locked, 0 = free
  intensities,      // Uint16Array — nRows × nChannels, column-major
  contrastLimits,   // Uint16Array — [min,max] per channel
  luminance,        // Uint16Array — [minL, maxL] in OKLab × 100
  excluded,         // string[] — C3 names to avoid (e.g. "grey", "white")
  colorNames,       // string[] — optional hint per channel, "" if none
  undefined,
  undefined,
  false             // include_spatial_channel_overlap (false = fast)
);
// optimized: Float32Array linear sRGB 0–1, length 3 × nChannels
```

Full API, Web Worker + Vite setup, React notes, and parameter tables: **[lib/README.md](lib/README.md)** (also shipped on [npm](https://www.npmjs.com/package/psudo)). Sync main-thread API: `import from "psudo/sync"`.

### WASM build prerequisites

`wasm-pack` must be on your PATH (`cargo install wasm-pack` or `brew install wasm-pack`).

Use **rustup** for the compiler (not Homebrew `rustc` alone). Homebrew Rust does not ship the `wasm32-unknown-unknown` target, which produces:

```text
wasm32-unknown-unknown target not found in sysroot: "/opt/homebrew/Cellar/rust/..."
```

If both are installed, `pnpm run wasm-build` prepends rustup’s toolchain so the correct `rustc` is used. One-time setup:

```bash
rustup target add wasm32-unknown-unknown
```

## CI / deploy

The web app is a static SPA (Vite + in-browser WASM). Production hosting is **GitHub Pages** — there is no application backend.

### GitHub Pages (production)

On every push to `main`, [`.github/workflows/deploy.yml`](.github/workflows/deploy.yml) runs **pnpm**, **Node 24**, and **wasm-pack** to build `dist/`, then deploys it with `actions/deploy-pages`.

**One-time repo setup** (maintainer, in GitHub):

1. **Settings → Pages → Build and deployment**: set **Source** to **GitHub Actions**.
2. **Custom domain**: enter your production hostname (e.g. `psudo.example.com`). GitHub shows the DNS records to add at your registrar (apex often uses A/AAAA records; subdomains use `CNAME` to `<user>.github.io`).
3. After DNS propagates, enable **Enforce HTTPS**.

Preview production builds locally: `pnpm run build && pnpm run preview`.

### npm publish

[`.github/workflows/publish-npm.yml`](.github/workflows/publish-npm.yml): on release or manual dispatch. Add repo secret `NPM_TOKEN` — a granular npm access token with **publish** permission (create at [npmjs.com](https://www.npmjs.com/settings/~youruser/tokens); do not commit tokens or CLI auth URLs).

### Decommissioning AWS (after GitHub Pages cutover)

Once the custom domain serves the new deployment:

1. Stop or delete the **App Runner** service (`psudo-app`, `us-east-1`).
2. Optionally empty the **ECR** repository `psudo` in account `337392631707`.
3. Remove GitHub repo secrets **`AWS_ACCESS_KEY_ID`** and **`AWS_SECRET_ACCESS_KEY`** if they are no longer used.