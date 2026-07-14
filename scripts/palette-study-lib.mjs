/**
 * Shared helpers for WASM palette_study (mirrors lib/examples/palette_study.rs defaults).
 */

export const DEFAULT_LUMINANCE = new Uint16Array([45, 92]);
export const DEFAULT_MAX_ITERS = 3000;
export const DEFAULT_CONFUSION_SAMPLES = 32;
export const DEFAULT_RESTARTS = 12;
export const DEFAULT_CHANNEL_COUNTS = [4, 6];
export const DEFAULT_PARENTS = 20;
export const DEFAULT_ROWS = 384;

export function parseEnvInt(name, fallback) {
  const v = process.env[name];
  if (v == null || v === "") return fallback;
  const n = Number.parseInt(v, 10);
  return Number.isFinite(n) && n > 0 ? n : fallback;
}

export function parseEnvBool(name) {
  const v = process.env[name];
  return v != null && /^(1|true|yes)$/i.test(v);
}

export function parseChannelCounts() {
  const raw = process.env.PALETTE_STUDY_CHANNELS;
  if (!raw) return [...DEFAULT_CHANNEL_COUNTS];
  const out = raw
    .split(",")
    .map((s) => Number.parseInt(s.trim(), 10))
    .filter((n) => Number.isFinite(n) && n >= 2);
  return out.length > 0 ? [...new Set(out)].sort((a, b) => a - b) : [6];
}

export function scaledBudget(base, channels) {
  return Math.max(1, Math.floor((Number(base) * channels) / 3));
}

