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

const project = copyFixturePlane();
const folder = join(project, "a-folder-that-comes-back");

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

describe("retrying a project", () => {
  let plane = "";

  before(async () => {
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
    await browser.waitUntil(
      async () =>
        (await ask<{ name: string }[]>("chats_that_would_not_start", { plane })).length === 1,
      { timeoutMsg: "the chat should not have started without its folder" },
    );

    mkdirSync(folder, { recursive: true });
    // The chat is listed as short before its project's opening is over, and a Retry while the
    // project is still opening does nothing, by design (`Planes::retry_project`). So it is pressed
    // again until the opening is over and the Retry starts the chat.
    let retried: { started: { id: string; session: number }[] } = { started: [] };
    await browser.waitUntil(
      async () => {
        retried = await ask<{ started: { id: string; session: number }[] }>("retry_project", {
          plane,
        });
        return retried.started.length > 0;
      },
      { timeout: 20_000, interval: 500, timeoutMsg: "Retry never started the chat" },
    );

    expect(retried.started).toHaveLength(1);
    expect(await ask("chats_that_would_not_start", { plane })).toEqual([]);
    const open = await ask<{ session: number }[]>("opened_chats", { plane });
    expect(open.map((chat) => chat.session)).toContain(retried.started[0].session);
  });
});
