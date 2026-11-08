// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every forced-colours probe, found on disk** (#3481, `src/theme/forcedProbe.testkit.ts`).
 *
 * `specs/forced-colors.e2e.ts` runs under WebdriverIO, where `import.meta.glob` is not there, so
 * the probes are found by a glob of the source tree when the spec loads, and imported one by one:
 * the window's first, then each family's in its name's order. A family's pull request adds its
 * own file and touches no line here or in the spec.
 */
import { globSync } from "node:fs";
import { basename, dirname, join, relative, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import type { ForcedProbe } from "../src/theme/forcedProbe.testkit.ts";

/** The app's `src`, which the probes are under. */
export const SRC = join(dirname(fileURLToPath(import.meta.url)), "..", "src");

/** A probe, and whose it is: `window`, or the family's folder name. */
export interface NamedProbe {
  readonly name: string;
  readonly file: string;
  readonly probe: ForcedProbe;
}

/** The probe files under `src`, the window's first, then each family's by its name. */
export function forcedProbeFiles(src: string = SRC): string[] {
  const families = globSync("ui/*/forced-colors.probe.ts", { cwd: src })
    .map((path) => path.split(sep).join("/"))
    .sort();
  return ["theme/forced-colors.probe.ts", ...families].map((path) => join(src, path));
}

/** The name a probe file reads under: `window` for the theme's own, else its family's folder. */
export function probeName(file: string, src: string = SRC): string {
  const path = relative(src, file).split(sep).join("/");
  return path.startsWith("theme/") ? "window" : basename(dirname(path));
}

/** Every probe, loaded. */
export async function forcedProbes(src: string = SRC): Promise<NamedProbe[]> {
  return Promise.all(
    forcedProbeFiles(src).map(async (file) => ({
      name: probeName(file, src),
      file,
      probe: ((await import(pathToFileURL(file).href)) as { default: ForcedProbe }).default,
    })),
  );
}

/** The `data-p` names a probe's markup draws, in order. */
export function probed(markup: string): string[] {
  return [...markup.matchAll(/\bdata-p="([^"]*)"/g)].map((match) => match[1] ?? "");
}
