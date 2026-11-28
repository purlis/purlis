// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { listen, untilStopped } from "./here";

/**
 * **One Tauri listener per event per window** (FS-W1, #7909, epic #7889): `listen` in `here.ts`
 * is the window's hub. However many of the window's parts listen for one event, the core is asked
 * to listen for it once, and that one listener calls each of them.
 */

/** A core that keeps every listen it is asked for, and answers each when the test lets it. */
function aCore({ held = false }: { held?: boolean } = {}) {
  const listens: { event: string; target: unknown; handler: number }[] = [];
  const unlistens: { event: string; eventId: number }[] = [];
  const listening = new Map<string, number[]>();
  let answer: () => void = () => {};
  let gate: Promise<void> = held ? new Promise((done) => (answer = done)) : Promise.resolve();
  let refusing = false;
  mockIPC(async (cmd, args) => {
    const given = args as Record<string, unknown>;
    if (cmd === "plugin:event|listen") {
      const { event, target, handler } = given as {
        event: string;
        target: unknown;
        handler: number;
      };
      listens.push({ event, target, handler });
      await gate;
      if (refusing) throw new Error("the bridge is gone");
      listening.set(event, [...(listening.get(event) ?? []), handler]);
      return handler;
    }
    if (cmd === "plugin:event|unlisten") {
      const { event, eventId } = given as { event: string; eventId: number };
      unlistens.push({ event, eventId });
      listening.set(
        event,
        (listening.get(event) ?? []).filter((one) => one !== eventId),
      );
      return null;
    }
    return null;
  });
  return {
    listens,
    unlistens,
    /** The Tauri listeners held now for `event`. */
    held: (event: string) => (listening.get(event) ?? []).length,
    /** Says `event` with `payload`, as the core does, to each listener held for it. */
    says: (event: string, payload: unknown) => {
      for (const handler of listening.get(event) ?? [])
        window.__TAURI_INTERNALS__.runCallback(handler, { event, id: handler, payload });
    },
    /** Lets the held listens answer. */
    answer: () => answer(),
    /** Holds every listen from now on until `answer`. */
    hold: () => {
      gate = new Promise((done) => (answer = done));
    },
    /** Refuses every listen from now on. */
    refuse: () => {
      refusing = true;
    },
    /** Accepts every listen from now on. */
    accept: () => {
      refusing = false;
    },
  };
}

/** Lets every promise already answered run its callbacks. */
const drained = () => new Promise((done) => setTimeout(done, 0));

afterEach(() => {
  clearMocks();
  vi.restoreAllMocks();
});

