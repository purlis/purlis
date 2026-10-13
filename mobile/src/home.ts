// Home's order (iOS design §6.1): what needs you first, across every desktop, then each
// desktop's chats by workspace, as the desktop's own list has them.

import type { Chat, DesktopRow } from "./ipc.ts";

/** A desktop and what it listed, as Home takes it. */
export type Listed = { desktop: DesktopRow; chats: Chat[] };

export type NeedsYou = { device: string; desktop: string; chat: Chat };

export type Workspace = { name: string | null; chats: Chat[] };
export type Group = { device: string; desktop: DesktopRow; workspaces: Workspace[] };

/** The state words a row shows, in the window's case. */
export const STATE_WORDS: Readonly<Record<string, string>> = {
  working: "Working",
  needs_you: "Needs you",
  idle: "Idle",
  ended: "Ended",
};

export function stateWord(state: string): string {
  return STATE_WORDS[state] ?? state;
}

/** Every chat that needs you, across every desktop, in the desktops' order. */
export function needsYou(listed: readonly Listed[]): NeedsYou[] {
  return listed.flatMap(({ desktop, chats }) =>
    chats
      .filter((chat) => chat.state === "needs_you")
      .map((chat) => ({ device: desktop.device, desktop: desktop.name, chat })),
  );
}

/** Each desktop's chats by workspace: the project's root first, then the workspaces by name;
 *  within one, the chats as the desktop listed them. */
export function grouped(listed: readonly Listed[]): Group[] {
  return listed.map(({ desktop, chats }) => {
    const byWorkspace = new Map<string | null, Chat[]>();
    for (const chat of chats) {
      const name = chat.workspace ?? null;
      const inIt = byWorkspace.get(name) ?? [];
      inIt.push(chat);
      byWorkspace.set(name, inIt);
    }
    const names = [...byWorkspace.keys()].sort((a, b) =>
      a === null ? -1 : b === null ? 1 : a.localeCompare(b),
    );
    return {
      device: desktop.device,
      desktop,
      workspaces: names.map((name) => ({ name, chats: byWorkspace.get(name) ?? [] })),
    };
  });
}

/** A chat's key across desktops: a chat id is unique within its desktop. */
export function keyOf(device: string, chat: string): string {
  return `${device}/${chat}`;
}

/** What a chat row is called: its title, or its id's tail for one with none. */
export function titleOf(chat: Chat): string {
  return chat.title ?? chat.chat.slice(-6);
}

/** The project a chat is in, as a person knows it: its folder's name. The protocol's
 *  `project` is the stable id from `charter.toml`, which is nothing to read. */
export function projectOf(chat: Chat): string {
  const path = chat.project_path?.replace(/[/\\]+$/, "");
  const name = path?.split(/[/\\]/).pop();
  return name || chat.project;
}
