// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { reposNotCloned } from "./actionResult";
import type { Said } from "./actions";
import { KEPT_RESULTS, keep, resultsFor, trackOf, useKeptResults } from "./keptResults";
import { cloneRepos, forgetRepoClones } from "./repoClones";

const said = (words: string) => ({ from: "x", refused: false, words });

describe("the results the window keeps (#3344)", () => {
  it("keeps each answer once, newest first", () => {
    const a = said("a");
    const b = said("b");
    let kept = keep([], a, "/p", 0);
    kept = keep(kept, a, "/p", 1);
    kept = keep(kept, b, "/p", 2);
    expect(kept.map((one) => one.said.words)).toEqual(["b", "a"]);
  });

  it("keeps at most the newest few", () => {
    let kept = keep([], said("0"), undefined, 0);
    for (let n = 1; n <= KEPT_RESULTS + 5; n += 1) kept = keep(kept, said(String(n)), undefined, n);
    expect(kept).toHaveLength(KEPT_RESULTS);
    expect(kept[0].said.words).toBe(String(KEPT_RESULTS + 5));
  });

  it("lists a project's own results and the window's in its Inbox", () => {
    const kept = [
      { id: 1, said: said("mine"), plane: "/a" },
      { id: 2, said: said("theirs"), plane: "/b" },
      { id: 3, said: said("window"), plane: undefined },
    ];
    expect(resultsFor(kept, "/a").map((one) => one.said.words)).toEqual(["mine", "window"]);
  });

  it("never keeps a line that only says the core has not answered yet", () => {
    const late = { from: "answer.late.createProject", refused: true, words: "Not yet." };
    expect(keep([], late, "/p", 0)).toEqual([]);
  });
});

describe("the kept results, as the window holds them (#3344's review)", () => {
  it("does not keep again a result put away while it is still the newest", () => {
    const said = { from: "x", refused: false, words: "Done." };
    const { result, rerender } = renderHook(({ now }) => useKeptResults(now, "/p"), {
      initialProps: { now: said as Said | undefined },
    });
    expect(result.current.kept).toHaveLength(1);
    act(() => result.current.drop(result.current.kept[0].id));
    rerender({ now: said });
    expect(result.current.kept).toEqual([]);
  });

  it("keeps a progress line on a track, and its result takes its place, in place (#3649)", () => {
    const ready = { from: "workspace.create", refused: false, words: "Ready.", workspace: "g" };
    const cloning = {
      from: "workspace.clone",
      refused: false,
      words: "Cloning…",
      progress: true,
      track: "clone:g",
    };
    const halfway = { ...cloning, words: "1 of 2 cloned." };
    const cloned = { from: "workspace.clone", refused: false, words: "Cloned.", track: "clone:g" };
    const later = { from: "x", refused: false, words: "Later." };
    let kept = keep([], ready, "/p", 0);
    kept = keep(kept, cloning, "/p", 1);
    kept = keep(kept, later, "/p", 2);
    kept = keep(kept, halfway, "/p", 3);
    kept = keep(kept, cloned, "/p", 4);
    expect(kept).toEqual([
      { id: 2, said: later, plane: "/p" },
      { id: 1, said: cloned, plane: "/p" },
      { id: 0, said: ready, plane: "/p" },
    ]);
    // Another project's track of the same name is its own.
    expect(keep(kept, cloning, "/q", 5).map((one) => one.id)).toEqual([5, 2, 1, 0]);
  });

  it("keeps the line said just before, in the same breath, first (#3649)", () => {
    const ready = { from: "workspace.create", refused: false, words: "Ready.", workspace: "g" };
    const cloning = {
      from: "workspace.clone",
      refused: false,
      words: "Cloning…",
      progress: true,
      track: "clone:g",
      after: ready,
    };
    expect(keep([], cloning, "/p", 7)).toEqual([
      { id: 8, said: cloning, plane: "/p" },
      { id: 7, said: ready, plane: "/p" },
    ]);
    // Never twice, where it was kept already.
    const once = keep([], ready, "/p", 0);
    expect(keep(once, cloning, "/p", 1).map((one) => one.id)).toEqual([1, 0]);
  });

  it("keeps no progress line on no track", () => {
    const cloning = { from: "workspace.create", refused: false, words: "Cloning…", progress: true };
    expect(keep([], cloning, "/p", 0)).toEqual([]);
  });
});

