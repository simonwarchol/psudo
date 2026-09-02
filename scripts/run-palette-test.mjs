#!/usr/bin/env node

import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputPath = process.env.PALETTE_TEST_OUTPUT
  ? path.resolve(root, process.env.PALETTE_TEST_OUTPUT)
  : path.join(root, "lib/target/palette-test.json");
const chromePath =
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => resolve(address.port));
    });
  });
}

async function waitForJson(url, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return response.json();
    } catch {
      // Server or browser is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Timed out waiting for ${url}`);
}

async function waitForServer(url, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // Server is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Timed out waiting for ${url}`);
}

function connectCdp(webSocketDebuggerUrl) {
  const socket = new WebSocket(webSocketDebuggerUrl);
  const pending = new Map();
  let nextId = 1;

  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    const request = pending.get(message.id);
    if (!request) return;
    pending.delete(message.id);
    if (message.error) request.reject(new Error(message.error.message));
    else request.resolve(message.result);
  });

  const ready = new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });

  return {
    async send(method, params = {}) {
      await ready;
      const id = nextId++;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        socket.send(JSON.stringify({ id, method, params }));
      });
    },
    close() {
      socket.close();
    },
  };
}

async function terminate(child) {
  if (!child || child.exitCode !== null) return;
  child.kill("SIGTERM");
  await Promise.race([
    once(child, "exit"),
    new Promise((resolve) => setTimeout(resolve, 2_000)),
  ]);
}

async function main() {
  const serverPort = await freePort();
  const debugPort = await freePort();
  const userDataDir = await mkdtemp(path.join(os.tmpdir(), "psudo-palette-"));
  const workers = process.env.PALETTE_TEST_WORKERS;
  if (workers && (!Number.isInteger(Number(workers)) || Number(workers) < 1)) {
    throw new TypeError("PALETTE_TEST_WORKERS must be a positive integer.");
  }
  const query = new URLSearchParams();
  if (workers) query.set("workers", workers);
  const palettes = process.env.PALETTE_TEST_PALETTES;
  if (
    palettes &&
    (!Number.isInteger(Number(palettes)) || Number(palettes) < 1)
  ) {
    throw new TypeError("PALETTE_TEST_PALETTES must be a positive integer.");
  }
  if (palettes) query.set("palettes", palettes);
  const restarts = process.env.PALETTE_TEST_RESTARTS;
  if (
    restarts &&
    (!Number.isInteger(Number(restarts)) || Number(restarts) < 1)
  ) {
    throw new TypeError("PALETTE_TEST_RESTARTS must be a positive integer.");
  }
  if (restarts) query.set("restarts", restarts);
  const maxIters = process.env.PALETTE_TEST_MAX_ITERS;
  if (
    maxIters &&
    (!Number.isInteger(Number(maxIters)) || Number(maxIters) < 1)
  ) {
    throw new TypeError("PALETTE_TEST_MAX_ITERS must be a positive integer.");
  }
  if (maxIters) query.set("maxIters", maxIters);
  const seedStart = process.env.PALETTE_TEST_SEED_START;
  if (seedStart && !Number.isInteger(Number(seedStart))) {
    throw new TypeError("PALETTE_TEST_SEED_START must be an integer.");
  }
  if (seedStart) query.set("seedStart", seedStart);
  if (process.env.PALETTE_TEST_PROFILED === "0") query.set("profiled", "0");
  if (process.env.PALETTE_TEST_RESTART_POLISH === "0") {
    query.set("restartPolish", "0");
  }
  const queryString = query.size > 0 ? `?${query}` : "";
  const pageUrl = `http://127.0.0.1:${serverPort}/palette-test.html${queryString}`;
  const server = spawn(
    "pnpm",
    ["exec", "vite", "--host", "127.0.0.1", "--port", String(serverPort)],
    { cwd: root, stdio: ["ignore", "pipe", "pipe"] },
  );
  let chrome;
  let cdp;

  try {
    await waitForServer(`http://127.0.0.1:${serverPort}/`);
    chrome = spawn(
      chromePath,
      [
        "--headless=new",
        "--disable-gpu",
        "--disable-background-timer-throttling",
        "--disable-renderer-backgrounding",
        "--no-first-run",
        "--no-default-browser-check",
        `--remote-debugging-port=${debugPort}`,
        `--user-data-dir=${userDataDir}`,
        pageUrl,
      ],
      { stdio: ["ignore", "ignore", "pipe"] },
    );

    const pages = await waitForJson(
      `http://127.0.0.1:${debugPort}/json/list`,
      30_000,
    );
    const page = pages.find((candidate) => candidate.url === pageUrl);
    if (!page) throw new Error(`Benchmark page not found at ${pageUrl}`);
    cdp = connectCdp(page.webSocketDebuggerUrl);

    let completed = 0;
    while (true) {
      const evaluated = await cdp.send("Runtime.evaluate", {
        expression:
          "({result: window.__PALETTE_BENCHMARK__ ?? null, error: window.__PALETTE_BENCHMARK_ERROR__ ?? null, status: document.querySelector('#status')?.textContent ?? ''})",
        returnByValue: true,
      });
      const state = evaluated.result.value;
      if (state.error) throw new Error(state.error);
      if (state.result) {
        await mkdir(path.dirname(outputPath), { recursive: true });
        await writeFile(outputPath, `${JSON.stringify(state.result, null, 2)}\n`);
        console.log(
          `[palette-test] ${state.result.config.palettes} palettes in ${(state.result.totalMs / 1000).toFixed(2)}s`,
        );
        console.log(`[palette-test] wrote ${outputPath}`);
        return;
      }
      const match = /Completed (\d+)\//.exec(state.status);
      const nextCompleted = match ? Number(match[1]) : 0;
      if (nextCompleted > completed) {
        completed = nextCompleted;
        console.log(`[palette-test] ${state.status}`);
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  } finally {
    cdp?.close();
    await terminate(chrome);
    await terminate(server);
    await rm(userDataDir, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 200,
    });
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
