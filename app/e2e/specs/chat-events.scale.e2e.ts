// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { appendFileSync } from "node:fs";
import process from "node:process";
import { browser } from "@wdio/globals";
import { closeEverything, openTab, sleep } from "../bench/window.js";
import { built, singleQuoted } from "../harness.js";
import { nextLoad } from "../load.js";

/**
 * **How many chat events reach the window a second while busy chats stream** (#4883, for
 * #4746's third criterion). Each chat's fake harness streams and, four times a second, fires a
 * real `PreToolUse` hook from inside its chat (`--tick-hook`), a `Read` of a new file: what a
 * working chat's tool calls tell the window, its doing and its touch. The window counts the
 * `chat-events` it is sent, and what they carry.
 *
 * Run twice by the nightly scale run: as the app batches (#4746, one event about every 100 ms)
 * and with `PURLIS_E2E_CHAT_EVENTS_MS=0`, each telling sent as it comes, which is the window's
 * load without the batching. Evidence only: rows of `$PURLIS_SCALE_OUT`, and nothing asserted
 * about the numbers but that the chats told the window something at all.
 */

const CHATS = 10;
const MEASURED_MS = 30_000;
const out = process.env.PURLIS_SCALE_OUT;
const os = process.platform === "darwin" ? "macos" : "linux";
const unbatched = process.env.PURLIS_E2E_CHAT_EVENTS_MS === "0";

/** The hook a tick fires: the payload and `CLAUDE_PID` a Claude Code's, as `harness.ts`'s are. */
const aReadFromInsideTheChat = [
  `printf '{"session_id":"%s","hook_event_name":"PreToolUse","tool_name":"Read",` +
    `"tool_input":{"file_path":"%s/scale-read-%s.md"}}'`,
  '"$CLAUDE_CODE_SESSION_ID" "$PWD" "$FAKE_HARNESS_TICK"',
  `| CLAUDE_PID=$PPID ${singleQuoted(built("charter"))} hook pretooluse >/dev/null`,
].join(" ");

describe("chat events reaching the window while busy chats stream", () => {
  after(closeEverything);

  it(`counts them for ${CHATS} chats, each firing four hooks a second`, async () => {
    await browser.waitUntil(async () => browser.execute(() => Boolean(window.charterBench)), {
      timeout: 60_000,
      timeoutMsg: "the window has no benchmark seam: was the frontend built with --mode e2e?",
    });
    nextLoad([
      "--synthetic",
      "4096",
      "--loops",
      "1000000",
      "--interval-ms",
      "20",
      "--tick-ms",
      "250",
      "--tick-hook",
      aReadFromInsideTheChat,
    ]);
    for (let n = 0; n < CHATS; n++) await openTab();
    // Every chat streaming and firing before the count starts.
    await sleep(5000);

    await browser.execute(() => {
      const counted = { events: 0, told: 0 };
      (window as unknown as { scaleChatEvents: typeof counted }).scaleChatEvents = counted;
      void window.__TAURI__.event.listen("chat-events", (event) => {
        counted.events += 1;
        counted.told += (event.payload as { events: unknown[] }).events.length;
      });
    });
    await sleep(MEASURED_MS);
    const counted = await browser.execute(
      () =>
        (window as unknown as { scaleChatEvents: { events: number; told: number } })
          .scaleChatEvents,
    );
    if (counted.told === 0) {
      throw new Error("no chat event reached the window: the chats' hooks told the app nothing");
    }

    const seconds = MEASURED_MS / 1000;
    const measured = {
      value: Math.round((counted.events / seconds) * 10) / 10,
      tellingsPerSecond: Math.round((counted.told / seconds) * 10) / 10,
      chats: CHATS,
    };
    const budget = unbatched ? "chat-events-unbatched" : "chat-events";
    console.log(`scale-run | ${budget} | ${JSON.stringify(measured)}`);
    if (out) appendFileSync(out, `${JSON.stringify({ budget, os, ...measured })}\n`);
  });
});