describe("the window's one listener per event", () => {
  it("asks the core to listen once for an event however many parts listen, and calls each of them", async () => {
    const core = aCore();
    const heard: string[] = [];
    const stops = await Promise.all(
      ["a", "b", "c"].map((who) =>
        listen<number>("plane-changed", (event) => void heard.push(`${who} ${event.payload}`)),
      ),
    );

    expect(core.listens.map((one) => one.event)).toEqual(["plane-changed"]);
    core.says("plane-changed", 7);
    expect(heard).toEqual(["a 7", "b 7", "c 7"]);
    for (const stop of stops) stop();
  });

  it("asks once for each event apart", async () => {
    const core = aCore();
    const stops = await Promise.all([
      listen("plane-changed", () => {}),
      listen("chat-events", () => {}),
      listen("plane-changed", () => {}),
    ]);

    expect(core.listens.map((one) => one.event)).toEqual(["plane-changed", "chat-events"]);
    for (const stop of stops) stop();
  });

  it("names this window as the listener's target", async () => {
    mockWindows("second");
    const core = aCore();
    const stop = await listen("quit-asked", () => {});

    expect(core.listens).toEqual([
      expect.objectContaining({ target: { kind: "AnyLabel", label: "second" } }),
    ]);
    stop();
  });

  it("answers a part only once the core's listener is there, and a later part at once", async () => {
    const core = aCore({ held: true });
    let first = false;
    void listen("asks-changed", () => {}).then(() => (first = true));
    await drained();
    expect(first, "answered before the core listened").toBe(false);

    core.answer();
    await drained();
    expect(first).toBe(true);

    let later = false;
    void listen("asks-changed", () => {}).then(() => (later = true));
    await drained();
    expect(later, "a later part waits for nothing").toBe(true);
    expect(core.listens).toHaveLength(1);
  });

  it("answers a part that came while the listen was asked once that one listen is there", async () => {
    const core = aCore({ held: true });
    const answered: string[] = [];
    void listen("asks-changed", () => {}).then(() => answered.push("first"));
    void listen("asks-changed", () => {}).then(() => answered.push("second"));
    await drained();
    expect(answered).toEqual([]);

    core.answer();
    await drained();
    expect(answered).toEqual(["first", "second"]);
    expect(core.listens).toHaveLength(1);
  });

  it("calls every part though one of them throws, and reports what it threw as the window's error", async () => {
    const core = aCore();
    const reported: unknown[] = [];
    const seen = (event: ErrorEvent) => {
      event.preventDefault();
      reported.push(event.error);
    };
    window.addEventListener("error", seen);
    const thrown = new Error("a notice broke");
    const heard: string[] = [];
    const stops = await Promise.all([
      listen("plane-changed", () => void heard.push("before")),
      listen("plane-changed", () => {
        throw thrown;
      }),
      listen("plane-changed", () => void heard.push("after")),
    ]);

    core.says("plane-changed", null);
    window.removeEventListener("error", seen);

    expect(heard).toEqual(["before", "after"]);
    expect(reported).toEqual([thrown]);
    for (const stop of stops) stop();
  });

  it("stops the core's listener once the last part stops, and not before", async () => {
    const core = aCore();
    const [one, two] = await Promise.all([
      listen("chat-events", () => {}),
      listen("chat-events", () => {}),
    ]);

    one();
    await drained();
    expect(core.unlistens, "stopped while a part still listens").toEqual([]);

    two();
    await drained();
    expect(core.unlistens.map((it) => it.event)).toEqual(["chat-events"]);
    expect(core.held("chat-events")).toBe(0);
  });

  it("stops a part's listening once, however often its stop is called", async () => {
    const core = aCore();
    const heard: string[] = [];
    const [one] = await Promise.all([
      listen("chat-events", () => void heard.push("one")),
      listen("chat-events", () => void heard.push("two")),
    ]);

    one();
    one();
    await drained();
    core.says("chat-events", null);

    expect(heard).toEqual(["two"]);
    expect(core.unlistens).toEqual([]);
  });

  it("keeps the core's listener for a part that listens as the last one stops, and loses no event", async () => {
    const core = aCore();
    const heard: string[] = [];
    const going = await listen("chat-hibernation-changed", () => void heard.push("going"));

    // A notice mounted again for another chat: the old one's stop and the new one's listen, in
    // one commit.
    going();
    const coming = listen("chat-hibernation-changed", () => void heard.push("coming"));
    core.says("chat-hibernation-changed", null);
    const stop = await coming;
    await drained();

    expect(heard).toEqual(["coming"]);
    expect(core.listens, "listened again").toHaveLength(1);
    expect(core.unlistens, "stopped").toEqual([]);
    expect(core.held("chat-hibernation-changed")).toBe(1);
    stop();
  });

  it("stops the core's listener once when parts come and go again in one task", async () => {
    const core = aCore();
    const first = await listen("task-quiet", () => {});

    first();
    const second = listen("task-quiet", () => {});
    const third = listen("task-quiet", () => {});
    (await second)();
    (await third)();
    await drained();

    expect(core.listens).toHaveLength(1);
    expect(core.unlistens).toHaveLength(1);
    expect(core.held("task-quiet")).toBe(0);
  });

  it("listens anew for a part that comes after the last one's stop has gone to the core", async () => {
    const core = aCore();
    (await listen("task-quiet", () => {}))();
    await drained();
    expect(core.unlistens).toHaveLength(1);

    const heard: string[] = [];
    const stop = await listen("task-quiet", () => void heard.push("again"));
    core.says("task-quiet", null);

    expect(core.listens).toHaveLength(2);
    expect(heard).toEqual(["again"]);
    stop();
  });

  it("leaves nothing listening when every part stops before the core's listen answers", async () => {
    const core = aCore({ held: true });
    const heard: string[] = [];
    // Two effects cleaned up at once, before the listen they started answered.
    const stops = [
      untilStopped(listen("dispatch-arrival", () => void heard.push("one"))),
      untilStopped(listen("dispatch-arrival", () => void heard.push("two"))),
    ];
    for (const stop of stops) stop();

    core.answer();
    await drained();
    core.says("dispatch-arrival", null);

    expect(core.listens).toHaveLength(1);
    expect(core.unlistens.map((one) => one.event)).toEqual(["dispatch-arrival"]);
    expect(core.held("dispatch-arrival")).toBe(0);
    expect(heard).toEqual([]);
  });

  it("refuses every part waiting on a listen the core refused, and asks again for the next", async () => {
    const core = aCore({ held: true });
    core.refuse();
    const first = listen("plane-changed", () => {});
    const second = listen("plane-changed", () => {});
    core.answer();
    await expect(first).rejects.toThrow("the bridge is gone");
    await expect(second).rejects.toThrow("the bridge is gone");

    core.accept();
    const stop = await listen("plane-changed", () => {});
    expect(core.listens).toHaveLength(2);
    expect(core.held("plane-changed")).toBe(1);
    stop();
  });

  it("listens anew under a new bridge, as each test's stand-in core is", async () => {
    aCore();
    const first = await listen("plane-changed", () => {});
    clearMocks();
    const core = aCore();
    const heard: string[] = [];
    const stop = await listen("plane-changed", () => void heard.push("heard"));
    core.says("plane-changed", null);

    expect(core.listens).toHaveLength(1);
    expect(heard).toEqual(["heard"]);
    stop();
    first();
  });
});
