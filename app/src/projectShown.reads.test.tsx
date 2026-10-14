// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PlaneId } from "./bindings";
import { useChatUsage, WHILE_RUNNING_MS } from "./ChatGauge";
import { PersonaGrantsNotice } from "./PersonaGrantsNotice";
import { ProjectShownProvider } from "./projectShown";
import { RegionFrame } from "./RegionFrame";
import { DEFAULT_ARRANGEMENT } from "./regions";
import { useOlderSandbox } from "./SandboxChanged";
import { SandboxOffer } from "./SandboxOffer";

/**
 * **What a part of a project's view reads, and holds of the window, while its project is kept
 * behind another** (`projectShown.tsx`): each part the review of #1940 named, one by one. A view
 * that is kept is not built again, so a read made only as a part mounts is made once for as
 * long as the project is kept, and a part that reads on a beat or holds a key of the window's
 * goes on doing so for a project nobody is looking at, unless it asks whether it is shown.
 */

const PLANE = "/home/dev/plane" as PlaneId;

let asked: string[];
beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  asked = [];
  mockIPC((cmd) => {
    asked.push(cmd);
    if (cmd === "plugin:event|listen") return 1;
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

const sent = (cmd: string) => asked.filter((one) => one === cmd).length;
/** Lets what was asked be answered, and what waits for the next paint run. */
const painted = () =>
  act(async () => {
    await vi.advanceTimersByTimeAsync(60);
  });
const inAProject = (shown: boolean, part: React.ReactElement) => (
  <ProjectShownProvider value={shown}>{part}</ProjectShownProvider>
);

describe("a read made only as a part is drawn", () => {
  it.each([
    ["the sandbox offer", "sandbox_state", <SandboxOffer plane={PLANE} />],
    [
      "a chat that holds another's grants",
      "persona_grants_held",
      <PersonaGrantsNotice
        plane={PLANE}
        session={7}
        running={false}
        owed={false}
        onRestart={() => {}}
      />,
    ],
  ])("is made again as its project comes back, after it is painted: %s", async (_, cmd, part) => {
    const { rerender } = render(inAProject(true, part));
    await painted();
    expect(sent(cmd)).toBe(1);

    rerender(inAProject(false, part));
    await painted();
    expect(sent(cmd)).toBe(1);

    rerender(inAProject(true, part));
    // Not before the project is on screen.
    expect(sent(cmd)).toBe(1);
    await painted();
    expect(sent(cmd)).toBe(2);
  });
});

describe("a chat's gauge", () => {
  it("is not read on its beat for a project behind, and is read after the paint as it comes back", async () => {
    let shown = true;
    const { rerender } = renderHook(() => useChatUsage(PLANE, 3, 1, true), {
      wrapper: ({ children }) => (
        <ProjectShownProvider value={shown}>{children}</ProjectShownProvider>
      ),
    });
    await painted();
    const before = sent("chat_usage");
    expect(before).toBeGreaterThan(0);

    shown = false;
    rerender();
    // `running` is what it was as the project left: nothing says the turn ended over there.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(WHILE_RUNNING_MS * 4);
    });
    expect(sent("chat_usage")).toBe(before);

    shown = true;
    rerender();
    // **Not in the switch.** The core answers this on the window's own thread, and every pane
    // of the tab in front has a gauge: one each, ahead of the paint, is what a switch waited on.
    expect(sent("chat_usage")).toBe(before);
    await painted();
    expect(sent("chat_usage")).toBe(before + 1);

    // And from there on its beat again, as before it left.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(WHILE_RUNNING_MS);
    });
    expect(sent("chat_usage")).toBe(before + 2);
  });

  it("is still read at once when its chat moves, and when its pane opens", async () => {
    const { rerender } = renderHook(
      ({ moved }: { moved: number }) => useChatUsage(PLANE, 3, moved, false),
      {
        initialProps: { moved: 1 },
      },
    );
    expect(sent("chat_usage")).toBe(1);
    rerender({ moved: 2 });
    expect(sent("chat_usage")).toBe(2);
  });
});

describe("the chats on an older sandbox", () => {
  it("are asked about for the project in front alone, and once as another comes to the front", async () => {
    const { rerender } = renderHook(
      ({ shown }: { shown: boolean }) => useOlderSandbox(PLANE, "asked", shown),
      { initialProps: { shown: false } },
    );
    await painted();
    act(() => void fireEvent.focus(window));
    await painted();
    expect(sent("chats_on_older_sandbox")).toBe(0);

    rerender({ shown: true });
    await painted();
    expect(sent("chats_on_older_sandbox")).toBe(1);
    act(() => void fireEvent.focus(window));
    await painted();
    expect(sent("chats_on_older_sandbox")).toBe(2);
  });
});

describe("F6, the window's key for the next region", () => {
  const frame = (
    <RegionFrame
      arrangement={DEFAULT_ARRANGEMENT}
      content={{ navigation: <div>navigation</div>, aside: <div>aside</div> }}
      views={{ chats: <div>chats</div> }}
      centre={<button type="button">a pane</button>}
      onResized={() => {}}
      keyboard
    />
  );
  /** Whether a press of F6 was taken by a frame: it stops the key there. */
  function f6Taken(): boolean {
    const key = new KeyboardEvent("keydown", { key: "F6", bubbles: true, cancelable: true });
    act(() => void document.body.dispatchEvent(key));
    return key.defaultPrevented;
  }

  it("is held by the frame of the project in front, and let go by one kept behind", () => {
    // The frame is not drawn again while its project is behind, so the `keyboard` it was last
    // given still says it is in front: it reads whether it is shown for itself.
    const { rerender } = render(inAProject(true, frame));
    expect(f6Taken()).toBe(true);

    rerender(inAProject(false, frame));
    expect(f6Taken()).toBe(false);

    rerender(inAProject(true, frame));
    expect(f6Taken()).toBe(true);
    expect(screen.getByRole("button", { name: "a pane" })).toBeInTheDocument();
  });
});
