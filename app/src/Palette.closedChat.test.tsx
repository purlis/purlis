// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Palette } from "./Palette";
import type { Offer, Ran } from "./actions";
import type { ChatCard, ChatLookup, PlaneId } from "./bindings";

/**
 * **The switcher's finished row** (#6214): a number typed in the palette that names a closed or
 * finished chat finds its row, and the row opens its card. *Read it* opens its kept screen's
 * view tab, and *Reopen* reopens its task. The core is mocked here; its lookup is
 * `chatlookup`'s own tests'.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/projects/alpha" as unknown as PlaneId;

const CARD: ChatCard = {
  number: 641,
  chat: "01K6W0RKER000000000000000B",
  started_by: "steward 4",
  screen_kept: true,
  task: {
    id: "01K9DISPATCH0000000000000A",
    asker: 0,
    name: "Signed commits #5607",
    chat: null,
    persona: null,
    how: "done",
    outcome: "done",
    folds: true,
    report: "Done.",
    changed: null,
    ended: "2026-11-11T11:08:00",
    place: "smart-ide",
    branch: null,
    reopens: true,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
  },
};

function core(answer: (number: number) => ChatLookup) {
  const asked: number[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "chat_by_number") {
      const { number } = args as { number: number };
      asked.push(number);
      return answer(number);
    }
    return null;
  });
  return asked;
}

async function open() {
  await userEvent.keyboard("{F2}");
  return screen.findByRole("dialog", { name: "Command palette" });
}

const ready = (id: string, title: string) =>
  ({ id, title, available: true, reason: "", does: { verb: "nothing" } }) satisfies Offer;

describe("a closed chat found by its number in the palette", () => {
  it("finds the finished row by 641, opens its card, and Read it opens its screen's tab", async () => {
    const asked = core((n) => (n === 641 ? { is: "ended", card: CARD } : { is: "no_chat" }));
    const ran: Offer[] = [];
    const onRun = vi.fn((offer: Offer): Ran => {
      ran.push(offer);
      return { ok: true };
    });
    render(
      <Palette offers={[ready("chat.new", "New tab")]} chats={{ plane: PLANE }} onRun={onRun} />,
    );
    const palette = await open();
    await userEvent.keyboard("641");
    const row = await within(palette).findByRole("option", {
      name: /Show closed chat #641 · Signed commits #5607/,
    });
    expect(asked).toContain(641);
    await userEvent.click(row);
    const card = await screen.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    expect(onRun).not.toHaveBeenCalled();
    await userEvent.click(within(card).getByRole("button", { name: "Read it" }));
    await waitFor(() => expect(onRun).toHaveBeenCalledTimes(1));
    expect(ran[0].does).toEqual({
      verb: "openView",
      view: { from: null, view: "chat-screen", key: CARD.chat },
      title: "#641 · Signed commits #5607 (read-only)",
    });
  });

  it("Reopen on the card reopens its task", async () => {
    core(() => ({ is: "ended", card: CARD }));
    const ran: Offer[] = [];
    render(
      <Palette
        offers={[]}
        chats={{ plane: PLANE }}
        onRun={(offer) => {
          ran.push(offer);
          return { ok: true };
        }}
      />,
    );
    const palette = await open();
    await userEvent.keyboard(`#${641}`);
    await userEvent.click(
      await within(palette).findByRole("option", { name: /Show closed chat #641/ }),
    );
    const card = await screen.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    await userEvent.click(within(card).getByRole("button", { name: "Reopen" }));
    await waitFor(() => expect(ran).toHaveLength(1));
    expect(ran[0].does).toEqual({ verb: "reopenTask", id: CARD.task.id });
  });

  it("Escape takes the card down first, and the palette stays", async () => {
    core(() => ({ is: "ended", card: CARD }));
    render(<Palette offers={[]} chats={{ plane: PLANE }} onRun={() => ({ ok: true })} />);
    const palette = await open();
    await userEvent.keyboard("641");
    await userEvent.click(
      await within(palette).findByRole("option", { name: /Show closed chat #641/ }),
    );
    await screen.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "#641 · Signed commits #5607" })).toBeNull(),
    );
    expect(screen.getByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
  });

  it("a number no chat had adds no row, and words are never looked up", async () => {
    const asked = core(() => ({ is: "no_chat" }));
    render(<Palette offers={[]} chats={{ plane: PLANE }} onRun={() => ({ ok: true })} />);
    const palette = await open();
    await userEvent.keyboard("tab");
    await userEvent.clear(within(palette).getByRole("combobox"));
    await userEvent.keyboard("9999");
    await waitFor(() => expect(asked).toContain(9999));
    expect(within(palette).queryByRole("option", { name: /Show closed chat/ })).toBeNull();
    expect(asked.every((n) => Number.isInteger(n))).toBe(true);
  });
});