/** Mulberry32 — deterministic floats in [0, 1). */
export function mulberry32(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** palette_study `random_intensities`: channel-major, 2000..62000. */
export function studyIntensities(nRows, nChannels, seed = 9000) {
  const rnd = mulberry32(seed + nChannels);
  const out = new Uint16Array(nChannels * nRows);
  for (let ch = 0; ch < nChannels; ch++) {
    for (let row = 0; row < nRows; row++) {
      out[ch * nRows + row] = 2000 + Math.floor(rnd() * (62000 - 2000));
    }
  }
  return out;
}

export function contrastAll(nChannels) {
  const out = new Uint16Array(nChannels * 2);
  for (let i = 0; i < nChannels; i++) {
    out[i * 2] = 0;
    out[i * 2 + 1] = 65535;
  }
  return out;
}

function srgbToOklabApprox(r, g, b) {
  const rl = r <= 0.04045 ? r / 12.92 : ((r + 0.055) / 1.055) ** 2.4;
  const gl = g <= 0.04045 ? g / 12.92 : ((g + 0.055) / 1.055) ** 2.4;
  const bl = b <= 0.04045 ? b / 12.92 : ((b + 0.055) / 1.055) ** 2.4;
  const l = 0.4122214708 * rl + 0.5363325363 * gl + 0.0514459929 * bl;
  const m = 0.2119034982 * rl + 0.6806995451 * gl + 0.1073969566 * bl;
  const s = 0.0883024619 * rl + 0.2817188376 * gl + 0.6299787005 * bl;
  const l_ = Math.cbrt(l);
  const m_ = Math.cbrt(m);
  const s_ = Math.cbrt(s);
  const L = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_;
  const a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_;
  const b2 = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_;
  return { L, a, b: b2 };
}

function oklabToSrgb8(L, a, b) {
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ ** 3;
  const m = m_ ** 3;
  const s = s_ ** 3;
  let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
  let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
  let bl = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
  const comp = (c) => {
    const x = Math.max(0, Math.min(1, c));
    const lin = x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055;
    return Math.max(0, Math.min(255, Math.round(lin * 255)));
  };
  return [comp(r), comp(g), comp(bl)];
}

/** Evenly spaced OKLab hues (palette_study `spread_initial_colors_u16`). */
export function spreadInitialColorsU16(nChannels, seed) {
  const rnd = mulberry32(seed);
  const colors = new Uint16Array(nChannels * 3);
  const primaries = [
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
  ];
  const nPrimaries = Math.min(3, nChannels);
  for (let i = 0; i < nPrimaries; i++) {
    colors[i * 3] = primaries[i][0];
    colors[i * 3 + 1] = primaries[i][1];
    colors[i * 3 + 2] = primaries[i][2];
  }
  if (nChannels <= 3) return colors;
  const l = 0.58;
  const TAU = Math.PI * 2;
  const extra = nChannels - 3;
  for (let i = 0; i < extra; i++) {
    const angle = (TAU * (i + 0.5)) / extra + (rnd() * 0.16 - 0.08);
    const chroma = 0.18 + rnd() * (0.34 - 0.18);
    const a = chroma * Math.cos(angle);
    const b = chroma * Math.sin(angle);
    const [r, g, bl] = oklabToSrgb8(l, a, b);
    const idx = 3 + i;
    colors[idx * 3] = r;
    colors[idx * 3 + 1] = g;
    colors[idx * 3 + 2] = bl;
  }
  return colors;
}

export function buildStudyInputs(nChannels, colors, options = {}) {
  const {
    nRows = DEFAULT_ROWS,
    intensitySeed = 9000 + nChannels,
    locked = null,
  } = options;
  const lockedArr =
    locked ?? new Uint16Array(nChannels);
  return {
    colors,
    locked: lockedArr,
    intensities: studyIntensities(nRows, nChannels, intensitySeed),
    contrastLimits: contrastAll(nChannels),
    luminance: DEFAULT_LUMINANCE,
    excluded: [],
    colorNames: Array(nChannels).fill(""),
    maxIters: DEFAULT_MAX_ITERS,
    confusionSamples: DEFAULT_CONFUSION_SAMPLES,
    spatial: false,
    numRestarts: DEFAULT_RESTARTS,
  };
}

export function linearToDisplayRgb8(linear, channelIndex) {
  const i = channelIndex * 3;
  const comp = (c) =>
    Math.max(0, Math.min(255, Math.round(Math.max(0, Math.min(1, c)) * 255)));
  return [comp(linear[i]), comp(linear[i + 1]), comp(linear[i + 2])];
}

export function lossTotalFromComponents(loss) {
  const n = loss.name_distance ?? 0;
  const p = loss.perceptual_distance ?? loss.perceptural_distance ?? 0;
  const pd = loss.perceptual_deficit ?? 0;
  const t = loss.term_loss ?? 0;
  const c = loss.confusion ?? 0;
  const satR = loss.min_saturation_reward ?? 0;
  const satD = loss.saturation_deficit ?? 0;
  return n + p + pd + t + c + satR + satD;
}

export function formatDuration(ms) {
  if (ms >= 10_000) return `${(ms / 1000).toFixed(2)}s`;
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)}s`;
  return `${Math.round(ms)}ms`;
}

/**
 * Minerva / exhibit document shape (no Zod): channelGroups with display RGB.
 * Matches normalized {@link DocumentData} after validateDocumentData.
 */
export function channelGroupsFromDocument(raw) {
  const groups = raw?.channelGroups ?? raw?.groups ?? [];
  if (!Array.isArray(groups)) return [];
  const out = [];
  for (const g of groups) {
    const rows = g?.channels ?? g?.GroupChannels ?? [];
    if (!Array.isArray(rows) || rows.length < 2) continue;
    const channels = [];
    for (const gc of rows) {
      const color = gc?.color ?? gc?.Color;
      if (!color) continue;
      const r = color.r ?? color.R ?? 0;
      const gch = color.g ?? color.G ?? 0;
      const b = color.b ?? color.B ?? 0;
      channels.push({ r, g, b });
    }
    if (channels.length < 2) continue;
    out.push({
      id: g.id ?? g.name ?? `group-${out.length}`,
      name: g.name ?? g.Name ?? g.id ?? "group",
      channels,
    });
  }
  return out;
}

export function colorsU16FromRgb(channels) {
  const n = channels.length;
  const colors = new Uint16Array(n * 3);
  for (let i = 0; i < n; i++) {
    const { r, g, b } = channels[i];
    colors[i * 3] = Math.max(0, Math.min(65535, Math.round(r)));
    colors[i * 3 + 1] = Math.max(0, Math.min(65535, Math.round(g)));
    colors[i * 3 + 2] = Math.max(0, Math.min(65535, Math.round(b)));
  }
  return colors;
}

export function svgSwatches(rgb8, sw = 40, sh = 100) {
  const n = rgb8.length;
  const w = sw * n;
  let s = `<div class="swatches-wrap"><svg class="swatches" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${sh}" width="${w}" height="${sh}">`;
  for (let i = 0; i < n; i++) {
    const [r, g, b] = rgb8[i];
    s += `<rect x="${i * sw}" y="0" width="${sw}" height="${sh}" fill="rgb(${r},${g},${b})"/>`;
  }
  return `${s}</svg></div>`;
}

export function htmlReportHeader(meta) {
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>Palette study (WASM)</title>
<style>
  body { font-family: system-ui, sans-serif; background: #1a1a1a; color: #ddd; margin: 24px; }
  h1 { color: #fff; }
  .note { color: #aaa; line-height: 1.5; }
  .section { margin: 2rem 0; padding: 1rem; background: #252525; border-radius: 8px; }
  .card { display: flex; gap: 12px; align-items: flex-start; margin: 12px 0; padding: 10px; background: #2e2e2e; border-radius: 6px; }
  .meta { font-size: 0.85rem; color: #bbb; }
  .time { color: #f5b041; font-weight: 600; }
  .better { color: #7dcea0; }
  code { background: #333; padding: 2px 6px; border-radius: 4px; }
</style>
</head>
<body>
<h1>Palette study (WASM / npm)</h1>
<p class="note">${meta}</p>`;
}
