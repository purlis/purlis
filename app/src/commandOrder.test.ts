// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { commands } from "./bindings";
import { LANES, inOrder, keepTheirOrder } from "./commandOrder";

/**
 * **The core's answers that read and change the same thing come back in the order asked**
 * (`commandOrder.ts`, #1949): the core answers them off the window's thread now, where a read
 * asked first can finish last.
 */

afterEach(() => clearMocks());

/** A core whose answers come when `release(n)` says, the `n`th ask counted from 0. */
function aCoreAnsweringWhenTold() {
  const sent: string[] = [];
  const waiting: (() => void)[] = [];
  const ask = (name: string) => () => {
    sent.push(name);
    return new Promise<string>((answer) => waiting.push(() => answer(name)));
  };
  return { sent, release: (n: number) => waiting[n](), ask };
}

const tick = () => new Promise((settled) => setTimeout(settled, 0));

describe("a lane", () => {
  it("sends an ask only once the one before it in the same project has answered", async () => {
    const core = aCoreAnsweringWhenTold();
    const read = inOrder("sandbox", core.ask("read"));
    const write = inOrder("sandbox", core.ask("write"));
    const said: string[] = [];

    void read("/p").then((x) => said.push(String(x)));
    void write("/p").then((x) => said.push(String(x)));
    await tick();
    // The read is out; the write waits for it.
    expect(core.sent).toEqual(["read"]);

    core.release(0);
    await tick();
    expect(core.sent).toEqual(["read", "write"]);
    core.release(1);
    await tick();
    expect(said).toEqual(["read", "write"]);
  });

  it("sends the first ask at once, in the same tick", () => {
    const core = aCoreAnsweringWhenTold();
    void inOrder("sandbox", core.ask("read"))("/q");
    expect(core.sent).toEqual(["read"]);
  });

  it("holds no project behind another's, nor one lane behind another", async () => {
    const core = aCoreAnsweringWhenTold();
    void inOrder("sandbox", core.ask("one"))("/a");
    void inOrder("sandbox", core.ask("two"))("/b");
    void inOrder("pins", core.ask("three"))("/a");
    await tick();
    expect(core.sent).toEqual(["one", "two", "three"]);
  });

  it("goes on after an ask that failed, and hands the failure to its own caller", async () => {
    const failing = inOrder("network", () => Promise.reject(new Error("no")));
    const next = inOrder("network", () => Promise.resolve("yes"));
    const first = failing("/c");
    const second = next("/c");
    await expect(first).rejects.toThrow("no");
    await expect(second).resolves.toBe("yes");
  });
});

describe("the window's commands", () => {
  it("each in a lane is one the bindings have, and takes the project first", () => {
    const asks = commands as unknown as Record<string, unknown>;
    for (const names of Object.values(LANES))
      for (const name of names) expect(typeof asks[name], name).toBe("function");
  });

  it("a read of the sandbox asked before an answer to it lands before that answer's", async () => {
    // A notice reads the state as the window regains focus, and the press that focused it
    // answers the offer: the read must not land last and draw the offer again.
    const sent: string[] = [];
    const answers: Record<string, () => void> = {};
    mockIPC(
      (cmd) =>
        new Promise((answer) => {
          sent.push(cmd);
          answers[cmd] = () => answer(cmd === "sandbox_state" ? { offer: true } : { offer: false });
        }),
    );
    keepTheirOrder();
    const read = commands.sandboxState("/p");
    const answered = commands.answerSandboxOffer("/p", true);
    await tick();
    expect(sent).toEqual(["sandbox_state"]);
    answers.sandbox_state();
    await read;
    await tick();
    expect(sent).toEqual(["sandbox_state", "answer_sandbox_offer"]);
    answers.answer_sandbox_offer();
    expect(await answered).toEqual({ status: "ok", data: { offer: false } });
  });
});
