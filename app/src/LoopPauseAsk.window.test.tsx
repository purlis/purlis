// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { beginTriage, endTriage, landOnFirst } from "./asksTriage";
import { chatGroup } from "./test-inbox";
import type { Shown } from "./bindings";

/**
 * **The block loop's pause is an ask** (R-216, R-217, T1-36): a chat paused at 3 blocks on the
 * same target (T1-12) is in Asks as "steward 3 was blocked 3 times on the same thing. Let it keep
 * trying?", with Tell it to stop trying (the lead), Let it keep trying and Stop chat, each sent to
 * the core's pause. The ask is drawn as the core writes it: its words and its answers are read
 * from `asking/pauses.rs`, which builds nothing, so this fails when the core's ask changes alone.
 */

const PAUSES = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../src-tauri/src/asking/pauses.rs",
);
const source = readFileSync(PAUSES, "utf8");

/** The value of `const name: &str` in `pauses.rs`. */
function constant(name: string): string {
  const found = new RegExp(`const ${name}: &str = "([^"]*)";`).exec(source);
  expect(found, `pauses.rs has const ${name}`).not.toBeNull();
  return found?.[1] ?? "";
}

/** What `fn blocked_says` in `pauses.rs` says of `chat`, blocked `times` times. */
function blockedSays(chat: string, times: number): string {
  const found = /fn blocked_says\([^)]*\)[^{]*\{\s*format!\("([^"]*)"\)/.exec(source);
  expect(found, "pauses.rs has fn blocked_says").not.toBeNull();
  return (found?.[1] ?? "").replace("{chat}", chat).replace("{times}", String(times));
}

/** Chat 3's block-loop pause, as `pauses::paused` lists it. */
function blockLoop(): Shown {
  return {
    session: 3,
    ask: "paused:3",
    says: blockedSays("steward 3", 3),
    options: [
      {
        id: constant("STOP_TRYING"),
        label: constant("STOP_TRYING_SAID"),
        allows: false,
        preselected: true,
      },
      { id: constant("KEEP_TRYING"), label: constant("KEEP_TRYING_SAID"), allows: true },
      { id: constant("STOP"), label: constant("STOP_CHAT_SAID"), allows: false },
    ],
    source: "looping",
    chain: ["steward 3"],
    answer: { via: "paused" },
  };
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date", "performance"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  endTriage();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";

/** The person reads what is drawn for longer than an answer waits. */
const read = () => act(() => vi.advanceTimersByTime(SETTLE_MS + 1));

function core() {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    return null;
  });
  return calls;
}

/** Draws Asks holding the block-loop pause, and reads it. */
function drawn() {
  render(<Inbox part="asks" plane={PLANE} asks={[blockLoop()]} onGo={vi.fn()} onLeave={vi.fn()} />);
  read();
}

const answers = () =>
  within(within(chatGroup("steward 3")).getByRole("group", { name: "Answers to steward 3" }))
    .getAllByRole("button")
    .map((one) => one.textContent);

describe("the block loop's pause, in Asks (R-216, R-217)", () => {
  it("the loop pause ask names the chat and the count", () => {
    core();
    drawn();
    expect(
      within(chatGroup("steward 3")).getByText(
        "steward 3 was blocked 3 times on the same thing. Let it keep trying?",
      ),
    ).toBeVisible();
  });

  it("the loop pause ask offers Tell it to stop trying first, then Let it keep trying and Stop chat", async () => {
    const calls = core();
    drawn();
    expect(answers()).toEqual(["Tell it to stop trying", "Let it keep trying", "Stop chat"]);
    // The lead takes the keyboard as ⌘⇧I opens Asks (ADR 0121 decision 9).
    act(() => {
      beginTriage(document.body);
      landOnFirst(document.querySelector(".inbox-list") as HTMLElement);
    });
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Tell it to stop trying" }),
    );
    // Let it keep trying is the core's, which starts its block count again (R-218, T1-12).
    await userEvent.click(screen.getByRole("button", { name: "Let it keep trying" }));
    expect(calls.filter(({ cmd }) => cmd === "answer_pause")).toEqual([
      { cmd: "answer_pause", args: { plane: PLANE, session: 3, option: "keep-trying" } },
    ]);
  });

  it.each([
    ["Tell it to stop trying", "stop-trying"],
    ["Let it keep trying", "keep-trying"],
    ["Stop chat", "stop"],
  ])("sends %s to the core's pause as %s", async (label, option) => {
    const calls = core();
    drawn();
    await userEvent.click(screen.getByRole("button", { name: label }));
    expect(calls.filter(({ cmd }) => cmd === "answer_pause")).toEqual([
      { cmd: "answer_pause", args: { plane: PLANE, session: 3, option } },
    ]);
  });
});
