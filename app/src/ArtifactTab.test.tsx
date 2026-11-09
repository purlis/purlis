// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactAbout, PlaneChanged } from "./bindings";
import { ArtifactTab } from "./ArtifactTab";
import { ARTIFACT_KEY, artifactOf, artifactView, coveredByAnOverlay } from "./artifacts";
import { CLASSIC_THEME, DEFAULT_THEME } from "./theme/theme";

const ABOUT: ArtifactAbout = {
  name: "Backlog state",
  pinned_by: "Ada",
  missing: null,
  fresh: null,
};

type Call = { cmd: string; args: Record<string, unknown> };

/** The core, answering an artifact's commands, with what each was asked. `about` is read at
 *  each ask, so a test may change what the core says. */
function core(about: ArtifactAbout | (() => ArtifactAbout) = ABOUT) {
  const calls: Call[] = [];
  mockIPC(
    (cmd, args) => {
      if (!cmd.startsWith("artifact_")) return null;
      calls.push({ cmd, args: args as Record<string, unknown> });
      if (cmd === "artifact_about") return typeof about === "function" ? about() : about;
      if (cmd === "artifact_open") return "artifact-7";
      return null;
    },
    { shouldMockEvents: true },
  );
  return calls;
}

const asked = (calls: Call[], cmd: string) => calls.filter((one) => one.cmd === cmd);

function Tab({ waits = false, onAsk = () => {} }: { waits?: boolean; onAsk?: () => void }) {
  return (
    <ArtifactTab
      plane="p"
      workspace="alpha"
      folder="backlog-state"
      title="Backlog state"
      waits={waits}
      onAsk={onAsk}
    />
  );
}

/** The plane's watch says `paths` of workspace `workspace` changed, as artifacts. */
async function changed(workspace: string, ...paths: string[]) {
  const told: PlaneChanged = {
    plane: "p",
    changes: paths.map((path) => ({
      kind: "artifacts",
      workspace,
      persona: null,
      path: `workspaces/${workspace}/artifacts/${path}`,
    })),
    answers: [{ answer: "git" }, { answer: "views" }],
  };
  await act(async () => {
    await emit("plane-changed", told);
  });
}

