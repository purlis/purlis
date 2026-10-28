// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view tab's own history** (#3667): back and forward walk it and stop at its ends, it is the
 * tab's and no other's, and a place reached from outside the tab gives a back to where the person
 * was inside it, or none.
 */
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import {
  forgetViewHistory,
  openViewAt,
  replaced,
  started,
  useViewHistory,
  wentBack,
  wentForward,
  wentTo,
} from "./viewHistory";

afterEach(() => {
  cleanup();
  for (const tab of ["one", "two", "studio"]) forgetViewHistory(tab);
});

describe("the steps", () => {
  it("go, back and forward, each stopping at its end", () => {
    let history = started("Library");
    expect(wentBack(history)).toBe(history);
    history = wentTo(history, "deploy");
    history = wentTo(history, "Grades");
    expect(history).toEqual({ back: ["Library", "deploy"], at: "Grades", forward: [] });
    history = wentBack(wentBack(history));
    expect(history).toEqual({ back: [], at: "Library", forward: ["deploy", "Grades"] });
    history = wentForward(history);
    expect(history.at).toBe("deploy");
    expect(wentForward(wentForward(history)).at).toBe("Grades");
    expect(wentForward(wentForward(wentForward(history)))).toEqual(
      wentForward(wentForward(history)),
    );
  });

  it("forget what was ahead on a new step, and take no step to where the person is", () => {
    const history = wentBack(wentTo(started("Library"), "deploy"));
    expect(wentTo(history, "Sources")).toEqual({
      back: ["Library"],
      at: "Sources",
      forward: [],
    });
    expect(wentTo(history, "Library")).toBe(history);
  });

  it("say again where the person is with no new step", () => {
    const history = wentTo(started({ page: "deploy", tab: "overview" }), {
      page: "deploy",
      tab: "grades",
    });
    expect(replaced(history, { page: "deploy", tab: "runs" })).toEqual({
      back: [{ page: "deploy", tab: "overview" }],
      at: { page: "deploy", tab: "runs" },
      forward: [],
    });
  });
});

describe("a tab's history", () => {
  it("is kept by the tab, found again when its view draws again, and shared by no other", () => {
    const first = renderHook(() => useViewHistory("one", "Library"));
    act(() => first.result.current.go("deploy"));
    expect(first.result.current).toMatchObject({ at: "deploy", canGoBack: true });
    first.unmount();
    // The tab back in front: its view draws again and finds where it was.
    const again = renderHook(() => useViewHistory("one", "Library"));
    expect(again.result.current).toMatchObject({ at: "deploy", canGoBack: true });
    const other = renderHook(() => useViewHistory("two", "Library"));
    expect(other.result.current).toMatchObject({
      at: "Library",
      canGoBack: false,
      canGoForward: false,
    });
    act(() => again.result.current.back());
    expect(again.result.current).toMatchObject({
      at: "Library",
      canGoBack: false,
      canGoForward: true,
    });
    act(() => again.result.current.forward());
    expect(again.result.current.at).toBe("deploy");
  });

  it("goes with its tab: closed, a new one starts afresh", () => {
    const first = renderHook(() => useViewHistory("one", "Library"));
    act(() => first.result.current.go("deploy"));
    first.unmount();
    forgetViewHistory("one");
    const again = renderHook(() => useViewHistory("one", "Library"));
    expect(again.result.current).toMatchObject({ at: "Library", canGoBack: false });
  });
});

describe("a place reached from outside the tab", () => {
  it("on a tab with a history, gives a back to where the person was inside it", () => {
    const view = renderHook(() => useViewHistory("studio", "Library"));
    act(() => view.result.current.go("deploy"));
    // The run chip's Runs, from a chat.
    act(() => openViewAt("studio", "deploy › Runs"));
    expect(view.result.current.at).toBe("deploy › Runs");
    act(() => view.result.current.back());
    expect(view.result.current.at).toBe("deploy");
  });

  it("on a tab with none, is where its history starts, with no back", () => {
    openViewAt("studio", "deploy › Runs");
    const view = renderHook(() => useViewHistory("studio", "Library"));
    expect(view.result.current).toMatchObject({ at: "deploy › Runs", canGoBack: false });
  });
});
