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

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig(() => ({
  plugins: [react(), tailwind(), ownLicence()],
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
