// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

import type { Offer } from "./actions";
import type { PlaneId, WindowDraft } from "./bindings";
import { CHOOSE_DRAFTS, DRAFTS_SAY, EndingChat } from "./EndingChat";
import { KEPT_REFUSED_WAITS, ThemeDraftStrip } from "./ThemeDraftStrip";
import { forgetDraftHere, isRevertKey, setDraftHere } from "./themeDrafts";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **The theme draft strip** (#2332): the window names a chat's draft where it is drawn, with
 * Keep, Revert and Compare, and the revert key drops the drafts there. The core decides what is
 * drawn (`purlis_core::theme::drafts`); these hold what the window shows and what it asks.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetDraftHere();
});

const PLANE = "/home/dev/plane" as PlaneId;
const DRAFT: WindowDraft = {
  folder: "dusk",
  session: 3,
  chat: "steward 3",
  workspace: "alpha",
  kept_to: "workspace",
  replaces: false,
  changed: false,
  drawn: true,
  broken: [],
  refused: [],
  review: null,
  kept_refused: false,
};

function asked(): { cmd: string; args: Record<string, unknown> }[] {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "theme_draft_keep")
      return "The theme dusk is kept in the workspace alpha's themes.";
    if (cmd === "theme_drafts_revert_here") return 1;
    return null;
  });
  return calls;
}

describe("the theme draft strip", () => {
  it("names the draft and its chat where it is drawn, and nowhere else", () => {
    asked();
    act(() => setDraftHere({ plane: PLANE, workspace: "alpha", draft: DRAFT }));
    const { rerender } = render(<ThemeDraftStrip plane={PLANE} workspace="alpha" />);
    const strip = screen.getByRole("region", { name: "Theme draft" });
    expect(strip.textContent).toContain("Theme draft — dusk, edited by steward 3");
    for (const words of ["Keep", "Revert", "Compare"])
      expect(screen.getByRole("button", { name: words })).toBeTruthy();
    rerender(<ThemeDraftStrip plane={PLANE} workspace="beta" />);
    expect(screen.queryByRole("region", { name: "Theme draft" })).toBeNull();
  });

  it("says which hard floor a save broke", () => {
    asked();
    act(() =>
      setDraftHere({
        plane: PLANE,
        workspace: "alpha",
        draft: { ...DRAFT, broken: ["primary text is readable on every surface: x"] },
      }),
    );
    render(<ThemeDraftStrip plane={PLANE} workspace="alpha" />);
    expect(screen.getByRole("region", { name: "Theme draft" }).textContent).toContain(
      "Its last save is not drawn, so its last good version is: primary text is readable",
    );
  });

  it("says that a refused kept theme's Notice waits while the draft is drawn (#2708)", () => {
    asked();
    act(() =>
      setDraftHere({ plane: PLANE, workspace: "alpha", draft: { ...DRAFT, kept_refused: true } }),
    );
    const { rerender } = render(<ThemeDraftStrip plane={PLANE} workspace="alpha" />);
    expect(screen.getByRole("region", { name: "Theme draft" }).textContent).toContain(
      KEPT_REFUSED_WAITS,
    );
    act(() => setDraftHere({ plane: PLANE, workspace: "alpha", draft: DRAFT }));
    rerender(<ThemeDraftStrip plane={PLANE} workspace="alpha" />);
    expect(screen.getByRole("region", { name: "Theme draft" }).textContent).not.toContain(
      KEPT_REFUSED_WAITS,
    );
  });

  it("asks the core to revert the draft, and to revert every draft here on the revert key", async () => {
    const calls = asked();
    act(() => setDraftHere({ plane: PLANE, workspace: "alpha", draft: DRAFT }));
    render(<ThemeDraftStrip plane={PLANE} workspace="alpha" />);
    fireEvent.click(screen.getByRole("button", { name: "Revert" }));
    await waitFor(() =>
      expect(calls.find((one) => one.cmd === "theme_draft_revert")?.args).toEqual({
        plane: PLANE,
        session: 3,
        folder: "dusk",
      }),
    );
    const mac = navigator.platform.startsWith("Mac");
    fireEvent.keyDown(window, {
      key: "Backspace",
      shiftKey: true,
      metaKey: mac,
      ctrlKey: !mac,
    });
    await waitFor(() =>
      expect(calls.find((one) => one.cmd === "theme_drafts_revert_here")?.args).toEqual({
        plane: PLANE,
        workspace: "alpha",
      }),
    );
  });

  it("keeps the draft, then asks once who draws it", async () => {
    const calls = asked();
    act(() => setDraftHere({ plane: PLANE, workspace: "alpha", draft: DRAFT }));
    render(<ThemeDraftStrip plane={PLANE} workspace="alpha" />);
    fireEvent.click(screen.getByRole("button", { name: "Keep" }));
    const question = await screen.findByRole("alertdialog");
    expect(question.textContent).toContain("Who draws dusk?");
    fireEvent.click(screen.getByRole("button", { name: "Only for me" }));
    await waitFor(() =>
      expect(calls.find((one) => one.cmd === "theme_choose")?.args).toEqual({
        plane: PLANE,
        workspace: "alpha",
        theme: "dusk",
        appearance: null,
        from: "workspace",
      }),
    );
    expect(calls.find((one) => one.cmd === "theme_draft_keep")?.args).toEqual({
      plane: PLANE,
      session: 3,
      folder: "dusk",
      keptTo: "workspace",
      replaces: false,
      changed: false,
    });
  });
});

