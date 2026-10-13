import { describe, expect, it } from "vitest";

import { grouped, keyOf, needsYou, projectOf, stateWord, titleOf, type Listed } from "./home.ts";
import type { Chat, DesktopRow } from "./ipc.ts";

const mac: DesktopRow = {
  device: "mac",
  name: "Mac",
  addresses: ["10.0.0.2:7777"],
  standing: "connected",
  why: null,
  generation: 1,
};
const linux: DesktopRow = {
  device: "linux",
  name: "Linux box",
  addresses: ["10.0.0.3:7777"],
  standing: "reconnecting",
  why: null,
  generation: 0,
};

function chat(id: string, state: string, workspace: string | null = null): Chat {
  return { chat: id, project: "purlis", state, workspace, title: `Chat ${id}`, level: 2 };
}

const listed: Listed[] = [
  {
    desktop: mac,
    chats: [chat("a", "working", "ios"), chat("b", "needs_you"), chat("c", "idle", "api")],
  },
  { desktop: linux, chats: [chat("d", "needs_you", "zed"), chat("e", "ended")] },
];

describe("needsYou", () => {
  it("lists every chat that needs you, across desktops, in the desktops' order", () => {
    expect(needsYou(listed).map((one) => [one.device, one.desktop, one.chat.chat])).toEqual([
      ["mac", "Mac", "b"],
      ["linux", "Linux box", "d"],
    ]);
  });

  it("is empty when nothing needs you", () => {
    expect(needsYou([{ desktop: mac, chats: [chat("a", "working")] }])).toEqual([]);
  });
});

describe("grouped", () => {
  it("groups each desktop's chats by workspace, the root first, then by name", () => {
    const groups = grouped(listed);
    expect(groups.map((group) => group.desktop.name)).toEqual(["Mac", "Linux box"]);
    expect(groups[0].workspaces.map((ws) => [ws.name, ws.chats.map((c) => c.chat)])).toEqual([
      [null, ["b"]],
      ["api", ["c"]],
      ["ios", ["a"]],
    ]);
    expect(groups[1].workspaces.map((ws) => ws.name)).toEqual([null, "zed"]);
  });

  it("keeps a desktop with no chats, with no workspaces", () => {
    expect(grouped([{ desktop: mac, chats: [] }])[0].workspaces).toEqual([]);
  });
});

describe("words", () => {
  it("keys a chat by its desktop, since ids are unique per desktop", () => {
    expect(keyOf("mac", "a")).toBe("mac/a");
    expect(keyOf("linux", "a")).not.toBe(keyOf("mac", "a"));
  });

  it("names a chat by its title, or its id's tail", () => {
    expect(titleOf(chat("a", "idle"))).toBe("Chat a");
    expect(titleOf({ chat: "01HZX4Y8ABCDEF", project: "p", state: "idle" })).toBe("ABCDEF");
  });

  it("names a project by its folder, since its id is nothing to read", () => {
    const row = { chat: "a", project: "01HZX4Y8ABCDEF", state: "idle" };
    expect(projectOf({ ...row, project_path: "/Users/me/code/purlis" })).toBe("purlis");
    expect(projectOf({ ...row, project_path: "/Users/me/code/purlis/" })).toBe("purlis");
    expect(projectOf(row)).toBe("01HZX4Y8ABCDEF");
  });

  it("speaks the state in the window's words", () => {
    expect(stateWord("needs_you")).toBe("Needs you");
    expect(stateWord("working")).toBe("Working");
    expect(stateWord("odd")).toBe("odd");
  });
});
