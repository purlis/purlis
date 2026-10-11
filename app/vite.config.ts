// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwind from "@tailwindcss/vite";
import process from "node:process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";
import { generate, NOTICES_FILE, render } from "../tools/third-party-notices.mjs";

const host = process.env.TAURI_DEV_HOST;

/**
 * **purlis's own LICENSE and NOTICE, as `virtual:purlis-licence`**: the two files at the
 * repository's root, which the Licences view draws. Read here rather than imported with
 * `?raw`, which would mean opening the dev server's file access to the whole repository.
 */
function ownLicence(): Plugin {
  const id = "virtual:purlis-licence";
  const root = (file: string) => fileURLToPath(new URL(`../${file}`, import.meta.url));
  return {
    name: "purlis-own-licence",
    resolveId: (asked) => (asked === id ? `\0${id}` : undefined),
    load(loading) {
      if (loading !== `\0${id}`) return undefined;
      const files = ["LICENSE", "NOTICE"].map(root);
      for (const file of files) this.addWatchFile(file);
      const [licence, notice] = files.map((file) => readFileSync(file, "utf8"));
      return `export const LICENSE = ${JSON.stringify(licence)};\nexport const NOTICE = ${JSON.stringify(notice)};\n`;
    },
  };
}

/**
 * **Every third-party crate and npm package the build ships, as `virtual:purlis-third-party`**
 * (#1784): generated from the locked dependency graph when `vite build` starts
 * (`tools/third-party-notices.mjs`), so the notices are the graph this build was made from and
 * are never committed to go stale. The build fails when a shipped package has no licence text.
 * It also writes them beside the window as `THIRD-PARTY-NOTICES`, the file the release bundles
 * carry as a resource (`tauri.release.conf.json`). The dev server and vitest serve
 * `generated: false`: reading every crate's source is a build's work, not each test run's.
 */
function thirdPartyNotices(): Plugin {
  const id = "virtual:purlis-third-party";
  const repo = fileURLToPath(new URL("..", import.meta.url));
  let building = false;
  let notices: { packages: unknown[]; texts: string[] } | undefined;
  return {
    name: "purlis-third-party-notices",
    configResolved(config) {
      building = config.command === "build";
    },
    buildStart() {
      if (!building) return;
      const made = generate({ repo });
      if (made.missing.length > 0)
        this.error(
          `a shipped package has no licence text (#1784):\n${made.missing.join("\n")}\n` +
            "Add the licence's standard text under tools/licence-texts/, never leave the package out.",
        );
      notices = { packages: made.packages, texts: made.texts };
      this.emitFile({ type: "asset", fileName: NOTICES_FILE, source: render(made) });
    },
    resolveId: (asked) => (asked === id ? `\0${id}` : undefined),
    load(loading) {
      if (loading !== `\0${id}`) return undefined;
      const served = notices
        ? { generated: true, ...notices }
        : { generated: false, packages: [], texts: [] };
      return `export default ${JSON.stringify(served)};\n`;
    },
  };
}

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig(() => ({
  plugins: [react(), tailwind(), ownLicence(), thirdPartyNotices()],
  // Keep Rust errors visible, and give Tauri the fixed port it expects.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
}));
