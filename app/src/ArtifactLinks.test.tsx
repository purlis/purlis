// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Offer } from "./actions";
import { ARTIFACT_LINK, ArtifactLinks, type ArtifactLinkFollowed } from "./ArtifactLinks";

type Call = { cmd: string; args: Record<string, unknown> };

/** The core, answering the link dialog's command with `answer` (nothing: it opened). */
function core(answer: (call: Call) => unknown = () => null) {
  const calls: Call[] = [];
  mockIPC(
    (cmd, args) => {
      const call = { cmd, args: args as Record<string, unknown> };
      calls.push(call);
      return answer(call);
    },
    { shouldMockEvents: true },
  );
  return calls;
}

const URL = "https://github.com/purlis/purlis/issues?q=is%3Aopen";

function asking(label = "artifact-7"): ArtifactLinkFollowed {
  return { label, url: URL, host: "github.com", refused: null, goes: null };
}

const opens = (calls: Call[]) => calls.filter((one) => one.cmd === "artifact_link_open");

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

describe("a link followed in an artifact's page", () => {
  it("asks first, showing its host and its whole address", async () => {
    core();
    render(<ArtifactLinks label="artifact-7" />);
    await emit(ARTIFACT_LINK, asking());
    const dialog = await screen.findByRole("dialog", { name: "Open a link to github.com?" });
    expect(dialog.textContent).toContain(URL);
    expect(dialog.textContent).toContain("github.com");
  });

  it("opens the link its page reported, once Open is pressed, and closes", async () => {
    const calls = core();
    render(<ArtifactLinks label="artifact-7" />);
    await emit(ARTIFACT_LINK, asking());
    await userEvent.click(await screen.findByRole("button", { name: "Open" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(opens(calls).map((one) => one.args)).toEqual([{ label: "artifact-7", always: false }]);
  });

  it("keeps its host on this machine when Always open is pressed", async () => {
    const calls = core();
    render(<ArtifactLinks label="artifact-7" />);
    await emit(ARTIFACT_LINK, asking());
    await userEvent.click(await screen.findByRole("button", { name: "Always open github.com" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(opens(calls).map((one) => one.args)).toEqual([{ label: "artifact-7", always: true }]);
  });

  it("opens nothing on Cancel", async () => {
    const calls = core();
    render(<ArtifactLinks label="artifact-7" />);
    await emit(ARTIFACT_LINK, asking());
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(opens(calls)).toEqual([]);
  });

  it("says why a link was refused, and offers nothing to open", async () => {
    const calls = core();
    render(<ArtifactLinks label="artifact-7" />);
    await emit(ARTIFACT_LINK, {
      label: "artifact-7",
      url: null,
      host: null,
      refused:
        "purlis opens only https links from an artifact, so it will not open this file: link",
    });
    const dialog = await screen.findByRole("dialog", { name: "Link not opened" });
    expect(dialog.textContent).toContain(
      "Purlis opens only https links from an artifact, so it will not open this file: link.",
    );
    expect(screen.queryByRole("button", { name: "Open" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(opens(calls)).toEqual([]);
  });

  it("says why the browser did not open, and stays", async () => {
    core(() => {
      throw "purlis could not open the link in your browser: no browser";
    });
    render(<ArtifactLinks label="artifact-7" />);
    await emit(ARTIFACT_LINK, asking());
    await userEvent.click(await screen.findByRole("button", { name: "Open" }));
    expect((await screen.findByRole("alert")).textContent).toBe(
      "purlis could not open the link in your browser: no browser",
    );
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("goes to a place in purlis without asking, by the window's own row", async () => {
    const calls = core();
    const pressed: Offer[] = [];
    render(<ArtifactLinks label="artifact-7" onPress={(offer) => pressed.push(offer)} />);
    await emit(ARTIFACT_LINK, {
      label: "artifact-7",
      url: null,
      host: null,
      refused: null,
      goes: { to: "workspace", workspace: "beta" },
    });
    await waitFor(() => expect(pressed).toHaveLength(1));
    expect(pressed[0].does).toEqual({ verb: "focusWorkspace", workspace: "beta" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(opens(calls)).toEqual([]);
  });

  it("says why a purlis:// link naming an action was refused, and goes nowhere", async () => {
    core();
    const pressed: Offer[] = [];
    render(<ArtifactLinks label="artifact-7" onPress={(offer) => pressed.push(offer)} />);
    await emit(ARTIFACT_LINK, {
      label: "artifact-7",
      url: null,
      host: null,
      refused:
        "a purlis link only opens a place, and this one names more after it (`done`), so purlis did not follow it",
      goes: null,
    });
    const dialog = await screen.findByRole("dialog", { name: "Link not opened" });
    expect(dialog.textContent).toContain("A purlis link only opens a place");
    expect(pressed).toEqual([]);
  });

  it("ignores a link another artifact's page followed", async () => {
    core();
    render(<ArtifactLinks label="artifact-7" />);
    await emit(ARTIFACT_LINK, asking("artifact-8"));
    await new Promise((done) => setTimeout(done, 50));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
