// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect, $ } from "@wdio/globals";
import { ask } from "../switching.js";

/**
 * The device's RAM class and its hot-chat target, as Copy diagnostics asks for them (ADR 0082
 * §3, SC-4f #7635), against the real app.
 *
 * **The CI runner's memory will not give the top class**, so the `e2e` build's
 * `ram_class_stand_in` puts a class in place of the machine's. It sets the class and the target
 * and nothing else: the open and hot counts are the chats this run holds, and are read before
 * and after to say so. Taking the stand-in away gives the machine's own class back.
 */

type DeviceClass =
  | { kind: "reading" }
  | { kind: "not_known" }
  | { kind: "known"; class_gb: number | null; hot_target: number };

type Here = { os: string; open: number; hot: number; device: DeviceClass };

const here = () => ask<Here>("diagnostics_here");

/** What the device says once its memory has been read, insisting it is read within 30 s. */
async function read(): Promise<Here> {
  let now = await here();
  await browser.waitUntil(
    async () => {
      now = await here();
      return now.device.kind !== "reading";
    },
    { timeout: 30_000, timeoutMsg: "the device's memory was still being read after 30 s" },
  );
  return now;
}

/** The class and target of each memory the stand-in names: ADR 0082 §3's table. */
const TABLE: [number, number | null, number][] = [
  [4, null, 1],
  [8, 8, 5],
  [16, 16, 15],
  [32, 32, 35],
  [64, 64, 50],
  [128, 64, 50],
];

/**
 * The Diagnostics line about the chats, as About's Copy diagnostics puts it together, found by
 * the button's label and the dialog's and its status's roles. The page cannot read the
 * clipboard back in the engine (see `terminal-menu.e2e.ts`), so `writeText` is recorded for
 * the one press and put back after it.
 */
async function chatsLine(): Promise<string> {
  await browser.execute(() => {
    const clipboard = navigator.clipboard;
    const real = clipboard.writeText.bind(clipboard);
    const kept = window as unknown as { __copied?: string; __writeText?: typeof real };
    kept.__writeText = real;
    Object.defineProperty(clipboard, "writeText", {
      configurable: true,
      value: (text: string) => {
        kept.__copied = text;
        return Promise.resolve();
      },
    });
  });
  try {
    await (await $("aria/About purlis — what this version brought")).click();
    const dialog = await $('[role="dialog"][aria-describedby="about-what"]');
    await dialog.waitForDisplayed({ timeout: 20_000 });
    await (await dialog.$("aria/Copy diagnostics")).click();
    const status = await dialog.$('[role="status"]');
    await browser.waitUntil(async () => (await status.getText()) === "Diagnostics copied.", {
      timeout: 20_000,
      timeoutMsg: "Copy diagnostics did not say it copied",
    });
    const report = await browser.execute(
      () => (window as unknown as { __copied?: string }).__copied ?? "",
    );
    await browser.keys("Escape");
    await dialog.waitForExist({ timeout: 20_000, reverse: true });
    return report.split("\n").find((line) => line.startsWith("Chats: ")) ?? report;
  } finally {
    await browser.execute(() => {
      delete (navigator.clipboard as unknown as { writeText?: unknown }).writeText;
    });
  }
}

describe("the device's RAM class and hot target", function () {
  this.timeout(120_000);

  after(async () => {
    await ask("ram_class_stand_in", { gigabytes: null });
  });

  it("answers the table's class and target for each memory stood in, and counts the same chats", async () => {
    await $('[data-testid="status-project"]').waitForDisplayed({ timeout: 30_000 });
    const before = await here();
    for (const [gigabytes, class_gb, hot_target] of TABLE) {
      await ask("ram_class_stand_in", { gigabytes });
      const now = await here();
      expect([gigabytes, now.device]).toEqual([gigabytes, { kind: "known", class_gb, hot_target }]);
      expect([now.open, now.hot]).toEqual([before.open, before.hot]);
    }
  });

  it("says the class and target in the Diagnostics line About copies", async () => {
    await ask("ram_class_stand_in", { gigabytes: 64 });
    const { open, hot } = await here();
    expect(await chatsLine()).toBe(`Chats: ${open} open, ${hot} hot, target 50 (64 GB class)`);
    await ask("ram_class_stand_in", { gigabytes: 4 });
    expect(await chatsLine()).toBe(`Chats: ${open} open, ${hot} hot, target 1 (under 8 GB)`);
  });

  it("gives the machine's own class back when the stand-in is taken away", async () => {
    await ask("ram_class_stand_in", { gigabytes: null });
    const machine = await read();
    await ask("ram_class_stand_in", { gigabytes: 64 });
    expect((await here()).device).toEqual({ kind: "known", class_gb: 64, hot_target: 50 });
    await ask("ram_class_stand_in", { gigabytes: null });
    const back = await read();
    expect(back.device).toEqual(machine.device);
    expect(back.device.kind).toBe("known");
  });
});
