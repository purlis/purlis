// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { copyFixturePlane } from "../harness.js";

/**
 * **A project's Retry brings its chats back** (#5324), in the built app: the real registry and
 * the real start of a real program. A chat whose folder has gone does not come back with its
 * project; the folder returns, and Retry starts it.
 *
 * It is driven through the window's own `invoke`, for `opener.e2e.ts`'s reason: this run's window
 * holds the launch's project, and what the Retry button does in the window is pinned in
 * `src/ProjectsLoad.window.test.tsx`. The project is closed at the end, as it found the window.
 */


async function ask<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const answer = await browser.executeAsync(
    (
      name: string,
      passed: Record<string, unknown>,
      done: (out: { ok?: unknown; trouble?: string }) => void,
    ) => {
      void window.__TAURI__.core
        .invoke(name, passed)
        .then((ok) => done({ ok }))
        .catch((e: unknown) => done({ trouble: String(e) }));
    },
    command,
    args,
  );
  if (answer.trouble !== undefined) throw new Error(`${command} refused: ${answer.trouble}`);
  return answer.ok as T;
}

/** The planes the core has said are done opening (`project-ready`), heard from now on. */
async function hearProjectsReady(): Promise<void> {
  await browser.executeAsync((done: () => void) => {
    const heard: string[] = [];
    (window as unknown as { projectsReady: string[] }).projectsReady = heard;
    void window.__TAURI__.event
      .listen<{ plane: string }>("project-ready", (event) => heard.push(event.payload.plane))
      .then(() => done());
  });
}

async function projectsReady(): Promise<string[]> {
  return browser.execute(
    () => (window as unknown as { projectsReady?: string[] }).projectsReady ?? [],
  );
}

// TEMPORARY (#5460 evidence, reverted before merge): the whole scenario, 25 rounds in a row.
for (let round = 1; round <= 25; round++) {
const project = copyFixturePlane();
const folder = join(project, "a-folder-that-comes-back");
describe(`retrying a project, round ${round}`, () => {
  let plane = "";

  before(async () => {
    // Heard before the project opens, so its word that the opening is over cannot be missed.
    await hearProjectsReady();
    rmSync(folder, { recursive: true, force: true });
    mkdirSync(join(project, ".charter", "app"), { recursive: true });
    // `reopen`'s own on-disk shape, spelled exactly (`opener.e2e.ts`).
    writeFileSync(
      join(project, ".charter", "app", "reopen.json"),
      JSON.stringify({
        version: 1,
        at: 0,
        chats: [
          {
            program: "/bin/sh",
            args: ["-c", "sleep 600"],
            cwd: folder,
            name: "comes back",
            resume: "",
            active: true,
            profile: "",
            persona: "",
            footer: "",
          },
        ],
      }),
    );
    const opened = await ask<{ plane: string | null; ask: { contributes: unknown } | null }>(
      "open_plane",
      { path: project },
    );
    plane =
      opened.plane ??
      (await ask<string>("approve_plane", {
        path: project,
        contributes: opened.ask?.contributes,
      }));
  });

  after(async () => {
    await ask("close_plane", { plane });
  });

  it("starts the chat its project was short of, once what stopped it is put right", async () => {
    // A Retry while the project is still opening does nothing, by design
    // (`Planes::retry_project`), and its chat is listed as short before the opening is over
    // (#5460). So the press waits on the core's word that the opening is over, not on time.
    await browser.waitUntil(async () => (await projectsReady()).includes(plane), {
      timeoutMsg: "the project never said it had finished opening",
    });
    expect(await ask<{ name: string }[]>("chats_that_would_not_start", { plane })).toHaveLength(1);

    mkdirSync(folder, { recursive: true });
    const retried = await ask<{ started: { id: string; session: number }[] }>("retry_project", {
      plane,
    });

    expect(retried.started).toHaveLength(1);
    expect(await ask("chats_that_would_not_start", { plane })).toEqual([]);
    const open = await ask<{ session: number }[]>("opened_chats", { plane });
    expect(open.map((chat) => chat.session)).toContain(retried.started[0].session);
  });
});
}
