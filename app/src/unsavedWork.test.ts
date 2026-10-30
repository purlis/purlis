// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view's unsaved work, and which close would lose it** (#3929): the registry every close
 * asks through (`unsavedWork.ts`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  discardGone,
  discardRemoved,
  discardUnsavedIn,
  keepUnsaved,
  unsavedClosedBy,
  unsavedIn,
  unsavedNow,
  type UnsavedWork,
} from "./unsavedWork";
import { focusPane, noTabs, openView, settingsView, splitFocusedPane, type ViewRef } from "./tabs";

const STUDIO: ViewRef = { from: null, view: "skill-studio", key: "" };
const SETTINGS = settingsView("project");

function work(key: string, over: Partial<UnsavedWork> = {}): UnsavedWork {
  const one: UnsavedWork = {
    key,
    plane: "p",
    view: STUDIO,
    name: key,
    save: () => Promise.resolve(undefined),
    discard: vi.fn(() => keepUnsaved(key, undefined)),
    ...over,
  };
  keepUnsaved(key, one);
  return one;
}

afterEach(() => {
  for (const one of unsavedNow()) keepUnsaved(one.key, undefined);
});

describe("the registry", () => {
  it("holds what each view says, by its key, until the view takes it off", () => {
    const deploy = work("deploy");
    work("review", { plane: "q" });
    expect(unsavedIn("p")).toEqual([deploy]);
    keepUnsaved("deploy", undefined);
    expect(unsavedIn("p")).toEqual([]);
    expect(unsavedNow().map((one) => one.key)).toEqual(["review"]);
  });

  it("names a workspace's work for its removal, and no other", () => {
    const ops = work("ops", { workspace: "ops" });
    work("root");
    expect(unsavedIn("p", "ops")).toEqual([ops]);
    expect(unsavedIn("p", "web")).toEqual([]);
  });

  it("lets a project's work go when the project is let go of", () => {
    const deploy = work("deploy");
    const elsewhere = work("review", { plane: "q" });
    // A view that does not take itself off still goes.
    work("stuck", { discard: vi.fn() });
    discardUnsavedIn("p");
    expect(deploy.discard).toHaveBeenCalledOnce();
    expect(unsavedNow()).toEqual([elsewhere]);
  });
});

describe("what goes with what holds it", () => {
  it("is the work of a view no tab shows any more, once its tab has closed", () => {
    const before = openView(noTabs(), STUDIO, "Skill studio", "ops");
    const deploy = work("deploy");
    const notShown = work("settings", { view: SETTINGS });
    discardGone("p", before, before);
    discardGone("p", before, openView(before, SETTINGS, "Settings", "ops"));
    expect(deploy.discard).not.toHaveBeenCalled();

    discardGone("p", before, noTabs());
    expect(deploy.discard).toHaveBeenCalledOnce();
    // A view never shown was not closed.
    expect(notShown.discard).not.toHaveBeenCalled();
  });

  it("is a removed workspace's work", () => {
    const ops = work("ops", { workspace: "ops" });
    const web = work("web", { workspace: "web" });
    discardRemoved("p", "ops");
    expect(ops.discard).toHaveBeenCalledOnce();
    expect(web.discard).not.toHaveBeenCalled();
  });
});

describe("which close would lose it", () => {
  it("is the close of the tab that shows its view, in its project", () => {
    const tabs = openView(noTabs(), STUDIO, "Skill studio", "ops");
    const tab = tabs.order[0];
    const close = { verb: "closeTab", tab, ends: false } as const;
    expect(unsavedClosedBy("p", tabs, close)).toBeUndefined();

    const deploy = work("deploy");
    expect(unsavedClosedBy("p", tabs, close)).toEqual({ work: [deploy], tab });
    expect(unsavedClosedBy("q", tabs, close)).toBeUndefined();
    // Work in a view no tab of the close shows is not lost by it.
    keepUnsaved("deploy", undefined);
    work("settings", { view: SETTINGS });
    expect(unsavedClosedBy("p", tabs, close)).toBeUndefined();
  });

  it("is a pane's close only where the focused pane shows it", () => {
    const one = openView(noTabs(), STUDIO, "Skill studio", "ops");
    const tab = one.order[0];
    // A chat beside it, focused.
    const split = splitFocusedPane(one, "row", 5, "ops.1");
    const deploy = work("deploy");
    const close = { verb: "closePane", ends: false } as const;
    const studioPane = one.byId[tab].focused;
    expect(split.byId[tab].focused).not.toBe(studioPane);

    expect(unsavedClosedBy("p", split, close)).toBeUndefined();
    expect(unsavedClosedBy("p", focusPane(split, studioPane), close)).toEqual({
      work: [deploy],
      tab,
    });
    // The tab's close takes both panes.
    expect(unsavedClosedBy("p", split, { verb: "closeTab", tab, ends: false })).toEqual({
      work: [deploy],
      tab,
    });
  });

  it("is no close of anything but a tab or a pane", () => {
    const tabs = openView(noTabs(), STUDIO, "Skill studio", "ops");
    work("deploy");
    expect(unsavedClosedBy("p", tabs, { verb: "closeProject", plane: "p" })).toBeUndefined();
  });
});
