import { afterEach, describe, expect, it } from "vitest";
import { MOST_FOLDS, foldsToAdopt, keepFolds, keptFolds, keptFoldsById } from "./chatFolds";
import { keepScope, keptScope } from "./chatsScope";
import { keptFacet } from "./projectViews";
import { GLOBAL } from "./windowprefs";

const ONE = "01JB6Q9X7ZK3M4N5P6R7S8T9VW";
const TWO = "01JB6Q9X7ZK3M4N5P6R7S8T9VX";

describe("the Chats list's folds across a relaunch (#1687)", () => {
  afterEach(() => Reflect.deleteProperty(globalThis, GLOBAL));

  it("keeps each listed chat's fold by its id in the project's entry, beside its scope", () => {
    keepScope("/p/one", "all");
    keepFolds(
      "/p/one",
      new Map([
        [3, true],
        [4, false],
      ]),
      new Map([
        [3, ONE],
        [4, TWO],
      ]),
    );

    expect(keptFacet("/p/one", "chats")).toEqual({
      scope: "all",
      folds: { [ONE]: true, [TWO]: false },
    });
    expect(keptScope("/p/one")).toBe("all");
    // The scope set back to the default leaves the folds.
    keepScope("/p/one", "workspace");
    expect(keptFacet("/p/one", "chats")).toEqual({ folds: { [ONE]: true, [TWO]: false } });
  });

  it("gives a relaunched list's chats their folds back by id, under their new numbers, once", () => {
    keepFolds("/p/one", new Map([[3, true]]), new Map([[3, ONE]]));

    // The next launch deals the chat number 8.
    const listed = new Map([[8, ONE]]);
    expect([...foldsToAdopt("/p/one", listed, new Set())]).toEqual([[8, true]]);
    expect(foldsToAdopt("/p/one", listed, new Set([ONE])).size).toBe(0);
  });

  it("keeps the fold of a chat not listed yet, and drops a listed chat's fold once it is cleared", () => {
    keepFolds("/p/one", new Map([[3, true]]), new Map([[3, ONE]]));
    // A launch whose list has only the other chat so far.
    keepFolds("/p/one", new Map(), new Map([[5, TWO]]));
    expect([...keptFoldsById("/p/one")]).toEqual([[ONE, true]]);
    // Its fold cleared by the list (a chat that starts asking opens), now that it is listed.
    keepFolds("/p/one", new Map(), new Map([[9, ONE]]));
    expect(keptFoldsById("/p/one").size).toBe(0);
    expect(keptFacet("/p/one", "chats")).toBeUndefined();
  });

  it("writes nothing to the file before any chat is listed", () => {
    keepFolds("/p/one", new Map([[3, true]]), new Map([[3, ONE]]));
    keepFolds("/p/one", new Map());
    expect(keptFoldsById("/p/one").get(ONE)).toBe(true);
  });

  it("keeps a chat with no id by its number for a reload only", () => {
    keepFolds("/p/one", new Map([[3, true]]), new Map());
    expect(keptFolds("/p/one").get(3)).toBe(true);
    expect(keptFacet("/p/one", "chats")).toBeUndefined();
  });

  it("holds the entry to the folds set last", () => {
    const folds = new Map<number, boolean>();
    const listed = new Map<number, string>();
    for (let n = 1; n <= MOST_FOLDS + 10; n += 1) {
      folds.set(n, true);
      listed.set(n, `01JB6Q9X7ZK3M4N5P6R7S8${String(n).padStart(4, "0")}`);
    }
    keepFolds("/p/one", folds, listed);

    const kept = keptFoldsById("/p/one");
    expect(kept.size).toBe(MOST_FOLDS);
    expect(kept.has(listed.get(1) ?? "")).toBe(false);
    expect(kept.has(listed.get(MOST_FOLDS + 10) ?? "")).toBe(true);
  });

  it("reads the file's folds as ids and answers and nothing else", () => {
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "/cfg/layout.json",
        found: true,
        document: {
          version: 2,
          regions: [],
          projects: {
            "/p/one": { chats: { folds: { [ONE]: true, [TWO]: "yes", ["x".repeat(200)]: false } } },
            "/p/two": { chats: { folds: [ONE] } },
          },
        },
        trouble: null,
      },
      theme: { path: "", found: false, document: null, trouble: null },
    };

    expect([...keptFoldsById("/p/one")]).toEqual([[ONE, true]]);
    expect(keptFoldsById("/p/two").size).toBe(0);
  });
});
