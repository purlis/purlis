// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { commands } from "./bindings";
import { LANES, OUTSIDE_THE_LANES, WRITERS, inOrder, keepTheirOrder } from "./commandOrder";

/**
 * **The core's answers that read and change the same files come back in the order asked, and
 * reads run together** (`commandOrder.ts`, #1949): the core answers them off the window's thread
 * now, where a read asked first can finish last.
 */

beforeAll(() => keepTheirOrder());
afterEach(() => clearMocks());

const tick = () => new Promise((settled) => setTimeout(settled, 0));

/** A core that answers each command only when the test says, and says what was sent. */
function aCoreAnsweringWhenTold() {
  const sent: string[] = [];
  const waiting: { cmd: string; answer: () => void }[] = [];
  mockIPC(
    (cmd) =>
      new Promise((answer) => {
        sent.push(cmd);
        waiting.push({ cmd, answer: () => answer(null) });
      }),
  );
  /** Answers every command `cmd` sent so far. */
  const release = (cmd: string) => {
    for (const one of waiting.filter((w) => w.cmd === cmd)) one.answer();
  };
  return { sent, release };
}

describe("a lane", () => {
  it("sends a read beside the reads before it, at once", () => {
    const sent: string[] = [];
    const ask = (name: string) => () => {
      sent.push(name);
      return new Promise(() => {});
    };
    void inOrder("t1", "read", ask("one"))("/p");
    void inOrder("t1", "read", ask("two"))("/p");
    expect(sent).toEqual(["one", "two"]);
  });

  it("sends a change once every read before it has answered, and a read after it once it has", async () => {
    const sent: string[] = [];
    const answers: Record<string, () => void> = {};
    const ask = (name: string) => () => {
      sent.push(name);
      return new Promise<void>((done) => (answers[name] = done));
    };
    void inOrder("t2", "read", ask("read 1"))("/p");
    void inOrder("t2", "read", ask("read 2"))("/p");
    void inOrder("t2", "change", ask("change"))("/p");
    void inOrder("t2", "read", ask("read 3"))("/p");
    await tick();
    expect(sent).toEqual(["read 1", "read 2"]);
    answers["read 1"]();
    await tick();
    expect(sent).toEqual(["read 1", "read 2"]);
    answers["read 2"]();
    await tick();
    expect(sent).toEqual(["read 1", "read 2", "change"]);
    answers.change();
    await tick();
    expect(sent).toEqual(["read 1", "read 2", "change", "read 3"]);
  });

  it("holds no project behind another's", () => {
    const sent: string[] = [];
    const ask = (name: string) => () => {
      sent.push(name);
      return new Promise(() => {});
    };
    void inOrder("t3", "change", ask("a"))("/a");
    void inOrder("t3", "change", ask("b"))("/b");
    expect(sent).toEqual(["a", "b"]);
  });

  it("goes on after a change that failed, and hands the failure to its own caller", async () => {
    const first = inOrder("t4", "change", () => Promise.reject(new Error("no")))("/c");
    const second = inOrder("t4", "change", () => Promise.resolve("yes"))("/c");
    await expect(first).rejects.toThrow("no");
    await expect(second).resolves.toBe("yes");
  });
});

