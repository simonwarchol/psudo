/**
 * AbortSignal coverage for worker-backed optimize (mock Worker, no WASM).
 * Run: node --test lib/npm/optimize-abort.test.mjs
 */
import { afterEach, describe, it } from "node:test";
import assert from "node:assert/strict";

const fixture = () => {
  const colors = new Uint16Array([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]);
  const locked = new Uint16Array([0, 0, 0, 0]);
  const intensities = new Uint16Array(4 * 8);
  const contrast = new Uint16Array([0, 65535, 0, 65535, 0, 65535, 0, 65535]);
  const luminance = new Uint16Array([60, 92]);
  return {
    colors,
    locked,
    intensities,
    contrast,
    luminance,
    excluded: [],
    names: ["", "", "", ""],
  };
};

function installMocks({ onNmRestart } = {}) {
  const workers = [];
  const messages = [];
  let warnCalls = 0;
  const originalWarn = console.warn;

  class MockWorker {
    constructor() {
      this.terminated = false;
      this.onmessage = null;
      this.onerror = null;
      workers.push(this);
    }

    postMessage(data) {
      if (this.terminated) return;
      messages.push({ worker: this, method: data.method, data });
      const { id, method } = data;

      if (method === "warmup") {
        queueMicrotask(() => {
          if (this.terminated || !this.onmessage) return;
          this.onmessage({ data: { id, ok: true, result: true } });
        });
        return;
      }

      if (method === "nmRestart") {
        if (onNmRestart?.(this, id)) return;
        queueMicrotask(() => {
          if (this.terminated || !this.onmessage) return;
          this.onmessage({
            data: {
              id,
              ok: true,
              result: {
                oklab: new Float32Array(12),
                total: 1,
                min_display_rgb_distance: 100,
                context_ms: 0,
                solver_ms: 0,
                polish_ms: 0,
                solver_objective_evaluations: 0,
                polish_objective_evaluations: 0,
              },
            },
          });
        });
        return;
      }

      if (method === "finalizePalette" || method === "optimize") {
        queueMicrotask(() => {
          if (this.terminated || !this.onmessage) return;
          this.onmessage({
            data: { id, ok: true, result: new Float32Array(12).fill(0.5) },
          });
        });
        return;
      }

      queueMicrotask(() => {
        if (this.terminated || !this.onmessage) return;
        this.onmessage({
          data: { id, ok: false, error: `unexpected method ${method}` },
        });
      });
    }

    terminate() {
      this.terminated = true;
    }
  }

  globalThis.window = globalThis.window || {};
  globalThis.Worker = MockWorker;
  console.warn = (...args) => {
    if (String(args[0]).includes("[psudo] parallel optimize failed")) {
      warnCalls += 1;
    }
    originalWarn(...args);
  };

  return {
    workers,
    messages,
    warnCount: () => warnCalls,
    restore() {
      console.warn = originalWarn;
      delete globalThis.Worker;
    },
  };
}

async function loadApi(tag) {
  return import(`./index.js?abort-test=${tag}`);
}

function runOptimize(api, f, signal) {
  return api.optimize(
    f.colors,
    f.locked,
    f.intensities,
    f.contrast,
    f.luminance,
    f.excluded,
    f.names,
    undefined,
    undefined,
    false,
    1,
    signal
  );
}

describe("optimize AbortSignal", () => {
  let mocks;

  afterEach(() => {
    mocks?.restore();
    mocks = null;
  });

  it("rejects immediately when signal is already aborted", async () => {
    mocks = installMocks();
    const api = await loadApi("before");
    api.setWorkerPoolSize(2);

    const controller = new AbortController();
    controller.abort();

    await assert.rejects(
      () => runOptimize(api, fixture(), controller.signal),
      (err) => err && err.name === "AbortError"
    );

    assert.equal(mocks.workers.length, 0);
    assert.equal(mocks.messages.length, 0);
  });

  it("aborts mid-wave without finalize; following optimize still works", async () => {
    let phase = "abort";
    let controller;
    mocks = installMocks({
      onNmRestart() {
        if (phase === "abort") {
          queueMicrotask(() => controller.abort());
          return true;
        }
        return false;
      },
    });

    const api = await loadApi("mid");
    api.setWorkerPoolSize(2);
    controller = new AbortController();
    const f = fixture();

    await assert.rejects(
      () => runOptimize(api, f, controller.signal),
      (err) => err && err.name === "AbortError"
    );

    assert.ok(
      mocks.messages.every((m) => m.method !== "finalizePalette"),
      "finalize must not run after abort"
    );
    assert.ok(
      mocks.workers.some((w) => w.terminated),
      "busy worker should be terminated"
    );
    assert.equal(
      mocks.warnCount(),
      0,
      "AbortError must not trigger single-worker fallback"
    );

    phase = "run";
    const result = await runOptimize(api, f);
    assert.ok(result instanceof Float32Array);
    assert.equal(result.length, 12);
    assert.ok(mocks.messages.some((m) => m.method === "finalizePalette"));
  });

  it("no-signal optimize resolves via finalizePalette", async () => {
    mocks = installMocks();
    const api = await loadApi("nosignal");
    api.setWorkerPoolSize(2);

    const result = await runOptimize(api, fixture());

    assert.ok(result instanceof Float32Array);
    assert.equal(result.length, 12);
    assert.ok(mocks.messages.some((m) => m.method === "finalizePalette"));
    assert.equal(mocks.warnCount(), 0);
  });
});
