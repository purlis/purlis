import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { AlertRow, PlaneAlerts } from "./bindings";
import { REREAD_EVERY_MS, useAlerts } from "./alerts";
import { forgetShown, windowShown } from "./test-shown";

/**
 * **How the alerts' reading is kept fresh**: the reading every project's Inbox lists its alerts
 * from (#1695).
 */

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

const A = "/home/dev/a";
const B = "/home/dev/b";

function row(subject: string): AlertRow {
  return { severity: "warn", subject, detail: "d", way: { kind: "saving" } };
}

function plane(at: string, alerts: AlertRow[], stopped: string | null = null): PlaneAlerts {
  return { plane: at, alerts, stopped };
}

describe("the reading", () => {
  it("asks the core once, for every project and no project in particular", async () => {
    const asked: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      return [plane(A, [row("reinit")])];
    });
    const { result } = renderHook(() => useAlerts([A]));

    await waitFor(() => expect(result.current.reading.at).toBe("read"));
    const mine = asked.filter((one) => one.cmd === "alerts_everywhere");
    expect(mine).toHaveLength(1);
    // Cross-project by construction: nothing names a plane.
    expect(mine[0].args ?? {}).toEqual({});
  });

  it("asks again when told to, and when the projects change", async () => {
    let asked = 0;
    mockIPC((cmd) => {
      if (cmd === "alerts_everywhere") asked += 1;
      return [];
    });
    const { result, rerender } = renderHook(({ planes }) => useAlerts(planes), {
      initialProps: { planes: [A] },
    });
    await waitFor(() => expect(asked).toBe(1));

    act(() => result.current.reread());
    await waitFor(() => expect(asked).toBe(2));

    rerender({ planes: [A, B] });
    await waitFor(() => expect(asked).toBe(3));
  });

  it("asks again when the window comes back into focus, and on its own once a minute", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let asked = 0;
    mockIPC((cmd) => {
      if (cmd === "alerts_everywhere") asked += 1;
      return [];
    });
    renderHook(() => useAlerts([A]));
    await waitFor(() => expect(asked).toBe(1));

    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => expect(asked).toBe(2));

    act(() => {
      vi.advanceTimersByTime(REREAD_EVERY_MS);
    });
    await waitFor(() => expect(asked).toBe(3));
  });

  it("reads nothing on its beat while the window is hidden, and once when it is shown (#1392)", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let asked = 0;
    mockIPC((cmd) => {
      if (cmd === "alerts_everywhere") asked += 1;
      return [];
    });
    try {
      renderHook(() => useAlerts([A]));
      await waitFor(() => expect(asked).toBe(1));

      act(() => windowShown(false));
      await act(async () => void vi.advanceTimersByTime(REREAD_EVERY_MS * 10));
      expect(asked).toBe(1);

      await act(async () => windowShown(true));
      await waitFor(() => expect(asked).toBe(2));
    } finally {
      forgetShown();
    }
  });

  it("says the core's words when the ask fails, and never makes up an empty reading", async () => {
    mockIPC(() => {
      throw "the registry is gone";
    });
    const { result } = renderHook(() => useAlerts([A]));

    await waitFor(() => expect(result.current.reading.at).toBe("failed"));
    expect(result.current.reading).toEqual({ at: "failed", why: "the registry is gone" });
  });

  it("does not take an answer that is not a list for a reading with nothing in it", async () => {
    mockIPC(() => null);
    const { result } = renderHook(() => useAlerts([A]));

    await waitFor(() => expect(result.current.reading.at).toBe("failed"));
  });
});
