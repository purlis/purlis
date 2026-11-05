// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **The monospace every picture is drawn in** (#5402): the family the pinned Playwright image's
 * fontconfig names first for `monospace`, read once by the stories' Vitest projects
 * (`vitest.project.ts`) and handed to each page, which pins Chromium's own `monospace` to it
 * before a story renders (`screenshots.setup.ts`).
 *
 * Chromium left alone draws `monospace` (Purlis Classic's mono and terminal stacks end in it:
 * the image has none of `ui-monospace`, `Menlo`, `Consolas` or `SFMono-Regular`) in
 * fontconfig's answer, WenQuanYi Zen Hei Mono, until something earlier in the same renderer
 * process makes it Liberation Mono instead. Liberation Mono is wider and has a bold face, so a
 * line with inline code comes out a pixel taller and wraps elsewhere: ChatText's pictures
 * missed by one pixel in whichever cells shared a process with that earlier story. A family
 * named outright is found the same way every time, so the pinned one is.
 */
import { execFileSync } from "node:child_process";

/** Runs `command` with `args` and returns what it printed. */
export type Run = (command: string, args: string[]) => string;

const run: Run = (command, args) =>
  execFileSync(command, args, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });

/** The family fontconfig names first for `monospace` on this machine, or none where there is
 *  no fontconfig (macOS, Windows) or it names none. */
export function imageMonospace(ask: Run = run): string | undefined {
  let family: string;
  try {
    family = ask("fc-match", ["-f", "%{family[0]}", "monospace"]);
  } catch {
    return undefined;
  }
  return family.trim() || undefined;
}
