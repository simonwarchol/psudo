# psudo

WASM palette optimization for multi-channel microscopy imaging (C3 color-name distances, perceptual separation, optional spatial overlap).

Paper: [psudo: Exploring Multi-Channel Biomedical Image Data with Spatially and Perceptually Optimized Pseudocoloring](https://www.biorxiv.org/content/10.1101/2024.04.11.589087v1)

## Installation

```bash
npm install psudo
```

Requires a bundler that supports WebAssembly (Vite recommended).

### Vite

```bash
npm install vite-plugin-wasm vite-plugin-top-level-await
```

```javascript
// vite.config.js
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default {
  plugins: [wasm(), topLevelAwait()],
  worker: {
    plugins: () => [wasm(), topLevelAwait()],
  },
};
```

## Import (Web Worker by default)

All exports are **async** and run in a **shared module worker** so the UI thread stays responsive.

```javascript
import * as psudo from "psudo";

// optional: preload WASM before the user clicks Optimize
await psudo.warmup();

const optimized = await psudo.optimize(/* ... */);
```

TypeScript types: `index.d.ts`. For **synchronous** WASM on the main thread (tests, debugging):

```javascript
import * as psudo from "psudo/sync";
const optimized = psudo.optimize(/* ... */);
```

Named imports:

```javascript
import { optimize, calculate_palette_loss, channel_gmm, ln, warmup } from "psudo";
```

## `optimize` — palette colors (main API)

Returns a `Float32Array` of **linear sRGB** in **0–1**, length `3 × nChannels` (channel-major: `[r,g,b, r,g,b, …]`).

```javascript
import * as psudo from "psudo";

const nChannels = 4;
const nRows = 1024;

// Per-channel RGB 0–255 (flat)
const colors = new Uint16Array([
  255, 0, 0,    // ch0 red
  0, 255, 0,    // ch1 green
  0, 0, 255,    // ch2 blue
  255, 255, 0,  // ch3 yellow
]);

// 1 = locked (held fixed), 0 = free to optimize
const locked = new Uint16Array([0, 0, 1, 0]);

// Intensities: column-major, shape nRows × nChannels
// index = channel * nRows + row
const intensities = new Uint16Array(nRows * nChannels);
for (let ch = 0; ch < nChannels; ch++) {
  for (let row = 0; row < nRows; row++) {
    intensities[ch * nRows + row] = 8000 + ((row * 13 + ch * 997) % 50000);
  }
}

// Per channel: [min, max] contrast (uint16)
const contrastLimits = new Uint16Array(nChannels * 2);
for (let i = 0; i < nChannels; i++) {
  contrastLimits[i * 2] = 0;
  contrastLimits[i * 2 + 1] = 65535;
}

// OKLab L bounds × 100 (e.g. 0.45–0.92)
const luminance = new Uint16Array([45, 92]);

const excluded = ["grey", "white", "lightgrey", "darkgrey", "offwhite"];
const colorNames = ["red", "", "blue", ""]; // optional C3 hint per channel; "" = none

const optimized = await psudo.optimize(
  colors,
  locked,
  intensities,
  contrastLimits,
  luminance,
  excluded,
  colorNames,
  undefined, // max_iters (WASM default ~1500, scaled by channel count)
  undefined, // confusion_baseline_samples
  false,     // include_spatial_channel_overlap (false = fast color-only path)
  undefined  // num_restarts
);

// Display RGB 0–255
for (let ch = 0; ch < nChannels; ch++) {
  const i = ch * 3;
  const rgb = [
    Math.round(optimized[i] * 255),
    Math.round(optimized[i + 1] * 255),
    Math.round(optimized[i + 2] * 255),
  ];
  console.log(`channel ${ch}:`, rgb);
}
```

### React (client-only)

Call `optimize` inside `useEffect` or an event handler so WASM runs in the browser, not during SSR:

```jsx
import { useState, useCallback } from "react";
import * as psudo from "psudo";

export function usePaletteOptimizer() {
  const [busy, setBusy] = useState(false);

  const runOptimize = useCallback((colors, locked, intensities, contrast, lum, excluded, names) => {
    setBusy(true);
    try {
      return await psudo.optimize(
        colors,
        locked,
        intensities,
        contrast,
        lum,
        excluded,
        names,
        undefined,
        undefined,
        false
      );
    } finally {
      setBusy(false);
    }
  }, []);

  return { runOptimize, busy };
}
```

## Other exports

| Function | Description |
|----------|-------------|
| `optimize` | Simulated-annealing palette optimization → `Float32Array` linear RGB |
| `calculate_palette_loss` | Loss breakdown object for a palette + intensities |
| `channel_gmm` | Per-channel GMM contrast limits from raw `Uint16Array` data |
| `ln` | Log transform of intensity data |
| `optimize_in_lens` | Lens-local confusion metric (scalar) |

### `calculate_palette_loss`

```javascript
import { calculate_palette_loss } from "psudo";

const loss = await calculate_palette_loss(
  intensities,
  colors,
  contrastLimits,
  luminance,
  excluded,
  colorNames,
  false // include_spatial_channel_overlap
);

console.log(loss.perceptual_distance, loss.name_distance, loss.min_display_rgb_distance);
```

## Optional parameters (`optimize`)

| Argument | Default (WASM) | Notes |
|----------|----------------|-------|
| `max_iters` | ~1500 (× channels/3) | Higher = slower, often better |
| `confusion_baseline_samples` | 16 | MC samples when spatial overlap is on |
| `include_spatial_channel_overlap` | `false` | `true` uses image intensities in objective (slower) |
| `num_restarts` | 2 (× channels/3) | Independent SA runs; best total wins |

## Building from source

```bash
# From the psudo repo root
pnpm run wasm-build
# Artifacts: lib/pkg/ (index.js, psudo.worker.js, psudo.js, psudo_bg.wasm, …)
```

Publish to npm from `lib/pkg` after `wasm-pack build` (see repo root README).

## Publication

Developed by Simon Warchol, Jakob Troidl, Jeremy Muhlich, Robert Krueger, John Hoffer, Tica Lin, Johanna Beyer, Elena Glassman, Peter Sorger, and Hanspeter Pfister.

### Affiliations

- Harvard John A. Paulson School of Engineering and Applied Sciences
- Harvard Medical School
- New York University Tandon School of Engineering