describe("the revert key", () => {
  it("is ⌘⇧⌫ on a Mac and Ctrl+Shift+Backspace elsewhere, and nothing else", () => {
    const mac = navigator.platform.startsWith("Mac");
    const key = (more: Partial<KeyboardEvent>) =>
      isRevertKey({
        key: "Backspace",
        shiftKey: true,
        metaKey: false,
        ctrlKey: false,
        altKey: false,
        ...more,
      });
    expect(key(mac ? { metaKey: true } : { ctrlKey: true })).toBe(true);
    expect(key({})).toBe(false);
    expect(key({ shiftKey: false, metaKey: true, ctrlKey: true })).toBe(false);
    expect(key(mac ? { metaKey: true, altKey: true } : { ctrlKey: true, altKey: true })).toBe(
      false,
    );
  });
});

describe("closing a chat with a theme draft", () => {
  const offer = { id: "pane.close", title: "Close chat" } as unknown as Offer;

  it("asks Keep or Revert, and offers no close until it is answered", () => {
    const ended = vi.fn();
    render(
      <EndingChat
        offer={offer}
        drafts={[DRAFT]}
        onEnd={ended}
        onSmartClose={() => undefined}
        onCancel={() => undefined}
      />,
    );
    expect(screen.getByRole("radiogroup", { name: DRAFTS_SAY(["dusk"]) })).toBeTruthy();
    const close = screen.getByRole("button", { name: "Close" }) as HTMLButtonElement;
    expect(close).toHaveAttribute("aria-disabled", "true");
    expect(tipOf(close)).toBe(CHOOSE_DRAFTS);
    fireEvent.click(screen.getByRole("radio", { name: "Revert" }));
    expect(close.disabled).toBe(false);
    fireEvent.click(close);
    expect(ended).toHaveBeenCalledWith(false, false);
  });
});

describe("Keep into the project", () => {
  it("says before it is pressed that the theme goes to every workspace, and asks before it replaces one", async () => {
    const calls = asked();
    const draft: WindowDraft = { ...DRAFT, kept_to: "project", replaces: true };
    act(() => setDraftHere({ plane: PLANE, workspace: "alpha", draft }));
    render(<ThemeDraftStrip plane={PLANE} workspace="alpha" />);
    expect(screen.getByRole("region", { name: "Theme draft" }).textContent).toContain(
      "Keep puts it in the project's themes, for every workspace, replacing the dusk that is there.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Keep" }));
    const question = await screen.findByRole("alertdialog");
    expect(question.textContent).toContain("Replace dusk?");
    expect(calls.some((one) => one.cmd === "theme_draft_keep")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Replace" }));
    await waitFor(() =>
      expect(calls.find((one) => one.cmd === "theme_draft_keep")?.args).toEqual({
        plane: PLANE,
        session: 3,
        folder: "dusk",
        keptTo: "project",
        replaces: true,
        changed: false,
      }),
    );
    // Who draws it is asked of the project, where it went.
    const who = await screen.findByRole("alertdialog");
    await waitFor(() => expect(who.textContent).toContain("Make it the project's default"));
  });

  it("is said in the close question too", () => {
    render(
      <EndingChat
        offer={{ id: "pane.close", title: "Close chat" } as unknown as Offer}
        drafts={[{ ...DRAFT, kept_to: "project", replaces: false }]}
        onEnd={() => undefined}
        onSmartClose={() => undefined}
        onCancel={() => undefined}
      />,
    );
    expect(document.body.textContent).toContain(
      "Keep puts it in the project's themes, for every workspace.",
    );
  });
});

describe("Keep over a folder that changed since the draft started (#2708)", () => {
  it("says so before it replaces it, and sends that it was shown", async () => {
    const calls = asked();
    const draft: WindowDraft = { ...DRAFT, replaces: true, changed: true };
    act(() => setDraftHere({ plane: PLANE, workspace: "alpha", draft }));
    render(<ThemeDraftStrip plane={PLANE} workspace="alpha" />);
    fireEvent.click(screen.getByRole("button", { name: "Keep" }));
    const question = await screen.findByRole("alertdialog");
    expect(question.textContent).toContain(
      "replacing the dusk that is there, which changed since this draft started.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Replace" }));
    await waitFor(() =>
      expect(calls.find((one) => one.cmd === "theme_draft_keep")?.args).toEqual({
        plane: PLANE,
        session: 3,
        folder: "dusk",
        keptTo: "workspace",
        replaces: true,
        changed: true,
      }),
    );
  });

  it("is said in the close question too", () => {
    render(
      <EndingChat
        offer={{ id: "pane.close", title: "Close chat" } as unknown as Offer}
        drafts={[{ ...DRAFT, replaces: true, changed: true }]}
        onEnd={() => undefined}
        onSmartClose={() => undefined}
        onCancel={() => undefined}
      />,
    );
    expect(document.body.textContent).toContain("which changed since this draft started");
  });
});
