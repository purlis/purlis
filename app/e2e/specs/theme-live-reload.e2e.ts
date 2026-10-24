// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $$, browser, expect } from "@wdio/globals";
import { focusWorkspace, projectRoot } from "../acting.js";
import { ask } from "../switching.js";
import { workspacesListed } from "../titleBarWorkspaces.js";

/**
 * **Saving a theme folder repaints the window** (#2324, spec #2313), against the real app: a
 * theme folder at each level — the person's own on this machine, the project's at the plane
 * root, and a workspace's — is chosen, saved, and seen drawn, in the same window.
 *
 * The engine's tests hold what is watched, what a change asks for, how a run of saves is paced
 * (`theme/reload_tests.rs`), and that a save is held to the consent floors and the font
 * approvals as any theme drawn is. What only the real app can say is that the platform's watcher
 * hears the save, the window is told, and what it then draws is the saved file's.
 *
 * **It leaves the window as it found it**: each choice is taken back and each folder removed.
 */

const WORKSPACE = "alpha";
/** A save is drawn within a second (`reload::QUIET_FOR` and `reload::GAP`); a busy runner's
 *  file events can be late, so the spec waits longer than that and says how long it took. */
const DRAWN_WITHIN = 10_000;

async function untilTheStripIsRead(): Promise<void> {
  await browser.waitUntil(
    async () =>
      (await $$('[data-testid="title-bar"] [data-titlebar-all]').getElements()).length > 0,
    { timeout: 30_000, interval: 250, timeoutMsg: "the strip never listed the fixture plane" },
  );
}

/**
 * The folder the person's own theme folders are kept in on this run's machine, **as the core
 * reads it now**: the config home the window was handed its theme file from, and in it the
 * `purlis` folder when there is one, else `charter` (`names::CONFIG_HOME`: the purlis spelling
 * wins once it is there). Asked at each use, since an earlier spec may have made the `purlis`
 * folder after the window was created.
 */
async function machineFolder(): Promise<string> {
  const path = await browser.execute(() => {
    const handed = (window as unknown as Record<string, unknown>).__CHARTER_AT_CREATION__ as {
      theme?: { path?: string };
    };
    return handed?.theme?.path ?? "";
  });
  expect(path).not.toBe("");
  const home = dirname(dirname(path));
  const purlis = join(home, "purlis");
  return existsSync(purlis) && statSync(purlis).isDirectory() ? purlis : join(home, "charter");
}

/** What happened at each step, for a failure's message. */
const steps: string[] = [];

/** Records `step`: the colour drawn, and what the core says the window draws there. */
async function note(step: string, plane: string): Promise<void> {
  let said: string;
  try {
    const theme = await ask<{ drawn?: unknown; level?: unknown; folders?: unknown }>(
      "project_theme",
      { plane, workspace: WORKSPACE },
    );
    said = JSON.stringify({ drawn: theme.drawn, level: theme.level, folders: theme.folders });
  } catch (why) {
    said = `project_theme refused: ${String(why)}`;
  }
  const line = `${step}: draws ${await surfaceDrawn()}; the core says ${said}`;
  steps.push(line);
  console.log(`theme-live-reload: ${line}`);
}

/** The workspace in front by its name, or `null` at the plane root (#2916: the title bar's ⌄). */
async function workspaceInFront(): Promise<string | null> {
  const front = (await workspacesListed()).find((one) => one.selected);
  return front === undefined || front.name === "Plane root" ? null : front.name;
}

/** Back to the plane root, by its row in the title bar's ⌄. */
async function focusThePlaneRoot(): Promise<void> {
  await focusWorkspace("Plane root");
}

/**
 * The surface the `save`th save of the `level`th level draws: a dark grey beside Purlis
 * Classic's own, which its text, its marks and its asks all clear, so no consent floor draws
 * purlis's own surface in its place. Each level and each save its own.
 */
function surfaceOf(level: number, save: number): string {
  return `#1${7 + save}1${7 + save}1${level}`;
}

/** What the window draws `surface.base` in now. */
async function surfaceDrawn(): Promise<string> {
  return browser.execute(() =>
    document.documentElement.style.getPropertyValue("--surface-base").trim().toLowerCase(),
  );
}

/** A theme folder's `theme.json`, whose one appearance draws `surface.base` in `surface`. */
function save(folder: string, surface: string): void {
  mkdirSync(folder, { recursive: true });
  writeFileSync(
    join(folder, "theme.json"),
    JSON.stringify({
      name: "Live reload",
      appearances: [{ id: "dark", appearance: "dark", tokens: { "surface.base": surface } }],
    }),
  );
}

/** Saves `surface` into `folder` and waits for the window to draw it; answers how long it took. */
async function savedAndDrawn(folder: string, surface: string, plane: string): Promise<number> {
  const from = Date.now();
  save(folder, surface);
  await browser
    .waitUntil(async () => (await surfaceDrawn()) === surface, {
      timeout: DRAWN_WITHIN,
      interval: 50,
    })
    .catch(async () => {
      await note(`never drew ${surface}`, plane);
      throw new Error(
        `the window never drew ${surface} after ${folder} was saved; it draws ${await surfaceDrawn()}. Steps: ${steps.join(" | ")}`,
      );
    });
  await note(`drew ${surface}`, plane);
  return Date.now() - from;
}

describe("a theme folder in use", () => {
  it("repaints the window when it is saved, at the machine, the project and the workspace, with no new window", async () => {
    await untilTheStripIsRead();
    const plane = await projectRoot();
    const was = await workspaceInFront();
    await focusWorkspace(WORKSPACE);
    const windows = await browser.getWindowHandles();
    const levels = [
      { level: "machine", themes: join(await machineFolder(), "themes") },
      { level: "project", themes: join(plane, "themes") },
      { level: "workspace", themes: join(plane, "workspaces", WORKSPACE, "themes") },
    ];
    const made: string[] = [];
    try {
      for (const [n, { level, themes }] of levels.entries()) {
        const name = `live-reload-${level}`;
        const folder = join(themes, name);
        made.push(folder);
        save(folder, surfaceOf(n, 0));
        await note(`${level}: ${name} written`, plane);
        // The person's choice for this workspace on this machine reaches a folder of every
        // level. A save then tells the window to ask again, and the choice with it.
        await ask("theme_choose", { plane, workspace: WORKSPACE, theme: name, appearance: null });
        await note(`${level}: ${name} chosen`, plane);
        const first = await savedAndDrawn(folder, surfaceOf(n, 1), plane);
        const again = await savedAndDrawn(folder, surfaceOf(n, 2), plane);
        console.log(`theme-live-reload: ${level} saved and drawn in ${first} ms, then ${again} ms`);
      }
      expect(await browser.getWindowHandles()).toEqual(windows);
    } finally {
      await ask("theme_choose", { plane, workspace: WORKSPACE, theme: null, appearance: null });
      for (const folder of made) rmSync(folder, { recursive: true, force: true });
      if (was === null) await focusThePlaneRoot();
      else await focusWorkspace(was);
    }
  });
});