describe("a track's lines, kept (#3649's review)", () => {
  const ready: Said = { from: "workspace.create", refused: false, words: "Ready.", workspace: "g" };
  const counting = (words: string, after?: Said): Said => ({
    from: "workspace.clone",
    refused: false,
    words,
    progress: true,
    track: "clone:g:1",
    after,
  });
  const cloned: Said = {
    from: "workspace.clone",
    refused: false,
    words: "Cloned.",
    track: "clone:g:1",
  };

  it("gives every line its own id when the first one said brings the line before it", () => {
    const first = counting("Cloning…", ready);
    const later = said("later");
    const { result, rerender } = renderHook(({ now }) => useKeptResults(now, "/p"), {
      initialProps: { now: first as Said | undefined },
    });
    rerender({ now: later });
    const ids = result.current.kept.map((one) => one.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(result.current.kept.map((one) => one.said)).toEqual([later, first, ready]);
  });

  it("never keeps more than its bound, on the path that brings a line before and replaces", () => {
    const full = Array.from({ length: KEPT_RESULTS }, (_, at) => ({
      id: at,
      said: at === KEPT_RESULTS - 1 ? counting("Cloning…") : said(`old ${at}`),
      plane: "/p",
    }));
    const next = keep(full, { ...cloned, after: ready }, "/p", KEPT_RESULTS);
    expect(next).toHaveLength(KEPT_RESULTS);
    expect(next[0].said).toBe(ready);
  });

  it("keeps no more counting lines on a track whose line was dismissed, but keeps its result", () => {
    const first = counting("Cloning…");
    const { result, rerender } = renderHook(({ now }) => useKeptResults(now, "/p"), {
      initialProps: { now: first as Said | undefined },
    });
    act(() => result.current.drop(result.current.kept[0].id));
    rerender({ now: counting("1 of 2 done.") });
    expect(result.current.kept).toEqual([]);
    rerender({ now: cloned });
    expect(result.current.kept.map((one) => one.said)).toEqual([cloned]);
  });

  it("ends with its run: another run's first line on another track is a notice of its own", () => {
    let kept = keep([], cloned, "/p", 0);
    kept = keep(kept, { ...counting("Cloning…"), track: "clone:g:2" }, "/p", 1);
    expect(kept.map((one) => one.id)).toEqual([1, 0]);
  });
});

describe("a dismissed track said on again (#3898)", () => {
  it("still keeps the line said before it, though not the track's counting", () => {
    const ready: Said = {
      from: "workspace.create",
      refused: false,
      words: "Ready.",
      workspace: "g",
    };
    const counting: Said = {
      from: "workspace.clone",
      refused: false,
      words: "Cloning…",
      progress: true,
      track: "clone:g:1",
    };
    const dropped = new Set([trackOf(counting, "/p")]);
    const kept = keep([], { ...counting, after: ready }, "/p", 0, dropped);
    expect(kept.map((one) => one.said)).toEqual([ready]);
  });
});

describe("a failed clone's line, once its repo lands (#4394)", () => {
  afterEach(() => {
    forgetRepoClones();
    clearMocks();
  });

  const failed = (workspace: string, ...repos: string[]): Said =>
    reposNotCloned(
      workspace,
      repos.map((repo) => ({ repo, said: `${repo} refused.` })),
      () => undefined,
      1,
    );

  const lands = async (plane: string, workspace: string, repo: string) => {
    mockIPC((cmd) => (cmd === "clone_repo" ? ["landed"] : null));
    await act(async () => {
      await cloneRepos(plane, workspace, [repo]);
    });
  };

  const keeping = (first: Said) =>
    renderHook(({ now }) => useKeptResults(now, "/p"), {
      initialProps: { now: first as Said | undefined },
    });

  it("goes with the last repo it names", async () => {
    const { result } = keeping(failed("alpha", "api"));
    await lands("/p", "alpha", "api");
    expect(result.current.kept).toEqual([]);
  });

  it("keeps its place and its track, naming the repos left", async () => {
    const line = failed("alpha", "api", "web");
    const { result } = keeping(line);
    const { id } = result.current.kept[0];
    await lands("/p", "alpha", "api");
    expect(result.current.kept).toHaveLength(1);
    expect(result.current.kept[0].id).toBe(id);
    expect(result.current.kept[0].said.track).toBe(line.track);
    expect(result.current.kept[0].said.title).toBe("Could not clone web into alpha");
    expect(result.current.kept[0].said.words).toBe("web refused.");
  });

  it("stays for the same repo in another workspace, or in another project", async () => {
    const { result } = keeping(failed("alpha", "api"));
    await lands("/p", "beta", "api");
    await lands("/other", "alpha", "api");
    expect(result.current.kept).toHaveLength(1);
  });

  it("does not come back with the project that said it", async () => {
    const line = failed("alpha", "api");
    const { result, rerender } = keeping(line);
    await lands("/p", "alpha", "api");
    rerender({ now: said("another project's") });
    rerender({ now: line });
    expect(result.current.kept.map((one) => one.said.words)).toEqual(["another project's"]);
  });

  it("stays when a repo it does not name lands, and for a line about no clone", async () => {
    const other = said("Unrelated.");
    const { result, rerender } = keeping(other);
    rerender({ now: failed("alpha", "api") });
    await lands("/p", "alpha", "web");
    expect(result.current.kept).toHaveLength(2);
  });
});
