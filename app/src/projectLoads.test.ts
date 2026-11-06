// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  AT_ONCE,
  inTurn,
  loadOf,
  loadSaid,
  offersRetry,
  placed,
  railOrder,
  STILL_LOADING_AFTER,
} from "./projectLoads";

/** A promise and the hand that settles it, for an open a test answers when it likes. */
function held<T>() {
  let settle!: (value: T) => void;
  const promise = new Promise<T>((resolve) => (settle = resolve));
  return { promise, settle };
}

/** Lets every settled promise's callbacks run. */
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("projects opened in turn at a launch (#5323)", () => {
  it("asks the front project first, then the rest in their order", () => {
    const asked: string[] = [];
    inTurn(["a", "b", "c"], "c", (path) => {
      asked.push(path);
      return new Promise(() => undefined);
    });
    expect(asked).toEqual(["c", "a", "b"]);
  });

  it("has no more than four open at once, and starts the next as one answers", async () => {
    const opens = new Map<string, ReturnType<typeof held<string>>>();
    const asked: string[] = [];
    inTurn(["a", "b", "c", "d", "e", "f"], undefined, (path) => {
      asked.push(path);
      const one = held<string>();
      opens.set(path, one);
      return one.promise;
    });
    expect(AT_ONCE).toBe(4);
    expect(asked).toEqual(["a", "b", "c", "d"]);
    opens.get("c")?.settle("c");
    await settled();
    expect(asked).toEqual(["a", "b", "c", "d", "e"]);
  });

  it("moves a waiting project chosen by the person to the front of the queue", async () => {
    const opens = new Map<string, ReturnType<typeof held<string>>>();
    const asked: string[] = [];
    const turn = inTurn(["a", "b", "c", "d", "e", "f"], undefined, (path) => {
      asked.push(path);
      const one = held<string>();
      opens.set(path, one);
      return one.promise;
    });
    turn.toFront("f");
    opens.get("a")?.settle("a");
    await settled();
    expect(asked).toEqual(["a", "b", "c", "d", "f"]);
  });

  it("is done once every open has answered, however one of them came out", async () => {
    const turn = inTurn(["a", "b"], undefined, (path) =>
      path === "a" ? Promise.reject(new Error("no")) : Promise.resolve(path),
    );
    await expect(turn.done).resolves.toEqual([undefined, "b"]);
  });
});

describe("what a project's row says of its loading (#5323)", () => {
  it("says it is loading, then that it is still loading after 15 s", () => {
    const load = { kind: "loading", since: 1_000 } as const;
    expect(loadSaid(load, 1_000)).toBe("Loading…");
    expect(loadSaid(load, 1_000 + STILL_LOADING_AFTER - 1)).toBe("Loading…");
    expect(loadSaid(load, 1_000 + STILL_LOADING_AFTER)).toBe("Still loading…");
  });

  it("says how many chats came back when only some did, and nothing when all did", () => {
    expect(loadSaid({ kind: "ready", back: 2, wanted: 3 }, 0)).toBe("2 of 3 chats back");
    expect(loadSaid({ kind: "ready", back: 3, wanted: 3 }, 0)).toBeUndefined();
  });

  it("takes the core's word for how a project's opening came out", () => {
    expect(loadOf({ kind: "ready", back: 1, wanted: 2 })).toEqual({
      kind: "ready",
      back: 1,
      wanted: 2,
    });
    expect(loadOf({ kind: "failed", error: "gone" })).toEqual({ kind: "failed", error: "gone" });
  });
});

describe("the rail's order while projects load (#5323)", () => {
  it("keeps the remembered order whichever project answers first", () => {
    expect(railOrder(["a", "b", "c"], ["c", "a"], ["b"])).toEqual(["a", "b", "c"]);
  });

  it("puts a project opened by hand after the remembered ones", () => {
    expect(railOrder(["a", "b"], ["x", "a"], ["b"])).toEqual(["a", "b", "x"]);
  });

  it("leaves out a remembered project that neither opened nor is still coming", () => {
    expect(railOrder(["a", "b"], ["a"], [])).toEqual(["a"]);
  });
});

describe("a project put in its remembered place as its open answers (#5323)", () => {
  it("goes before the first one held that was remembered after it", () => {
    expect(placed(["a", "c"], "b", ["a", "b", "c"])).toEqual(["a", "b", "c"]);
    expect(placed(["c"], "a", ["a", "b", "c"])).toEqual(["a", "c"]);
  });

  it("goes last when nothing held was remembered after it, or it was not remembered", () => {
    expect(placed(["a"], "c", ["a", "b", "c"])).toEqual(["a", "c"]);
    expect(placed(["x", "a"], "y", ["a"])).toEqual(["x", "a", "y"]);
  });

  it("keeps a project already held where it is", () => {
    expect(placed(["b", "a"], "a", ["a", "b"])).toEqual(["b", "a"]);
  });
});

describe("what a row offers Retry on (#5324)", () => {
  it("offers it on a failure, a slow load and a project short of chats, and on nothing else", () => {
    const now = 100_000;
    expect(offersRetry({ kind: "failed", error: "No." }, now)).toBe(true);
    expect(offersRetry({ kind: "loading", since: now - 15_000 }, now)).toBe(true);
    expect(offersRetry({ kind: "ready", back: 2, wanted: 3 }, now)).toBe(true);
    expect(offersRetry({ kind: "loading", since: now - 14_999 }, now)).toBe(false);
    expect(offersRetry({ kind: "ready", back: 3, wanted: 3 }, now)).toBe(false);
    expect(offersRetry(undefined, now)).toBe(false);
  });

  it("says a failure's error as it came", () => {
    expect(loadSaid({ kind: "failed", error: "The record could not be read." }, 0)).toBe(
      "The record could not be read.",
    );
  });
});
