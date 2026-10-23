// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { ENGINE_DOWNLOAD, type EngineDownloadShown } from "./bindings";
import { EngineDownloadNotice } from "./EngineDownloadNotice";

/**
 * **The browser engine's first download is shown as it goes, with Cancel** (#2207): one Notice
 * per download under way, drawn once the host says one started and gone once its start answers.
 */

function core(shown: () => EngineDownloadShown[]): { asked: [string, unknown][] } {
  const asked: [string, unknown][] = [];
  mockIPC(
    (cmd, args) => {
      asked.push([cmd, args]);
      if (cmd === "engine_downloads") return shown();
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked };
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("the notice of the browser engine's download", () => {
  it("is not drawn while nothing downloads", async () => {
    const { asked } = core(() => []);

    render(<EngineDownloadNotice />);

    await waitFor(() => expect(asked.map(([cmd]) => cmd)).toContain("engine_downloads"));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("is not drawn, and does not fail, where the core answers nothing", async () => {
    const { asked } = core(() => null as unknown as EngineDownloadShown[]);

    render(<EngineDownloadNotice />);

    await waitFor(() => expect(asked.map(([cmd]) => cmd)).toContain("engine_downloads"));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("follows the download as it goes, and leaves once its start answers", async () => {
    let now: EngineDownloadShown[] = [];
    core(() => now);
    render(<EngineDownloadNotice />);

    now = [{ program: "B1", says: "Downloading the browser engine… 3 of 152 MB." }];
    await act(() => emit(ENGINE_DOWNLOAD, null));
    expect((await screen.findByRole("status")).textContent).toContain("3 of 152 MB");

    now = [{ program: "B1", says: "Downloading the browser engine… 90 of 152 MB." }];
    await act(() => emit(ENGINE_DOWNLOAD, null));
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("90 of 152 MB"));

    now = [];
    await act(() => emit(ENGINE_DOWNLOAD, null));
    await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
  });

  it("cancels the download it shows, naming its browser", async () => {
    const { asked } = core(() => [
      { program: "B7", says: "Downloading the browser engine… 3 MB so far." },
    ]);
    render(<EngineDownloadNotice />);

    await userEvent.click(await screen.findByRole("button", { name: "Cancel download" }));

    await waitFor(() =>
      expect(asked).toContainEqual(["cancel_engine_download", { program: "B7" }]),
    );
  });
});
