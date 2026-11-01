// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#4477): builds the Skills view prototype into one self-contained HTML file.
 * From app/: npx vite build --config src/prototype-skills-plugin-off/vite.standalone.config.ts
 */
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import base from "../../vite.config";
import { handedThemes } from "../../.storybook/handed.plugin";

const here = dirname(fileURLToPath(import.meta.url));
const out = process.env.SP_OUT ?? resolve(here, "../../../../skills-plugin-off-dist");

/** Puts the script and the stylesheet into the page. */
function oneFile(): Plugin {
  return {
    name: "prototype-one-file",
    enforce: "post",
    generateBundle(_, bundle) {
      const page = Object.values(bundle).find(
        (one) => one.type === "asset" && one.fileName.endsWith(".html"),
      );
      if (!page || page.type !== "asset") return;
      let html = String(page.source);
      for (const [name, one] of Object.entries(bundle)) {
        if (one.type === "asset" && name.endsWith(".css")) {
          html = html.replace(/<link[^>]*rel="stylesheet"[^>]*>/, "");
          html = html.replace("</head>", () => `<style>${String(one.source)}</style></head>`);
          delete bundle[name];
        }
        if (one.type === "chunk") {
          html = html.replace(/<script[^>]*type="module"[^>]*><\/script>/, "");
          const code = one.code.replace(/<\/script/g, "<\\/script");
          html = html.replace("</body>", () => `<script type="module">${code}</script></body>`);
          delete bundle[name];
        }
      }
      page.source = html;
    },
  };
}

export default defineConfig(async (env) => {
  const given = await (base as unknown as (e: typeof env) => Promise<{ plugins: Plugin[] }>)(env);
  return {
    root: here,
    plugins: [...given.plugins, handedThemes(), oneFile()],
    cacheDir: resolve(out, ".vite"),
    build: {
      outDir: out,
      emptyOutDir: true,
      assetsInlineLimit: 100_000_000,
      cssCodeSplit: false,
      minify: true,
      rollupOptions: {
        input: resolve(here, "standalone.html"),
        output: { inlineDynamicImports: true },
      },
    },
  };
});