afterEach(() => {
  document.documentElement.removeAttribute("style");
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

describe("an artifact's tab", () => {
  it("names who pinned it on purlis's bar, outside the page", async () => {
    core();
    render(<Tab />);
    const bar = await screen.findByTestId("artifact-bar");
    await waitFor(() => expect(bar).toHaveTextContent("Backlog state"));
    expect(bar).toHaveTextContent("Pinned by Ada");
  });

  it("says a pin no commit holds yet is not saved", async () => {
    core({ ...ABOUT, pinned_by: null });
    render(<Tab />);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-bar")).toHaveTextContent(
        "Pinned here, not saved to the project yet",
      ),
    );
  });

  it("opens its page in a frame titled with the artifact's name, and closes it as it goes", async () => {
    const calls = core();
    const { unmount } = render(<Tab />);
    const frame = await screen.findByTestId("artifact-frame");
    expect(frame).toHaveAttribute("role", "region");
    expect(frame).toHaveAccessibleName("Backlog state");
    expect(frame).toHaveAttribute("tabindex", "0");
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    expect(asked(calls, "artifact_open")[0].args).toMatchObject({
      plane: "p",
      workspace: "alpha",
      folder: "backlog-state",
    });
    unmount();
    await vi.waitFor(() =>
      expect(asked(calls, "artifact_close").map((one) => one.args.label)).toEqual(["artifact-7"]),
    );
  });

  it("runs nothing when it was put back by a launch, until Open is pressed", async () => {
    const calls = core();
    const onAsk = vi.fn();
    const { rerender } = render(<Tab waits onAsk={onAsk} />);
    const open = await screen.findByRole("button", { name: "Open Backlog state" });
    // The bar still names it: purlis's, not the page's.
    expect(screen.getByTestId("artifact-bar")).toHaveTextContent("Pinned by Ada");
    expect(screen.queryByTestId("artifact-frame")).toBeNull();
    expect(asked(calls, "artifact_open")).toHaveLength(0);

    await userEvent.click(open);
    expect(onAsk).toHaveBeenCalledTimes(1);
    rerender(<Tab waits={false} onAsk={onAsk} />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
  });

  it("puts the keyboard in the page on Enter in its frame", async () => {
    const calls = core();
    render(<Tab />);
    const frame = await screen.findByTestId("artifact-frame");
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    frame.focus();
    await userEvent.keyboard("{Enter}");
    await vi.waitFor(() =>
      expect(asked(calls, "artifact_enter").map((one) => one.args.label)).toEqual(["artifact-7"]),
    );
  });

  it("takes the keyboard back to its frame on Esc pressed in the page, and goes on on F6", async () => {
    const calls = core();
    render(
      <>
        <button type="button">Elsewhere</button>
        <Tab />
      </>,
    );
    const frame = await screen.findByTestId("artifact-frame");
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    screen.getByRole("button", { name: "Elsewhere" }).focus();

    await emit(ARTIFACT_KEY, { label: "artifact-7", key: "escape" });
    await waitFor(() => expect(frame).toHaveFocus());

    const f6 = vi.fn((event: KeyboardEvent) => event.key);
    window.addEventListener("keydown", f6);
    await emit(ARTIFACT_KEY, { label: "artifact-7", key: "f6-back" });
    await vi.waitFor(() => expect(f6).toHaveBeenCalled());
    const pressed = f6.mock.calls[0][0];
    expect(pressed.key).toBe("F6");
    expect(pressed.shiftKey).toBe(true);
    window.removeEventListener("keydown", f6);
  });

  it("ignores a key from another artifact's page", async () => {
    const calls = core();
    render(
      <>
        <button type="button">Elsewhere</button>
        <Tab />
      </>,
    );
    const frame = await screen.findByTestId("artifact-frame");
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    const elsewhere = screen.getByRole("button", { name: "Elsewhere" });
    elsewhere.focus();
    await emit(ARTIFACT_KEY, { label: "artifact-8", key: "escape" });
    await new Promise((settle) => setTimeout(settle, 20));
    expect(frame).not.toHaveFocus();
    expect(elsewhere).toHaveFocus();
  });

  it("loads its page again when its own folder changes, and not for another's", async () => {
    const calls = core();
    render(<Tab />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));

    await changed("alpha", "backlog-state/data.json");
    await vi.waitFor(() =>
      expect(asked(calls, "artifact_reload").map((one) => one.args.label)).toEqual(["artifact-7"]),
    );

    await changed("alpha", "release/data.json");
    await changed("beta", "backlog-state/data.json");
    await new Promise((settle) => setTimeout(settle, 20));
    expect(asked(calls, "artifact_reload")).toHaveLength(1);
  });

  it("hands its page the theme the window draws, and each one it draws after", async () => {
    const root = document.documentElement;
    const [one, other] = [
      DEFAULT_THEME.values["surface.base"],
      CLASSIC_THEME.values["surface.base"],
    ];
    expect(one).not.toBe(other);
    root.style.setProperty("--surface-base", one);
    root.style.colorScheme = "light";
    const calls = core();
    render(<Tab />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    expect(asked(calls, "artifact_open")[0].args.theme).toEqual({
      appearance: "light",
      values: { "surface.base": one },
    });

    root.style.setProperty("--surface-base", other);
    root.style.colorScheme = "dark";
    await vi.waitFor(() =>
      expect(asked(calls, "artifact_theme").at(-1)?.args).toEqual({
        label: "artifact-7",
        theme: { appearance: "dark", values: { "surface.base": other } },
      }),
    );
  });

  it("says on purlis's bar when its data was last updated", async () => {
    const at = Date.now() - 3 * 60 * 60 * 1000;
    core({ ...ABOUT, fresh: { at, stale_at: at + 86_400_000, source: "declared" } });
    render(<Tab />);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-bar")).toHaveTextContent("Updated 3 hours ago"),
    );
  });

  it("says it cannot be opened once its pin goes, and closes its page", async () => {
    let about = ABOUT;
    const calls = core(() => about);
    render(<Tab />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));

    about = { ...ABOUT, missing: "'backlog-state' is not pinned to this workspace any more" };
    await changed("alpha", "pins.toml");

    expect(await screen.findByTestId("artifact-missing")).toHaveTextContent("not pinned");
    await vi.waitFor(() =>
      expect(asked(calls, "artifact_close").map((one) => one.args.label)).toEqual(["artifact-7"]),
    );
  });

  it("says why it cannot be opened, and opens nothing", async () => {
    const calls = core({ ...ABOUT, missing: "there is no folder 'backlog-state' in artifacts/" });
    render(<Tab />);
    const said = await screen.findByTestId("artifact-missing");
    expect(said).toHaveTextContent("there is no folder 'backlog-state' in artifacts/");
    expect(asked(calls, "artifact_open")).toHaveLength(0);
  });
});

describe("an artifact's view", () => {
  it("names its workspace and folder, so two workspaces' folders of one name are two tabs", () => {
    expect(artifactView("alpha", "state")).toEqual({
      from: null,
      view: "artifact",
      key: "alpha/state",
    });
    expect(artifactOf(artifactView("alpha", "state"))).toEqual({
      workspace: "alpha",
      folder: "state",
    });
    expect(artifactOf({ from: null, view: "artifact", key: "nofolder" })).toBeUndefined();
    expect(artifactOf({ from: "ext", view: "artifact", key: "a/b" })).toBeUndefined();
    expect(artifactOf({ from: null, view: "vault", key: "a/b" })).toBeUndefined();
  });

  it("is put away while a dialog or a menu is over its frame, and not for a tooltip", () => {
    const at = (left: number, top: number, width: number, height: number) =>
      ({
        left,
        top,
        right: left + width,
        bottom: top + height,
        width,
        height,
      }) as DOMRect;
    const frame = at(100, 100, 400, 300);
    const root = document.createElement("div");
    const over = (role: string, rect: DOMRect) => {
      const one = document.createElement("div");
      one.setAttribute("role", role);
      one.getBoundingClientRect = () => rect;
      root.append(one);
      return one;
    };
    over("tooltip", at(150, 150, 50, 20));
    expect(coveredByAnOverlay(frame, root)).toBe(false);
    over("dialog", at(0, 0, 50, 50));
    expect(coveredByAnOverlay(frame, root)).toBe(false);
    over("menu", at(480, 380, 100, 100));
    expect(coveredByAnOverlay(frame, root)).toBe(true);
  });
});
