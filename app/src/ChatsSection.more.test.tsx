// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
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
