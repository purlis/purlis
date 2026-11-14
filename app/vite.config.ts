// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vitest/config" />
import { defineConfig, normalizePath } from "vite";
import react from "@vitejs/plugin-react";
import tailwind from "@tailwindcss/vite";
import process from "node:process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";
import { generate, NOTICES_FILE, render } from "../tools/third-party-notices.mjs";
import { withTheFamilies } from "./src/ui/stylesheets.ts";
import { keyedOnClasses } from "./src/ui/partClasses.ts";

const host = process.env.TAURI_DEV_HOST;

/** The source the window's JSX is compiled against: `src/ui/jsx-runtime.ts`, by its alias. */
const PARTS_JSX = "@purlis/parts";

/** A path in this folder, as Vite resolves it. */
const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));

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

/**
 * **Every family's stylesheet, in `styles.css`'s `@families` line** (`src/ui/stylesheets.ts`):
 * each `src/ui/<family>/<family>.css` that `styles.css` does not import by name, by name, so a
 * pull request that adds a family adds no line there; and each family's `forced-colors.css` in
 * its `@forced` line's place. Before Tailwind, which reads the imports.
 */
function familyStylesheets(): Plugin {
  const ui = fileURLToPath(new URL("src/ui", import.meta.url));
  const styles = normalizePath(fileURLToPath(new URL("src/styles.css", import.meta.url)));
  /** A family's own stylesheet, `src/ui/<family>/<family>.css`, or its forced-colours sheet. */
  const isFamilySheet = (file: string) => {
    const [family, sheet, ...rest] = normalizePath(file)
      .slice(normalizePath(ui).length + 1)
      .split("/");
    return rest.length === 0 && (sheet === `${family}.css` || sheet === "forced-colors.css");
  };
  return {
    name: "purlis-family-stylesheets",
    enforce: "pre",
    // A family added or taken away while the dev server runs: styles.css is built again.
    configureServer(server) {
      server.watcher.add(ui);
      const changed = (file: string) => {
        if (!normalizePath(file).startsWith(`${normalizePath(ui)}/`) || !isFamilySheet(file))
          return;
        for (const module of server.moduleGraph.getModulesByFile(styles) ?? [])
          server.moduleGraph.invalidateModule(module);
        server.ws.send({ type: "full-reload" });
      };
      server.watcher.on("add", changed);
      server.watcher.on("unlink", changed);
    },
    transform(code, id) {
      if (normalizePath(id.split("?")[0]) !== styles) return undefined;
      return { code: withTheFamilies(code, ui), map: null };
    },
  };
}

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig(async ({ command }) => ({
  plugins: [
    react({ jsxImportSource: PARTS_JSX }),
    familyStylesheets(),
    tailwind(),
    ownLicence(),
    thirdPartyNotices(),
  ],
  // Each part's classes beside its attributes, and every rule keyed on them (#4351,
  // `src/ui/partClasses.ts`): the window's JSX runs through React's runtime by way of ours, and
  // its CSS is keyed on the classes once it is built.
  resolve: {
    alias: [
      { find: `${PARTS_JSX}/jsx-runtime`, replacement: here("src/ui/jsx-runtime.ts") },
      { find: `${PARTS_JSX}/jsx-dev-runtime`, replacement: here("src/ui/jsx-dev-runtime.ts") },
      // React's profiling build of the renderer, so a shipped window's `Profiler` still says
      // what each region's commits cost and a stall can name it (`src/slowLog.ts`, felt speed
      // round 4, H9). The production build drops the profiler's timing; this one keeps it, at
      // a few clock reads a component, and is otherwise the production build. `vite dev` and the
      // tests draw with the development build, which keeps it already.
      ...(command === "build"
        ? [{ find: /^react-dom\/client$/, replacement: "react-dom/profiling" }]
        : []),
    ],
  },
  css: { postcss: { plugins: [keyedOnClasses()] } },
  // Keep Rust errors visible, and give Tauri the fixed port it expects.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  // Two pages: the window, and the theme specimen a hidden window photographs (#2330).
  build: {
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("index.html", import.meta.url)),
        specimen: fileURLToPath(new URL("specimen.html", import.meta.url)),
      },
    },
  },
  // Two kinds of test (#2901): `unit`, every `*.test.ts(x)` in jsdom (`npm test`), and
  // `stories`, every story run in real Chromium and WebKit in each cell of the theme matrix
  // (`npm run test:stories`, `.storybook/vitest.project.ts`, `docs/component-stories.md`).
  test: {
    projects: [
      {
        extends: true,
        // xterm.js's key-to-bytes mapping, read by `keys/harnessBytes.test.ts` from the sources
        // the package ships, names its modules by the paths its own build resolves (`common/…`).
        resolve: {
          alias: [
            {
              find: /^common\/(.*)$/,
              replacement: `${fileURLToPath(new URL("./node_modules/@xterm/xterm/src/common/", import.meta.url))}$1`,
            },
          ],
        },
        test: {
          name: "unit",
          environment: "jsdom",
          setupFiles: ["./src/test-setup.ts"],
          // **The budget a test is given, not a wait for anything** (with `test-setup.ts`'s
          // `asyncUtilTimeout`). A test that draws the whole window (`render(<App />)`, every
          // `*.window.test.tsx`) takes 1–3 s on an idle machine and past Vitest's default 5 s on
          // a runner whose cores are all busy with the other workers; one shared budget here
          // stands in for each file raising its own.
          testTimeout: 20_000,
        },
      },
      // Loaded only when Vitest reads this file: `vite dev` and `vite build` never load
      // Storybook.
      ...(process.env.VITEST ? (await import("./.storybook/vitest.project.ts")).stories() : []),
    ],
  },
}));
