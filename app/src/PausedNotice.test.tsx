// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { PausedNotice } from "./PausedNotice";
import { SETTINGS_LINK, type SettingsLinkAsk } from "./settings/links";
import type { Shown } from "./bindings";

/**
 * A chat paused at its budget, or as looping, is a Blocking ask on its own tab too (#6015, ADR
 * 0121 decision 20): what it was paused for, and the answers that settle it. Continue, It's fine
 * and Stop are the core's; Raise the budget opens Settings where the limit is and answers its ask.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const SESSION = 7;

const budget: Shown = {
  session: SESSION,
  ask: `paused:${SESSION}`,
  says:
    "Budget reached: It had worked 30 minutes and a task may work 30 minutes here (minutes " +
    "per task: working time only, not time waiting on the person or on its own tasks). The " +
    "person sets that limit in Settings › Project › Dispatch.",
  options: [
    { id: "continue", label: "Continue", allows: true },
    { id: "raise", label: "Raise the budget", allows: true },
    { id: "stop", label: "Stop", allows: false },
  ],
  source: "budget",
  chain: ["steward 2", "#7 check prod"],
  answer: { via: "paused" },
  since: 1_000,
};

const looping: Shown = {
  ...budget,
  says: "It may be looping: the same command failed 6 times in a row.",
  options: [
    { id: "fine", label: "It's fine, continue", allows: true },
    { id: "stop", label: "Stop", allows: false },
  ],
  source: "looping",
};

/** A core that records what the window sends. */
function core() {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    return null;
  });
  return () => asked.filter((one) => one.cmd !== "pending_asks");
}

describe("a chat paused into an ask, on its tab", () => {
  it("says what it reached and offers Continue, Raise the budget and Stop", () => {
    const sent = core();
    render(<PausedNotice plane={PLANE} session={SESSION} asks={[budget]} />);
    const notice = screen.getByRole("status", { name: "Budget reached" });
    expect(notice.textContent).toContain("Budget reached: It had worked 30 minutes");
    expect(notice.textContent).toContain("It is paused until you answer.");
    expect(screen.getAllByRole("button").map((one) => one.textContent)).toEqual([
      "Continue",
      "Raise the budget",
      "Stop",
    ]);
    expect(sent()).toEqual([]);
  });

  it("sends Continue to the core as the person's answer to the pause", async () => {
    const sent = core();
    const onWay = vi.fn();
    render(<PausedNotice plane={PLANE} session={SESSION} asks={[budget]} onWay={onWay} />);
    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    await vi.waitFor(() =>
      expect(sent()).toContainEqual({
        cmd: "answer_pause",
        args: { plane: PLANE, session: SESSION, option: "continue" },
      }),
    );
    expect(onWay).toHaveBeenCalledWith("Continue", budget);
  });

  it("opens Settings at the dispatch limits on Raise the budget, and answers its ask", async () => {
    const sent = core();
    const linked: SettingsLinkAsk[] = [];
    const heard = (event: Event) => linked.push((event as CustomEvent<SettingsLinkAsk>).detail);
    window.addEventListener(SETTINGS_LINK, heard);
    try {
      render(<PausedNotice plane={PLANE} session={SESSION} asks={[budget]} />);
      await userEvent.click(screen.getByRole("button", { name: "Raise the budget" }));
      await vi.waitFor(() =>
        expect(linked).toEqual([
          {
            plane: PLANE,
            link: { group: "project.dispatch", setting: "project.dispatch.limits" },
          },
        ]),
      );
      // Q261: the ask is answered, so the Chat log never leaves it Not answered; the chat stays
      // paused until a look finds it within the new limit.
      await vi.waitFor(() =>
        expect(sent().filter((one) => one.cmd === "answer_pause")).toEqual([
          { cmd: "answer_pause", args: { plane: PLANE, session: SESSION, option: "raise" } },
        ]),
      );
    } finally {
      window.removeEventListener(SETTINGS_LINK, heard);
    }
  });

  it("offers It's fine, continue and Stop for a loop, and sends Stop", async () => {
    const sent = core();
    render(<PausedNotice plane={PLANE} session={SESSION} asks={[looping]} />);
    expect(screen.getByRole("status", { name: "Paused" }).textContent).toContain(
      "It may be looping: the same command failed 6 times in a row.",
    );
    expect(screen.getAllByRole("button").map((one) => one.textContent)).toEqual([
      "It's fine, continue",
      "Stop",
    ]);
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    await vi.waitFor(() =>
      expect(sent()).toContainEqual({
        cmd: "answer_pause",
        args: { plane: PLANE, session: SESSION, option: "stop" },
      }),
    );
  });

  it("draws nothing for another chat's pause", () => {
    core();
    const { container } = render(
      <PausedNotice plane={PLANE} session={SESSION + 1} asks={[budget]} />,
    );
    expect(container.textContent).toBe("");
  });
});
