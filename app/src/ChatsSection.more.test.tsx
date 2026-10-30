// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { setChatsListPrefs } from "./chatsListPrefs";
import { stateClock } from "./stateClock";
import { chatsTree, type ListedChat } from "./chatsTree";
import { DISPATCHES_SHOW, type Offer } from "./actions";

afterEach(cleanup);

/** One chat, a top-level one open in a tab. */
const CHAT: ListedChat = {
  session: 1,
  name: "steward 1",
  persona: "steward",
  workspace: "alpha",
  shell: false,
  parent: null,
  mode: null,
  from: null,
  tab: true,
  branch: null,
  report: null,
  outcome: null,
  asking: null,
  harness: "Claude Code",
};

/** A catalogue row, as `actions.ts` offers one: available, or off with its reason. */
function offer(id: string, title: string, reason = ""): Offer {
  return { id, title, available: reason === "", reason, does: { verb: "newShell" } } as Offer;
}

function draw(offers: Offer[]) {
  const pressed: string[] = [];
  render(
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <ChatsSection
        rows={chatsTree([CHAT])}
        front={1}
        onOpen={() => {}}
        offers={new Map(offers.map((one) => [one.id, one]))}
        onPress={(one) => pressed.push(one.id)}
      />
    </ChatsHere.Provider>,
  );
  return pressed;
}

describe("the Chats title line's More (#3342)", () => {
  it("lists the catalogue's rows for chats here, and runs the one chosen", async () => {
    const pressed = draw([
      offer("shell.new", "New shell"),
      offer(DISPATCHES_SHOW, "Open dispatches"),
      offer("settings.show", "Settings…"),
    ]);

    await userEvent.click(screen.getByRole("button", { name: "More" }));
    const menu = await screen.findByRole("menu", { name: "More for chats" });
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.textContent),
    ).toEqual(["New shell", "Open dispatches", "Settings…"]);

    await userEvent.click(within(menu).getByRole("menuitem", { name: "New shell" }));
    expect(pressed).toEqual(["shell.new"]);
  });

  it("says why a row it cannot run is off, under its words, and runs nothing", async () => {
    const pressed = draw([offer("shell.new", "New shell", "No workspace is focused.")]);

    await userEvent.click(screen.getByRole("button", { name: "More" }));
    const menu = await screen.findByRole("menu", { name: "More for chats" });
    const row = within(menu).getByRole("menuitem", { name: /New shell/ });
    expect(row).toHaveAttribute("aria-disabled", "true");
    expect(row).toHaveTextContent("No workspace is focused.");

    await userEvent.click(row);
    expect(pressed).toEqual([]);
  });

  it("is not drawn where the catalogue offers none of its rows", () => {
    draw([]);
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
  });
});

/** Chats in two workspaces: 1 and 2 at work in alpha, 3 asking for the person in beta. */
const THREE: ListedChat[] = [
  CHAT,
  { ...CHAT, session: 2, name: "steward 2", tab: false },
  { ...CHAT, session: 3, name: "devops 3", persona: "devops", workspace: "beta", tab: false },
];
const AT_WORK: ChatStates = {
  ...nothingKnown,
  bySession: { 1: "running", 2: "running", 3: "waiting" },
  needsYou: [3],
};

describe("the Chats view's prototype details (#3484)", () => {
  afterEach(() => {
    vi.useRealTimers();
    act(() => setChatsListPrefs({ grouped: false }));
  });

  it("counts each chip's chats, the count out of the chip's name and said once", () => {
    render(
      <ChatsHere.Provider value={fixedChats(AT_WORK)}>
        <ChatsSection rows={chatsTree(THREE)} front={1} onOpen={() => {}} />
      </ChatsHere.Provider>,
    );

    const needs = screen.getByRole("checkbox", { name: "needs you" });
    const working = screen.getByRole("checkbox", { name: "working" });
    expect(needs.closest("label")?.querySelector('[data-part="count"]')).toHaveTextContent("1");
    expect(needs).toHaveAccessibleDescription("1");
    expect(working.closest("label")?.querySelector('[data-part="count"]')).toHaveTextContent("2");
  });

  it("heads a workspace's chats with its name and count, as the frame heads a group", () => {
    setChatsListPrefs({ grouped: true });
    render(
      <ChatsHere.Provider value={fixedChats(AT_WORK)}>
        <ChatsSection rows={chatsTree(THREE)} front={1} onOpen={() => {}} />
      </ChatsHere.Provider>,
    );

    const groups = [...document.querySelectorAll(".chats-group")].map((one) => [
      one.querySelector(".chats-group-name")?.textContent,
      one.querySelector("[data-count]")?.textContent,
    ]);
    expect(groups).toEqual([
      ["alpha", "2"],
      ["beta", "1"],
    ]);
    // Still no row of the tree: it counts each workspace's chats on its own.
    expect(screen.getAllByRole("treeitem")).toHaveLength(3);
  });

  it("ends a chat's row with the time it came into its state, quietly", () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const at = new Date(2026, 9, 27, 10, 42);
    const clock = stateClock();
    // Chat 1 was there before the window looked; chat 2 arrived while it was open.
    clock.read(AT_WORK, chatsTree([CHAT]), at.getTime() - 60_000);
    clock.read(AT_WORK, chatsTree(THREE.slice(0, 2)), at.getTime());
    const drawnAt = (ms: number) => {
      vi.setSystemTime(at.getTime() + ms);
      cleanup();
      render(
        <ChatsHere.Provider value={fixedChats(AT_WORK)}>
          <ChatsSection
            rows={chatsTree(THREE.slice(0, 2))}
            front={1}
            onOpen={() => {}}
            clock={clock}
          />
        </ChatsHere.Provider>,
      );
      const time = (name: string) =>
        screen.getByRole("treeitem", { name: new RegExp(`^${name}`) }).querySelector(".chat-time");
      return { one: time("steward 1"), two: time("steward 2") };
    };

    expect(drawnAt(20_000).two).toHaveTextContent(/^now$/);
    // Seen and not heard: the row's card says how long in words.
    expect(drawnAt(20_000).two).toHaveAttribute("aria-hidden", "true");
    expect(drawnAt(150_000).two).toHaveTextContent(/^2m$/);
    expect(drawnAt(2 * 3_600_000).two).toHaveTextContent(/^10:42$/);
    // A chat the window did not see come into its state says no time.
    expect(drawnAt(20_000).one).toBeNull();
  });
});

