// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { expect, $ } from "@wdio/globals";
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

type Here = {
  os: string;
  open: number;
  hot: number;
  ram_class_gb: number | null;
  hot_target: number;
};

const here = () => ask<Here>("diagnostics_here");

/** The class and target of each memory the stand-in names: ADR 0082 §3's table. */
const TABLE: [number, number | null, number][] = [
  [4, null, 1],
  [8, 8, 5],
  [16, 16, 15],
  [32, 32, 35],
  [64, 64, 50],
  [128, 64, 50],
];

describe("the device's RAM class and hot target", function () {
  this.timeout(120_000);

  after(async () => {
    await ask("ram_class_stand_in", { gigabytes: null });
  });

  it("answers the table's class and target for each memory stood in, and counts the same chats", async () => {
    await $('[data-testid="status-project"]').waitForDisplayed({ timeout: 30_000 });
    const before = await here();
    for (const [gigabytes, class_, target] of TABLE) {
      await ask("ram_class_stand_in", { gigabytes });
      const now = await here();
      expect([gigabytes, now.ram_class_gb, now.hot_target]).toEqual([gigabytes, class_, target]);
      expect([now.open, now.hot]).toEqual([before.open, before.hot]);
    }
  });

  it("gives the machine's own class back when the stand-in is taken away", async () => {
    const machine = await (async () => {
      await ask("ram_class_stand_in", { gigabytes: null });
      return here();
    })();
    await ask("ram_class_stand_in", { gigabytes: 64 });
    expect((await here()).hot_target).toBe(50);
    await ask("ram_class_stand_in", { gigabytes: null });
    const back = await here();
    expect([back.ram_class_gb, back.hot_target]).toEqual([
      machine.ram_class_gb,
      machine.hot_target,
    ]);
    expect(back.hot_target).toBeGreaterThanOrEqual(1);
  });
});
