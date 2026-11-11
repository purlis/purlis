// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { PROMPTS } from "../src/interruptBudget.ts";
import {
  CEILINGS,
  type Counted,
  DEFAULT_CEILING,
  ceilingOf,
  countIn,
  overCeiling,
  overSaid,
} from "./interrupts.js";

describe("a flow's interruptions (#6021)", () => {
  it("are within the ceiling at it, and over it past it", () => {
    expect(overCeiling({ prompts: 3, asks: 3, notifications: 1 }, DEFAULT_CEILING)).toEqual([]);
    expect(overCeiling({ prompts: 4, asks: 0, notifications: 2 }, DEFAULT_CEILING)).toEqual([
      "4 prompts (ceiling 3)",
      "2 notifications (ceiling 1)",
    ]);
  });

  it("are held to W10's three prompts where the file names no ceiling", () => {
    expect(ceilingOf("/run/app/e2e/specs/unnamed.e2e.ts")).toEqual(DEFAULT_CEILING);
    expect(DEFAULT_CEILING.prompts).toBe(3);
  });

  it("are held to a named file's ceiling only with its reason", () => {
    for (const [file, named] of Object.entries(CEILINGS)) {
      expect(named.why.length, file).toBeGreaterThan(20);
      expect(ceilingOf(`/x/specs/${file}`)).toEqual({ ...DEFAULT_CEILING, ...named.ceiling });
    }
  });

  it("name each prompt counted past the ceiling", () => {
    expect(
      overCeiling(
        { prompts: 4, asks: 0, notifications: 0, prompted: ["a", "b", "c", "d"] },
        DEFAULT_CEILING,
      ),
    ).toEqual(["4 prompts (ceiling 3): a · b · c · d"]);
  });

  it("are said naming the file, each flow over and what it cost", () => {
    expect(
      overSaid("/run/app/e2e/specs/opener.e2e.ts", [
        { flow: "opening a stranger's project asks", past: ["4 prompts (ceiling 3)"] },
      ]),
    ).toBe(
      'specs/opener.e2e.ts: a flow interrupts more than its ceiling allows: "opening a ' +
        "stranger's project asks\": 4 prompts (ceiling 3). Take the interruption out, or raise " +
        "the file's ceiling in e2e/interrupts.ts with the reason (#6021).",
    );
  });
});

describe("what a flow's window counts (#6021)", () => {
  const TALLY = "__test_tally__";
  const HAND = '[data-testid="hand"]';
  type Held = Counted & { stop: () => void };
  const held = () => (window as unknown as Record<string, Held>)[TALLY];
  const settle = () => new Promise((done) => setTimeout(done, 0));
  const show = (name: string, role = "dialog") => {
    const one = document.createElement("div");
    one.setAttribute("role", role);
    one.setAttribute("aria-label", name);
    document.body.append(one);
    return one;
  };
  const counting = async (flow: () => Promise<void>) => {
    countIn(TALLY, PROMPTS, HAND, "asks-notified");
    await flow();
    await settle();
    const tally = held();
    tally.stop();
    return { prompts: tally.prompts, asks: tally.asks, prompted: tally.prompted };
  };
  const button = () => {
    const one = document.createElement("button");
    document.body.append(one);
    return one;
  };

  afterEach(() => {
    held()?.stop();
    document.body.replaceChildren();
  });

  it("does not count a dialog the flow's own press opened", async () => {
    const press = button();
    const cost = await counting(async () => {
      press.dispatchEvent(new Event("pointerdown", { bubbles: true }));
      press.click();
      show("Pick a harness");
      await settle();
    });
    expect(cost.prompts).toBe(0);
  });

  it("does not count the palette a key with ⌘ opened", async () => {
    const cost = await counting(async () => {
      document.body.dispatchEvent(
        new KeyboardEvent("keydown", { key: "k", metaKey: true, bubbles: true }),
      );
      show("Command palette");
      await settle();
    });
    expect(cost.prompts).toBe(0);
  });

  it("counts a dialog no action of the flow's opened, by its name", async () => {
    const cost = await counting(async () => {
      show("Allow this chat to reach example.com?", "alertdialog");
      await settle();
    });
    expect(cost).toMatchObject({ prompts: 1, prompted: ["Allow this chat to reach example.com?"] });
  });

  it("counts a second prompt with no action between it and the one the flow opened", async () => {
    const press = button();
    const cost = await counting(async () => {
      press.click();
      show("Pick a harness");
      await settle();
      show("Do you trust this project?", "alertdialog");
      await settle();
    });
    expect(cost).toMatchObject({ prompts: 1, prompted: ["Do you trust this project?"] });
  });

  it("takes two clicks made one straight after the other for two actions", async () => {
    const [first, second] = [button(), button()];
    const cost = await counting(async () => {
      document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
      await settle();
      first.click();
      show("Switch workspace");
      await settle();
      second.click();
      show("Pick a harness");
      await settle();
    });
    expect(cost.prompts).toBe(0);
  });

  it("does not count the palette a function key opened", async () => {
    const cost = await counting(async () => {
      document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "F2", bubbles: true }));
      show("Command palette");
      await settle();
    });
    expect(cost.prompts).toBe(0);
  });

  it("does not take typing into a field for an action", async () => {
    const cost = await counting(async () => {
      document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "a", bubbles: true }));
      show("Pick a harness");
      await settle();
    });
    expect(cost.prompts).toBe(1);
  });

  it("takes a pointer's press and the click it ends in for one action", async () => {
    const press = button();
    const cost = await counting(async () => {
      press.dispatchEvent(new Event("pointerdown", { bubbles: true }));
      show("Pick a harness");
      await settle();
      press.click();
      show("Do you trust this project?", "alertdialog");
      await settle();
    });
    expect(cost.prompted).toEqual(["Do you trust this project?"]);
  });

  it("counts each rise of the ✋", async () => {
    const hand = document.createElement("span");
    hand.dataset.testid = "hand";
    hand.textContent = "1";
    document.body.append(hand);
    const cost = await counting(async () => {
      hand.textContent = "3";
      await settle();
      hand.textContent = "2";
      await settle();
    });
    expect(cost.asks).toBe(2);
  });
});
