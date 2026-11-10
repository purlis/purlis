// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AllChats, type FleetDoes } from "./AllChats";
import type { FleetChat } from "./everyChat";
import type { ChatState } from "./shownState";

/**
 * **All chats** (ADR 0121 decisions 18 and 21, #6013): every chat of every open project, Waiting
 * on you first, each with its actions, and several stopped or snoozed at once. Driven as the
 * person reads it: its rows, their words, their boxes and buttons.
 */

beforeEach(() => {
  mockIPC(() => null);
});

afterEach(() => {
  cleanup();
  clearMocks();
});

function chat(session: number, state: ChatState, more: Partial<FleetChat> = {}): FleetChat {
  return {
    plane: "/p/purlis",
    project: "purlis",
    session,
    name: `chat ${session}`,
    persona: null,
    workspace: "alpha",
    state,
    word: state,
    since: null,
    line: null,
    asks: [],
    task: false,
    stoppable: true,
    ...more,
  };
}

const CHATS: FleetChat[] = [
  chat(1, "working", { line: "running cargo" }),
  chat(2, "ended", { stoppable: false }),
  chat(3, "needs", { plane: "/p/docs", project: "docs", workspace: "site", asks: ["q3"] }),
  chat(4, "stuck"),
];

function draw(filter?: ChatState) {
  const does: FleetDoes = {
    go: vi.fn(),
    answer: vi.fn(),
    snooze: vi.fn(() => Promise.resolve(undefined)),
    stop: vi.fn(() => Promise.resolve(undefined)),
  };
  const onFilter = vi.fn();
  const onLeave = vi.fn();
  render(
    <div style={{ height: 800 }}>
      <AllChats chats={CHATS} filter={filter} onFilter={onFilter} onLeave={onLeave} does={does} />
    </div>,
  );
  return { does, onFilter, onLeave };
}

const list = () => screen.getByRole("grid", { name: "All chats" });
const rows = () => within(list()).getAllByRole("row");
const row = (name: string) => within(list()).getByRole("row", { name: new RegExp(`^${name},`) });

describe("All chats lists every chat of every open project (#6013)", () => {
  it("Waiting on you first, then Stuck?, then the rest", () => {
    draw();
    expect(rows().map((one) => one.getAttribute("data-state"))).toEqual([
      "needs",
      "stuck",
      "working",
      "ended",
    ]);
  });

  it("says each chat's state by name, its project and workspace, and its last line", () => {
    draw();
    expect(row("chat 3")).toHaveTextContent("Waiting on you · docs · site");
    expect(row("chat 1")).toHaveTextContent("Working · purlis · alpha");
    expect(row("chat 1")).toHaveTextContent("running cargo");
  });

  it("goes to a chat its row is pressed for, in whichever project it is", async () => {
    const { does } = draw();
    await userEvent.click(within(row("chat 3")).getByRole("button", { name: "Go to chat chat 3" }));
    expect(does.go).toHaveBeenCalledWith(expect.objectContaining({ plane: "/p/docs", session: 3 }));
  });

  it("offers Answer only on a chat that waits on you, and the actions in the one order", () => {
    draw();
    const actions = (name: string) =>
      within(row(name))
        .getAllByRole("button")
        .map((one) => one.textContent)
        .filter((words) => ["Answer", "Go to chat", "Snooze", "Stop"].includes(words ?? ""));
    expect(actions("chat 3")).toEqual(["Answer", "Go to chat", "Snooze", "Stop"]);
    expect(actions("chat 1")).toEqual(["Go to chat", "Snooze", "Stop"]);
  });

  it("is left by its switch", async () => {
    const { onLeave } = draw();
    await userEvent.click(screen.getByRole("button", { name: "All chats", pressed: true }));
    expect(onLeave).toHaveBeenCalled();
  });
});

describe("several chats at once (#6013)", () => {
  it("stops the chosen chats that still run, asked once first", async () => {
    const { does } = draw();
    await userEvent.click(within(row("chat 1")).getByRole("checkbox"));
    await userEvent.click(within(row("chat 2")).getByRole("checkbox"));
    const chosen = screen.getByRole("group", { name: "Chosen chats" });
    await waitFor(() => expect(chosen).toHaveTextContent("2 chats chosen"));

    await userEvent.click(within(chosen).getByRole("button", { name: "Stop" }));
    expect(does.stop).not.toHaveBeenCalled();
    // Asked once, in the view, of the one that still runs.
    const asked = screen.getByRole("group", { name: "Stop chats" });
    expect(asked).toHaveTextContent("Stop chat 1?");
    await userEvent.click(within(asked).getByRole("button", { name: "Stop" }));

    expect(does.stop).toHaveBeenCalledWith([expect.objectContaining({ session: 1 })]);
    await waitFor(() => expect(screen.queryByRole("group", { name: "Chosen chats" })).toBeNull());
  });

  it("snoozes the chosen chats that ask something", async () => {
    const { does } = draw();
    await userEvent.click(within(row("chat 3")).getByRole("checkbox"));
    await userEvent.click(within(row("chat 4")).getByRole("checkbox"));
    const chosen = screen.getByRole("group", { name: "Chosen chats" });
    await userEvent.click(within(chosen).getByRole("button", { name: "Snooze" }));
    expect(does.snooze).toHaveBeenCalledWith([expect.objectContaining({ session: 3 })]);
  });

  it("says what went wrong, and keeps the chats chosen", async () => {
    const { does } = draw();
    vi.mocked(does.snooze).mockResolvedValue("The core is not answering.");
    await userEvent.click(within(row("chat 3")).getByRole("checkbox"));
    const chosen = screen.getByRole("group", { name: "Chosen chats" });
    await userEvent.click(within(chosen).getByRole("button", { name: "Snooze" }));
    expect(await screen.findByText("The core is not answering.")).toBeInTheDocument();
    expect(within(row("chat 3")).getByRole("checkbox")).toBeChecked();
  });
});

describe("a row's own Stop (#6013)", () => {
  it("asks once in the view, then stops that chat", async () => {
    const { does } = draw();
    await userEvent.click(within(row("chat 4")).getByRole("button", { name: "Stop chat 4" }));
    const asked = screen.getByRole("group", { name: "Stop chats" });
    expect(asked).toHaveTextContent("Stop chat 4? It ends the program it runs.");
    await userEvent.click(within(asked).getByRole("button", { name: "Cancel" }));
    expect(does.stop).not.toHaveBeenCalled();

    await userEvent.click(within(row("chat 4")).getByRole("button", { name: "Stop chat 4" }));
    await userEvent.click(
      within(screen.getByRole("group", { name: "Stop chats" })).getByRole("button", {
        name: "Stop",
      }),
    );
    expect(does.stop).toHaveBeenCalledWith([expect.objectContaining({ session: 4 })]);
  });
});

describe("filtered to one state, as a link from Asks asks (#6020's seam)", () => {
  it("lists only that state, and Show all takes the filter off", async () => {
    const { onFilter } = draw("working");
    expect(rows().map((one) => one.getAttribute("data-state"))).toEqual(["working"]);
    await userEvent.click(screen.getByRole("button", { name: "Show all" }));
    expect(onFilter).toHaveBeenCalledWith(undefined);
  });
});
