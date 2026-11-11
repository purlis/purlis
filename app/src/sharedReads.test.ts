// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { commands } from "./bindings";
import { LANES, WRITERS, keepTheirOrder } from "./commandOrder";
import { WAITS_BY_DESIGN, watchTheAnswers } from "./answerWatch";
import { SHARED_READS, shareTheReads } from "./sharedReads";

/** Every answer the window times, so a test can count how many asks were timed. */
const timings = vi.hoisted(() => [] as string[]);
vi.mock("./slowLog", async (original) => ({
  ...(await original<typeof import("./slowLog")>()),
  answered: (command: string) => timings.push(command),
}));

/**
 * **Identical reads asked while one is in flight share its answer** (`sharedReads.ts`, felt
 * speed round 2, H6): every command enters the core through one queue on its main thread, so
 * each extra ask of the same read delays every other command, typing included.
 */

beforeAll(() => {
  // As `main.tsx` wires them: the lanes, the timing, then the sharing outermost.
  keepTheirOrder();
  watchTheAnswers();
  shareTheReads();
});
afterEach(() => {
  clearMocks();
  timings.length = 0;
});

const tick = () => new Promise((settled) => setTimeout(settled, 0));

/** A core that answers each command only when the test says, and says what was sent. */
function aCoreAnsweringWhenTold() {
  const sent: { cmd: string; args: unknown }[] = [];
  const waiting: { cmd: string; answer: (said: unknown) => void }[] = [];
  mockIPC(
    (cmd, args) =>
      new Promise((answer) => {
        sent.push({ cmd, args });
        waiting.push({ cmd, answer });
      }),
  );
  /** Answers every command `cmd` sent so far, and not yet answered, with `said`. */
  const release = (cmd: string, said: unknown = null) => {
    for (const one of waiting.filter((w) => w.cmd === cmd)) {
      waiting.splice(waiting.indexOf(one), 1);
      one.answer(said);
    }
  };
  const count = (cmd: string) => sent.filter((one) => one.cmd === cmd).length;
  return { sent, release, count };
}

describe("an identical read already in flight", () => {
  it("is asked of the core once for seventeen asks in one tick, and all seventeen get its answer", async () => {
    const core = aCoreAnsweringWhenTold();
    const asks = Array.from({ length: 17 }, () => commands.signingWaiting("/p"));
    await tick();
    expect(core.count("signing_waiting")).toBe(1);

    const waiting = [{ forge: "GitHub" }];
    core.release("signing_waiting", waiting);
    const answers = await Promise.all(asks);
    expect(answers).toHaveLength(17);
    for (const one of answers) expect(one).toEqual({ status: "ok", data: waiting });
  });

  it("is timed once, as the one ask the core was sent", async () => {
    const core = aCoreAnsweringWhenTold();
    const asks = Array.from({ length: 17 }, () => commands.signingWaiting("/p"));
    await tick();
    core.release("signing_waiting", []);
    await Promise.all(asks);
    expect(timings.filter((one) => one === "signingWaiting")).toHaveLength(1);
  });

  it("is not shared with the same read of another project or another chat", async () => {
    const core = aCoreAnsweringWhenTold();
    void commands.chatSkills("/p", 1);
    void commands.chatSkills("/p", 2);
    void commands.chatSkills("/q", 1);
    void commands.chatSkills("/p", 1);
    await tick();
    expect(core.sent.filter((one) => one.cmd === "chat_skills").map((one) => one.args)).toEqual([
      { plane: "/p", session: 1 },
      { plane: "/p", session: 2 },
      { plane: "/q", session: 1 },
    ]);
  });

  it("shares its error too, and the next ask after it asks again", async () => {
    mockIPC(() => Promise.reject("the plane is gone"));
    const answers = await Promise.all([commands.skillDrafts("/p"), commands.skillDrafts("/p")]);
    for (const one of answers) expect(one).toEqual({ status: "error", error: "the plane is gone" });
  });
});

describe("a read asked after the one in flight answered", () => {
  it("asks the core afresh: nothing is kept past the answer", async () => {
    const core = aCoreAnsweringWhenTold();
    const first = commands.signingWaiting("/p");
    await tick();
    core.release("signing_waiting", []);
    await first;
    void commands.signingWaiting("/p");
    await tick();
    expect(core.count("signing_waiting")).toBe(2);
  });
});

describe("a read in a lane", () => {
  it("asked after a change in its lane does not share the read asked before the change", async () => {
    const core = aCoreAnsweringWhenTold();
    const before = commands.personaGrantsHeld("/p", 1);
    await tick();
    const change = commands.allowPersonaGrants("/p", 1);
    const after = commands.personaGrantsHeld("/p", 1);
    core.release("persona_grants_held", null);
    await before;
    await tick();
    core.release("allow_persona_grants", true);
    await change;
    await tick();
    expect(core.count("persona_grants_held")).toBe(2);
    core.release("persona_grants_held", { from: 2 });
    expect(await after).toEqual({ status: "ok", data: { from: 2 } });
  });
});

describe("a command that is not a read", () => {
  it("is asked of the core every time, however alike: typing is never shared", async () => {
    const core = aCoreAnsweringWhenTold();
    void commands.sendInput("/p", 1, "a");
    void commands.sendInput("/p", 1, "a");
    await tick();
    expect(core.count("send_input")).toBe(2);
  });
});

describe("the reads that are shared", () => {
  it("are each one the bindings have", () => {
    const asks = commands as unknown as Record<string, unknown>;
    for (const name of SHARED_READS) expect(typeof asks[name], name).toBe("function");
  });

  it("include none that is a lane's change, writes a lane's file or waits by design", () => {
    const changes = new Set<string>(Object.values(LANES).flatMap((lane) => lane.changes));
    const writers = new Set(Object.values(WRITERS).flatMap((files) => Object.values(files).flat()));
    for (const name of SHARED_READS) {
      expect(changes.has(name), name).toBe(false);
      expect(writers.has(name), name).toBe(false);
      expect(WAITS_BY_DESIGN.has(name), name).toBe(false);
    }
  });
});