describe("the window's lanes", () => {
  it("hold each command once, of the bindings, in one lane only", () => {
    const asks = commands as unknown as Record<string, unknown>;
    const seen = new Map<string, string>();
    for (const [lane, { reads, changes }] of Object.entries(LANES))
      for (const name of [...reads, ...changes]) {
        expect(typeof asks[name], name).toBe("function");
        expect(seen.get(name), `${name} is in ${seen.get(name)} and in ${lane}`).toBeUndefined();
        seen.set(name, lane);
      }
  });

  it("hold every writer of a lane's files in that lane, or out of it with a reason", () => {
    const asks = commands as unknown as Record<string, unknown>;
    const inALane = new Set(
      Object.values(LANES).flatMap(({ reads, changes }) => [...reads, ...changes] as string[]),
    );
    for (const [lane, files] of Object.entries(WRITERS)) {
      const changes = LANES[lane as keyof typeof LANES].changes as readonly string[];
      const writers = new Set<string>();
      for (const [file, named] of Object.entries(files))
        for (const writer of named as readonly string[]) {
          expect(typeof asks[writer], writer).toBe("function");
          writers.add(writer);
          const outside = (OUTSIDE_THE_LANES as Record<string, string | undefined>)[writer];
          if (outside === undefined)
            expect(changes, `${writer} writes ${file} and is not a change of ${lane}`).toContain(
              writer,
            );
          else expect(inALane.has(writer), `${writer} is out of the lanes and in one`).toBe(false);
        }
      // And a change of the lane writes one of its files.
      for (const change of changes)
        expect(writers.has(change), `${change} writes none of ${lane}'s files`).toBe(true);
    }
  });

  it("hold a host's removal behind a read of the sandbox already asked (Settings › Sandbox)", async () => {
    // The page reads the state as it is focused again, git makes that read slow, and the
    // person removes a host meanwhile: the read must not land after the removal and draw the
    // host back.
    const core = aCoreAnsweringWhenTold();
    void commands.sandboxState("/h");
    void commands.removeSandboxHost("/h", "local", null, "example.com");
    await tick();
    expect(core.sent).toEqual(["sandbox_state"]);
    core.release("sandbox_state");
    await tick();
    await tick();
    expect(core.sent).toEqual(["sandbox_state", "remove_sandbox_host"]);
    core.release("remove_sandbox_host");
  });

  it("send a Keep blocked at once, whatever the sandbox's lane is waiting on: it types into the chat", () => {
    const core = aCoreAnsweringWhenTold();
    void commands.sandboxState("/k");
    void commands.keepSandboxBlock("/k", 1, {
      operation: "network-outbound",
      kind: "host",
      what: "host",
      target: "example.com",
    });
    expect(core.sent).toEqual(["sandbox_state", "keep_sandbox_block"]);
  });

  it("send the reads a switch makes all at once: five notices and forty panes", async () => {
    // What a switch to a project with forty panes asks of the sandbox's lane. They run
    // together; none waits for another.
    const core = aCoreAnsweringWhenTold();
    for (let n = 0; n < 5; n++) void commands.sandboxState("/p");
    for (let session = 1; session <= 40; session++) void commands.personaGrantsHeld("/p", session);
    await tick();
    expect(core.sent.filter((cmd) => cmd === "sandbox_state")).toHaveLength(5);
    expect(core.sent.filter((cmd) => cmd === "persona_grants_held")).toHaveLength(40);
    core.release("sandbox_state");
    core.release("persona_grants_held");
  });

  it("send a read of the sandbox asked before an answer to it ahead of that answer", async () => {
    // A notice reads the state as the window regains focus, and the press that focused it
    // answers the offer: the answer waits for the read, so the read cannot land last.
    const core = aCoreAnsweringWhenTold();
    const read = commands.sandboxState("/q");
    const answered = commands.answerSandboxOffer("/q", true);
    await tick();
    expect(core.sent).toEqual(["sandbox_state"]);
    core.release("sandbox_state");
    await read;
    await tick();
    expect(core.sent).toEqual(["sandbox_state", "answer_sandbox_offer"]);
    core.release("answer_sandbox_offer");
    await answered;
  });

  it("hold an Allow of the persona grants behind a read of the persona's hosts, across notices", async () => {
    // Two notices, one file: the persona hosts allowed here. A read by one and a change by the
    // other keep their order.
    const core = aCoreAnsweringWhenTold();
    void commands.personaGrantsHeld("/r", 1);
    void commands.allowPersonaHosts("/r", "scribe", "digest");
    await tick();
    expect(core.sent).toEqual(["persona_grants_held"]);
    core.release("persona_grants_held");
    await tick();
    await tick();
    expect(core.sent).toEqual(["persona_grants_held", "allow_persona_hosts"]);
    core.release("allow_persona_hosts");
  });

  it("hold a dispatch Allow behind a read of what was refused while you were away", async () => {
    // The away list and a held dispatch's Notice read and change the same grants.
    const core = aCoreAnsweringWhenTold();
    void commands.dispatchAway("/s");
    void commands.allowDispatch("/s", 7, "project", [], "");
    await tick();
    expect(core.sent).toEqual(["dispatch_away"]);
    core.release("dispatch_away");
    await tick();
    await tick();
    expect(core.sent).toEqual(["dispatch_away", "allow_dispatch"]);
    core.release("allow_dispatch");
  });
});
