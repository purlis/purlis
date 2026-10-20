// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";

import type { PlaneId, WindowDraft } from "./bindings";
import {
  CLOSE_WAITS_IN_THE_WINDOW,
  draftsOfClosing,
  settleClosingDrafts,
  type DraftsClient,
} from "./closingDrafts";
import { commands as linkCommands } from "./uiRpc";
import { WINDOW_ONLY_SAID, windowOnly, windowOnlyResult } from "./uiRpcLink";

/** What a close does about its chats' theme drafts (#2332, #2708). */

const PLANE = "/home/dev/plane" as PlaneId;

const draft = (folder: string, session: number): WindowDraft => ({
  folder,
  session,
  chat: `steward ${session}`,
  workspace: "alpha",
  kept_to: "workspace",
  replaces: false,
  changed: false,
  drawn: true,
  broken: [],
  refused: [],
  review: null,
  kept_refused: false,
});

/** The window's client, answering each chat's drafts and refusing a Keep of `refusedFolder`. */
function windowClient(
  drafts: Record<number, WindowDraft[]>,
  refusedFolder?: string,
): DraftsClient & { asked: string[] } {
  const asked: string[] = [];
  return {
    asked,
    themeDraftsOfChat: (_plane, session) => {
      asked.push(`drafts ${session}`);
      return Promise.resolve({ status: "ok", data: drafts[session] ?? [] });
    },
    themeDraftKeep: (_plane, _session, folder) => {
      asked.push(`keep ${folder}`);
      return Promise.resolve(
        folder === refusedFolder
          ? { status: "error", error: `${folder} changed since you were shown it` }
          : { status: "ok", data: `${folder} is kept` },
      );
    },
    themeDraftRevert: (_plane, _session, folder) => {
      asked.push(`revert ${folder}`);
      return Promise.resolve({ status: "ok", data: null });
    },
    handDraftsCloseToTheWindow: (_plane, sessions) => {
      asked.push(`hand ${sessions.join(" ")}`);
      return Promise.resolve({ status: "ok", data: false });
    },
  };
}

/** A link's client, as `uiRpc.ts` is one: the drafts are the window's, and the core answers
 *  whether it handed the close of `sessions` to the window by `handed`. */
function linkClient(handed: boolean): DraftsClient & { asked: string[] } {
  const asked: string[] = [];
  return {
    asked,
    themeDraftsOfChat: windowOnlyResult,
    themeDraftKeep: windowOnlyResult,
    themeDraftRevert: windowOnly,
    handDraftsCloseToTheWindow: (_plane, sessions) => {
      asked.push(`hand ${sessions.join(" ")}`);
      return Promise.resolve({ status: "ok", data: handed });
    },
  };
}

describe("the theme drafts a close asks about", () => {
  it("are every closing chat's, in the window", async () => {
    const client = windowClient({ 3: [draft("dusk", 3)], 4: [draft("dawn", 4)] });
    const found = await draftsOfClosing(client, PLANE, [3, 4]);
    expect(found).toEqual({ ok: true, drafts: [draft("dusk", 3), draft("dawn", 4)] });
  });

  it("are not asked on a link, whose close is refused in a window-only command's words", async () => {
    const found = await draftsOfClosing(linkCommands, PLANE, [3]);
    expect(found).toEqual({ ok: false, refused: WINDOW_ONLY_SAID });
  });

  it("on a link, hand a close of chats with drafts to the window, which settles them", async () => {
    // #2761: the link is told the close waits in the window, and keeps or reverts nothing.
    const client = linkClient(true);
    const found = await draftsOfClosing(client, PLANE, [3, 4]);
    expect(found).toEqual({ ok: false, handed: CLOSE_WAITS_IN_THE_WINDOW });
    expect(client.asked).toEqual(["hand 3 4"]);
  });

  it("on a link, refuse a close the core did not hand, in a window-only command's words", async () => {
    const client = linkClient(false);
    const found = await draftsOfClosing(client, PLANE, [3]);
    expect(found).toEqual({ ok: false, refused: WINDOW_ONLY_SAID });
  });

  it("in the window, are asked of the core and never handed", async () => {
    const client = windowClient({ 3: [draft("dusk", 3)] });
    await draftsOfClosing(client, PLANE, [3]);
    expect(client.asked).toEqual(["drafts 3"]);
  });

  it("hold the close when purlis cannot tell", async () => {
    const client: DraftsClient = {
      ...windowClient({}),
      themeDraftsOfChat: () => Promise.reject(new Error("gone")),
    };
    const found = await draftsOfClosing(client, PLANE, [3]);
    expect(found.ok).toBe(false);
  });
});

describe("keeping a closing chat's drafts", () => {
  it("asks who draws each one kept, even when a later Keep is refused", async () => {
    const client = windowClient({}, "dawn");
    const settled = await settleClosingDrafts(
      client,
      PLANE,
      [draft("dusk", 3), draft("dawn", 4)],
      true,
    );
    expect(settled.kept).toEqual([{ folder: "dusk", workspace: "alpha", keptTo: "workspace" }]);
    expect(settled.refused).toBe("dawn changed since you were shown it");
  });

  it("asks nothing of who draws a reverted one", async () => {
    const client = windowClient({});
    const settled = await settleClosingDrafts(client, PLANE, [draft("dusk", 3)], false);
    expect(settled).toEqual({ kept: [] });
    expect(client.asked).toEqual(["revert dusk"]);
  });
});
