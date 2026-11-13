// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import {
  ASKS_SCOPES,
  asksScopeDrawn,
  asksWiderWaiting,
  forgetAsksScope,
  inChatOf,
  keptAsksScope,
  pickAsksScope,
  scopeCounts,
} from "./asksScope";
import { forgetProjectViews, keptFacet } from "./projectViews";

afterEach(() => {
  forgetAsksScope("/p");
  forgetAsksScope("/q");
  forgetProjectViews();
});

/**
 * Chat 8 dispatched 12 and 13; 12 dispatched 20; 20 dispatched 21. Chat 30 is a sibling of 8,
 * dispatched by 1, and 40 stands alone.
 */
const PARENT: Record<number, number> = { 12: 8, 13: 8, 20: 12, 21: 20, 30: 1, 8: 1 };
const askedBy = (session: number) => PARENT[session];

describe("Chat's set (ADR 0121, amended 2026-11-12, decision 2)", () => {
  it("holds the focused chat and every chat it dispatched, three deep", () => {
    const inChat = inChatOf(8, askedBy);
    expect([8, 12, 13, 20, 21].filter(inChat)).toEqual([8, 12, 13, 20, 21]);
  });

  it("leaves out a sibling, the chat above and a chat of no lineage", () => {
    const inChat = inChatOf(8, askedBy);
    expect([30, 1, 40].filter(inChat)).toEqual([]);
  });

  it("is empty with no chat focused", () => {
    const inChat = inChatOf(undefined, askedBy);
    expect([8, 12, 1].filter(inChat)).toEqual([]);
  });

  it("ends at a lineage that loops, holding none of it but what is below the focus", () => {
    const loop: Record<number, number> = { 2: 3, 3: 2 };
    const inChat = inChatOf(8, (session) => loop[session]);
    expect(inChat(2)).toBe(false);
    expect(inChat(8)).toBe(true);
  });
});

describe("the chips' counts", () => {
  const asks = [8, 12, 21, 30, 50].map((session) => ({ session }));
  /** Chats 8, 12, 21 and 30 are filed in alpha; 50 in beta. */
  const inHere = ({ session }: { session: number }) => session !== 50;
  const inChat = ({ session }: { session: number }) => inChatOf(8, askedBy)(session);

  it("counts Chat, Workspace and Project, nested", () => {
    expect(scopeCounts(asks, { inChat, inHere })).toEqual({ chat: 3, workspace: 4, project: 5 });
  });

  it("has no Workspace where the project has one workspace", () => {
    expect(scopeCounts(asks, { inChat })).toEqual({ chat: 3, project: 5 });
  });

  it("counts in Workspace a task of the focused chat filed in another workspace", () => {
    const filed = ({ session }: { session: number }) => session !== 21;
    expect(scopeCounts(asks, { inChat, inHere: filed })).toEqual({
      chat: 3,
      workspace: 5,
      project: 5,
    });
  });

  it("counts Chat 0 with no chat focused", () => {
    expect(scopeCounts(asks, { inChat: () => false, inHere }).chat).toBe(0);
  });

  it("holds Chat ≤ Workspace ≤ Project, whatever the focus and the strip", () => {
    for (const focus of [undefined, 1, 8, 12, 30, 50]) {
      for (const here of [() => true, inHere, () => false]) {
        const counts = scopeCounts(asks, {
          inChat: ({ session }) => inChatOf(focus, askedBy)(session),
          inHere: here,
        });
        expect(counts.chat).toBeLessThanOrEqual(counts.workspace ?? counts.project);
        expect(counts.workspace ?? 0).toBeLessThanOrEqual(counts.project);
      }
    }
  });
});

describe("the scopes Asks offers", () => {
  it("are Chat, Workspace and Project, narrowest first", () => {
    expect(ASKS_SCOPES.map((one) => one.says)).toEqual(["Chat", "Workspace", "Project"]);
  });

  it("draws the pick where it can be listed", () => {
    expect(asksScopeDrawn("chat", { chat: 0, workspace: 2, project: 3 }, true)).toBe("chat");
    expect(asksScopeDrawn("workspace", { chat: 1, workspace: 2, project: 3 }, true)).toBe(
      "workspace",
    );
  });

  it("draws Workspace for Chat with no chat focused, or Project with one workspace", () => {
    expect(asksScopeDrawn("chat", { chat: 0, workspace: 2, project: 3 }, false)).toBe("workspace");
    expect(asksScopeDrawn("chat", { chat: 0, project: 3 }, false)).toBe("project");
  });

  it("draws Project for a Workspace not offered", () => {
    expect(asksScopeDrawn("workspace", { chat: 1, project: 3 }, true)).toBe("project");
  });

  it("names the nearest wider scope with something waiting, Chat to Workspace to Project", () => {
    expect(asksWiderWaiting("chat", { chat: 0, workspace: 2, project: 3 }, true)).toEqual({
      scope: "workspace",
      count: 2,
    });
    expect(asksWiderWaiting("chat", { chat: 0, workspace: 0, project: 3 }, true)).toEqual({
      scope: "project",
      count: 3,
    });
    expect(asksWiderWaiting("chat", { chat: 0, project: 3 }, true)).toEqual({
      scope: "project",
      count: 3,
    });
    expect(asksWiderWaiting("project", { chat: 0, project: 0 }, true)).toBeUndefined();
  });
});

describe("the pick", () => {
  it("is Chat until the person picks another, in each project apart", () => {
    expect(keptAsksScope("/p")).toBe("chat");
    pickAsksScope("/p", "project");
    expect(keptAsksScope("/p")).toBe("project");
    expect(keptAsksScope("/q")).toBe("chat");
  });

  it("is Chat again once Asks closes", () => {
    pickAsksScope("/p", "workspace");
    forgetAsksScope("/p");
    expect(keptAsksScope("/p")).toBe("chat");
  });

  it("is never kept under inbox.scope", () => {
    pickAsksScope("/p", "project");
    expect(keptFacet("/p", "inbox")).toBeUndefined();
  });
});