describe("the restarts' status beside the tree (#3886)", () => {
  /** A task of chat 1's, with no tab, which a restart can say something of. */
  const TASK: ListedChat = {
    ...CHAT,
    session: 2,
    name: "devops 2",
    persona: "devops",
    parent: 1,
    mode: "task",
    tab: false,
  };

  const drawn = (restarts: Record<number, string>) => (
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <ChatsSection
        rows={chatsTree([CHAT, TASK])}
        front={1}
        onOpen={() => {}}
        restarts={restarts}
      />
    </ChatsHere.Provider>
  );

  it("is mounted before a restart says anything, and only its text changes", () => {
    const { rerender } = render(drawn({}));
    const region = screen.getByRole("status", { name: "Restarts" });
    expect(region).toHaveTextContent(/^$/);
    // Additions are announced, not every line again.
    expect(region).toHaveAttribute("aria-atomic", "false");

    rerender(drawn({ 2: "It is restarted when its turn ends." }));
    expect(screen.getByRole("status", { name: "Restarts" })).toBe(region);
    // Led by its chat, where the words do not name it.
    expect(region).toHaveTextContent("Restart of devops 2. It is restarted when its turn ends.");

    rerender(drawn({ 2: "Not restarted: devops 2 ended while the restart was asked." }));
    expect(screen.getByRole("status", { name: "Restarts" })).toBe(region);
    // Words that name it already are said as they are.
    expect(region).toHaveTextContent(
      /^Not restarted: devops 2 ended while the restart was asked\.$/,
    );

    rerender(drawn({}));
    expect(screen.getByRole("status", { name: "Restarts" })).toBe(region);
    expect(region).toHaveTextContent(/^$/);
  });

  // The review of #3935: a line built from the rows drawn came again as a fold, the filter or
  // the scope drew its row again, and was announced again as if it were new.
  it("keeps a restart's line as it is while its row is folded away, filtered away and back", async () => {
    render(drawn({ 2: "It is restarted when its turn ends." }));
    const region = screen.getByRole("status", { name: "Restarts" });
    const line = region.querySelector("p");
    expect(line).not.toBeNull();
    const still = () => {
      expect(region.querySelectorAll("p")).toHaveLength(1);
      expect(region.querySelector("p")).toBe(line);
    };
    const tree = screen.getByRole("tree", { name: "Chats of this project" });
    const steward = within(tree)
      .getAllByRole("treeitem")
      .find((one) => one.querySelector(".session")?.textContent === "steward 1") as HTMLElement;

    // Folded, and opened again.
    act(() => steward.focus());
    await userEvent.keyboard("{ArrowLeft}");
    expect(within(tree).getAllByRole("treeitem")).toHaveLength(1);
    still();
    await userEvent.keyboard("{ArrowRight}");
    expect(within(tree).getAllByRole("treeitem")).toHaveLength(2);
    still();

    // Filtered away, and the filter cleared.
    const filter = screen.getByRole("searchbox", { name: /^Filter chats/ });
    await userEvent.type(filter, "steward");
    still();
    await userEvent.clear(filter);
    still();
  });

  it("is still mounted while the scope lists no chat", () => {
    render(
      <ChatsHere.Provider value={fixedChats(nothingKnown)}>
        <ChatsSection rows={[]} onOpen={() => {}} />
      </ChatsHere.Provider>,
    );
    expect(screen.getByRole("status", { name: "Restarts" })).toHaveTextContent(/^$/);
  });
});
