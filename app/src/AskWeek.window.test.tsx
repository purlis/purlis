// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AllChatsOf, fleetStore, type EveryChat } from "./AllChats";
import { waitSaid, weekSaid } from "./AskWeek";
import type { AskMeasure } from "./bindings";
import type { FleetChat } from "./everyChat";

/**
 * **Each project's week of asking** (ADR 0121 decision 18, #6021): All chats reads the week's
 * measures once as it opens and says them in a quiet line per project.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const CHAT: FleetChat = {
  plane: "/p/purlis",
  project: "purlis",
  session: 1,
  name: "chat 1",
  persona: null,
  workspace: "alpha",
  state: "working",
  word: "working",
  since: null,
  line: null,
  asks: [],
  task: false,
  stoppable: true,
};

function every(): EveryChat {
  const store = fleetStore();
  store.set([CHAT]);
  return {
    store,
    does: {
      go: vi.fn(),
      answer: vi.fn(),
      snooze: vi.fn(() => Promise.resolve(undefined)),
      stop: vi.fn(() => Promise.resolve(undefined)),
    },
    lentOf: () => undefined,
  };
}

const week = (more: Partial<AskMeasure>): AskMeasure => ({
  plane: "/p/purlis",
  asks: 0,
  median_secs: null,
  saved: 0,
  ...more,
});

describe("the week's line", () => {
  it("says the asks, the median wait and the asks saved", () => {
    expect(weekSaid(week({ asks: 12, median_secs: 150, saved: 3 }))).toBe(
      "12 asks this week · answered in 2 min at the median · 3 saved by rules",
    );
    expect(weekSaid(week({ asks: 1 }))).toBe("1 ask this week");
  });

  it("says a wait in words a person reads at a glance", () => {
    expect(waitSaid(20)).toBe("under a minute");
    expect(waitSaid(4 * 60 + 59)).toBe("4 min");
    expect(waitSaid(2 * 3600)).toBe("2 h");
    expect(waitSaid(2 * 3600 + 5 * 60)).toBe("2 h 5 min");
  });
});

describe("All chats (#6021)", () => {
  it("reads every project's week as it opens, and says each that has something to say", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      if (cmd === "ask_measures")
        return [
          week({ asks: 3, median_secs: 30 }),
          week({ plane: "/p/elsewhere/docs", saved: 2 }),
          week({ plane: "/p/quiet" }),
        ];
      return null;
    });
    render(<AllChatsOf every={every()} onFilter={vi.fn()} onLeave={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/3 asks this week · answered in under a minute/)).toBeTruthy(),
    );
    // A project with no chat open is named by its folder.
    expect(screen.getByText("docs")).toBeTruthy();
    expect(screen.getByText(/0 asks this week · 2 saved by rules/)).toBeTruthy();
    expect(screen.queryByText("quiet")).toBeNull();
    expect(asked.filter((cmd) => cmd === "ask_measures")).toHaveLength(1);
  });

  it("says nothing where the week cannot be read", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      if (cmd === "ask_measures") throw new Error("no data folder");
      return null;
    });
    render(<AllChatsOf every={every()} onFilter={vi.fn()} onLeave={vi.fn()} />);
    await waitFor(() => expect(asked).toContain("ask_measures"));
    expect(screen.queryByText(/this week/)).toBeNull();
  });
});
