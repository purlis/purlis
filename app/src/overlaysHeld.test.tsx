// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, type ComponentPropsWithoutRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";
import { cleanup } from "@testing-library/react";
import * as Radix from "@radix-ui/react-dialog";
import * as Ours from "./overlays/dialog";
import { PACKAGES, partsNotHeld } from "./overlaysHeld.testkit";

/**
 * **The guard on `overlays/` finds a part that draws by what it is, not by its name** (#1979).
 *
 * `projectShown.guard.test.tsx` holds each of the window's overlay files to it; here it is held
 * to the ways a file could hand on a part that draws over the window without asking whether its
 * project is shown, each written into a copy of the dialog's file. The menus and the popovers are
 * the menu and popover families now (`ui/`, #2909), held by `projectShown.guard.test.tsx`.
 */

let reported: MockInstance;
beforeEach(() => {
  // React reports the error a part not written for this throws; the guard counts it.
  reported = vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => {
  cleanup();
  reported.mockRestore();
});

const check = (ours: Record<string, unknown>, real: Record<string, unknown> = Radix) =>
  partsNotHeld({ ...PACKAGES.dialog, real }, ours);

/** A part written by hand around Radix's content, as a new overlay might be. */
const Popup = forwardRef<HTMLDivElement, ComponentPropsWithoutRef<typeof Radix.Content>>(
  function Popup(props, ref) {
    return <Radix.Content ref={ref} aria-describedby={undefined} {...props} />;
  },
);

describe("the overlays guard", () => {
  it("finds nothing in the window's own file", () => {
    expect(check(Ours)).toEqual([]);
  });

  it("fails on a part handed on under a name nobody listed, whatever it is called", () => {
    expect(check({ ...Ours, Popup })).toEqual([expect.stringContaining("Popup")]);
    expect(check({ ...Ours, Sheet: Radix.Content })).toEqual([expect.stringContaining("Sheet")]);
  });

  it("fails on a listed part that draws and is not held back while its project is behind", () => {
    expect(check({ ...Ours, Content: Popup })).toEqual([expect.stringContaining("Content")]);
    expect(check({ ...Ours, Overlay: Radix.Overlay })).toEqual([
      expect.stringContaining("Overlay"),
    ]);
  });

  it("fails on a long name Radix gives a part, held back or not", () => {
    expect(check({ ...Ours, DialogPortal: Radix.DialogPortal })).toEqual([
      expect.stringContaining("DialogPortal"),
    ]);
  });

  it("fails on a part the package gained that nobody has said whether it draws", () => {
    const Sheet = () => null;
    expect(check(Ours, { ...Radix, Sheet, DialogSheet: Sheet })).toEqual([
      expect.stringContaining("Sheet"),
    ]);
  });

  it("fails on a part that does not draw handed on as something other than Radix's", () => {
    expect(check({ ...Ours, Title: Popup })).toEqual([expect.stringContaining("Title")]);
  });
});
