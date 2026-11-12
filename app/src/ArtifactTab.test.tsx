// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactAbout, ArtifactOpened, PlaneChanged } from "./bindings";
import { ArtifactTab } from "./ArtifactTab";
import {
  APPROVALS_CHANGED,
  ARTIFACT_KEY,
  artifactOf,
  artifactView,
  coveredByAnOverlay,
  templateOf,
} from "./artifacts";
import { CLASSIC_THEME, DEFAULT_THEME } from "./theme/theme";

const ABOUT: ArtifactAbout = {
  name: "Backlog state",
  pinned_by: "Ada",
  missing: null,
  too_large: null,
  fresh: null,
};

const OPENED: ArtifactOpened = {
  label: "artifact-7",
  standing: "script-free",
  changed_by: null,
  generated: true,
  queries: [],
  plane_granted: false,
};

type Call = { cmd: string; args: Record<string, unknown> };

/** The core, answering an artifact's commands, with what each was asked. `about` is read at
 *  each ask, so a test may change what the core says; `after` is what it says once Repin or
 *  Unpin was pressed. */
function core(
  about: ArtifactAbout | (() => ArtifactAbout) = ABOUT,
  opened: Partial<ArtifactOpened> = {},
  after: (cmd: string) => ArtifactAbout | undefined = () => undefined,
) {
  const calls: Call[] = [];
  let now = about;
  mockIPC(
    (cmd, args) => {
      if (!cmd.startsWith("artifact_")) return null;
      calls.push({ cmd, args: args as Record<string, unknown> });
      if (cmd === "artifact_repin" || cmd === "artifact_unpin") {
        now = after(cmd) ?? now;
        return null;
      }
      if (cmd === "artifact_about") return typeof now === "function" ? now() : now;
      if (cmd === "artifact_open" || cmd === "artifact_reload") return { ...OPENED, ...opened };
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

/** What the core answers an artifact's tab that reads and opens it, and nothing more. */
function answering(cmd: string): unknown {
  if (cmd === "artifact_about") return ABOUT;
  if (cmd === "artifact_open") return OPENED;
  return null;
}

/** The rejections nobody handled while `act` ran, and as its last promises settled. */
async function unhandledDuring(act: () => Promise<void>): Promise<unknown[]> {
  const unhandled: unknown[] = [];
  const heard = (reason: unknown) => unhandled.push(reason);
  process.on("unhandledRejection", heard);
  try {
    await act();
    await new Promise((settled) => setTimeout(settled, 0));
  } finally {
    process.off("unhandledRejection", heard);
  }
  return unhandled;
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
    // The bar still names it: purlis's, not the page's (once the core has said who pinned it).
    await waitFor(() =>
      expect(screen.getByTestId("artifact-bar")).toHaveTextContent("Pinned by Ada"),
    );
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

  it("loads its page again in place when its own data changes, and not for another's", async () => {
    const calls = core();
    render(<Tab />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));

    await changed("alpha", "backlog-state/data.json");
    await vi.waitFor(() =>
      expect(asked(calls, "artifact_reload").map((one) => one.args.label)).toEqual(["artifact-7"]),
    );
    await changed("alpha", "backlog-state/data/weeks.json");
    await vi.waitFor(() => expect(asked(calls, "artifact_reload")).toHaveLength(2));

    await changed("alpha", "release/data.json");
    await changed("beta", "backlog-state/data.json");
    await new Promise((settle) => setTimeout(settle, 20));
    expect(asked(calls, "artifact_reload")).toHaveLength(2);
    // In place: the same webview, never closed and opened again for its data.
    expect(asked(calls, "artifact_open")).toHaveLength(1);
    expect(asked(calls, "artifact_close")).toHaveLength(0);
  });

  it("says on its bar where it stands as a reload read it", async () => {
    const standing: Partial<ArtifactOpened> = {};
    const calls = core(ABOUT, standing);
    render(<Tab />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    const edge = screen.getByTestId("artifact-edge");
    await waitFor(() => expect(edge).toHaveAttribute("data-standing", "plain"));

    // Its template took script with its data, in one burst: the core reads it as new here.
    standing.standing = "new";
    await changed("alpha", "backlog-state/data.json");
    await waitFor(() => expect(edge).toHaveAttribute("data-standing", "asks"));
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

  it("stops listening without an unhandled error when its listens answer after it went and the window's event bridge went too (#6194)", async () => {
    const unhandled = await unhandledDuring(async () => {
      const answers: ((id: number) => void)[] = [];
      mockIPC((cmd) => {
        if (cmd === "plugin:event|listen") return new Promise<number>((r) => answers.push(r));
        return answering(cmd);
      });
      const { unmount } = render(<Tab />);
      await waitFor(() => expect(answers.length).toBeGreaterThanOrEqual(2));
      unmount();
      clearMocks();
      answers.forEach((answer, id) => answer(id));
    });
    expect(unhandled).toEqual([]);
  });

  it("stops listening without an unhandled error when the window's event bridge went before it did (#6194)", async () => {
    const unhandled = await unhandledDuring(async () => {
      let listened = 0;
      mockIPC((cmd) => {
        if (cmd === "plugin:event|listen") return ++listened;
        return answering(cmd);
      });
      const { unmount } = render(<Tab />);
      await waitFor(() => expect(listened).toBeGreaterThanOrEqual(2));
      await act(async () => {});
      clearMocks();
      unmount();
    });
    expect(unhandled).toEqual([]);
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
    expect(said).toHaveTextContent("There is no folder 'backlog-state' in artifacts/");
    expect(asked(calls, "artifact_open")).toHaveLength(0);
  });

  it("names the missing page's path and repins it once it is back", async () => {
    const calls = core(
      { ...ABOUT, missing: "there is no folder 'backlog-state' in artifacts/" },
      {},
      () => ABOUT,
    );
    render(<Tab />);
    const said = await screen.findByTestId("artifact-missing");
    expect(said).toHaveTextContent("Backlog state is missing");
    expect(said).toHaveTextContent("artifacts/backlog-state/index.html");
    expect(screen.getByRole("button", { name: "Unpin" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Repin" }));

    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    expect(asked(calls, "artifact_repin")[0]?.args).toMatchObject({
      workspace: "alpha",
      folder: "backlog-state",
    });
    expect(screen.queryByTestId("artifact-missing")).toBeNull();
  });

  it("says a repin that is refused, and stays as it was", async () => {
    core({ ...ABOUT, missing: "there is no folder 'backlog-state' in artifacts/" });
    mockIPC(
      (cmd) => {
        if (cmd === "artifact_repin") throw "there is no folder 'backlog-state' in artifacts/";
        if (cmd === "artifact_about")
          return { ...ABOUT, missing: "there is no folder 'backlog-state' in artifacts/" };
        return null;
      },
      { shouldMockEvents: true },
    );
    render(<Tab />);
    await userEvent.click(await screen.findByRole("button", { name: "Repin" }));
    const refused = await waitFor(() => {
      const found = document.querySelector('[data-cause="artifact-repin-refused:backlog-state"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(refused).toHaveTextContent(
      "purlis could not repin Backlog state: there is no folder 'backlog-state' in artifacts/. Put its folder back and repin it again, or unpin it.",
    );
    expect(screen.getByTestId("artifact-missing")).toBeInTheDocument();

    await userEvent.click(within(refused).getByRole("button", { name: "Dismiss" }));
    await waitFor(() =>
      expect(document.querySelector('[data-cause^="artifact-repin-refused"]')).toBeNull(),
    );
  });

  it("says a page over the size cap with both sizes, opens nothing, and unpins it", async () => {
    const over =
      "'backlog-state' is 9.0 MB, over the 8.0 MB an artifact may be, so none of it is shown";
    const calls = core({ ...ABOUT, too_large: over });
    render(<Tab />);
    const said = await screen.findByTestId("artifact-too-large");
    expect(said).toHaveTextContent("Backlog state is too large to open");
    expect(said).toHaveTextContent("9.0 MB, over the 8.0 MB");
    expect(screen.getByRole("button", { name: "Repin" })).toBeInTheDocument();
    expect(asked(calls, "artifact_open")).toHaveLength(0);

    await userEvent.click(screen.getByRole("button", { name: "Unpin" }));

    await screen.findByText("Backlog state is unpinned");
    expect(asked(calls, "artifact_unpin")[0]?.args).toMatchObject({
      workspace: "alpha",
      folder: "backlog-state",
    });
    expect(screen.queryByRole("button", { name: "Repin" })).toBeNull();
  });
});

describe("an artifact with script (#5959)", () => {
  it("is shown without its script and asks in purlis's bar, framed as asking", async () => {
    core(ABOUT, { standing: "new" });
    render(<Tab />);
    const bar = await screen.findByTestId("artifact-bar");
    await waitFor(() =>
      expect(bar).toHaveTextContent("Has script, shown without it until you approve it"),
    );
    expect(screen.getByRole("button", { name: "Approve script" })).toBeInTheDocument();
    expect(screen.getByTestId("artifact-edge")).toHaveAttribute("data-standing", "asks");
  });

  it("asks nothing of a page with no script, or one already approved here", async () => {
    core(ABOUT, { standing: "script-free" });
    const { unmount } = render(<Tab />);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-edge")).toHaveAttribute("data-standing", "plain"),
    );
    expect(screen.queryByRole("button", { name: "Approve script" })).toBeNull();
    unmount();
    clearMocks();

    core(ABOUT, { standing: "approved" });
    render(<Tab />);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-bar")).toHaveTextContent(
        "Script approved on this machine",
      ),
    );
    expect(screen.getByTestId("artifact-edge")).toHaveAttribute("data-standing", "runs");
    expect(screen.queryByRole("button", { name: "Approve script" })).toBeNull();
  });

  it("approves only from its bar, the page its own webview was served", async () => {
    const calls = core(ABOUT, { standing: "changed" });
    render(<Tab />);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-bar")).toHaveTextContent(
        "Its template changed since you approved it",
      ),
    );
    await userEvent.click(screen.getByRole("button", { name: "Approve script" }));
    await vi.waitFor(() =>
      expect(asked(calls, "artifact_approve").map((one) => one.args)).toEqual([
        { label: "artifact-7" },
      ]),
    );
  });

  it("names who changed its pin", async () => {
    core(ABOUT, { standing: "pin-changed", changed_by: "Grace" });
    render(<Tab />);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-bar")).toHaveTextContent(
        "Grace changed its pin since you approved it",
      ),
    );
  });

  it("says a scripted page renders without it on a platform where none runs", async () => {
    core(ABOUT, { standing: "not-here" });
    render(<Tab />);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-bar")).toHaveTextContent(
        "Shown without its script: scripted artifacts do not run on this system yet",
      ),
    );
    expect(screen.queryByRole("button", { name: "Approve script" })).toBeNull();
  });

  it("says on its bar when the data its page reads was not generated on this machine", async () => {
    core(ABOUT, { standing: "approved", generated: false });
    render(<Tab />);
    await waitFor(() =>
      expect(screen.getByTestId("artifact-data")).toHaveTextContent(
        "Not generated on this machine",
      ),
    );
    expect(screen.getByTestId("artifact-bar")).toContainElement(
      screen.getByTestId("artifact-data"),
    );
  });

  it("says nothing of its data once it was generated here", async () => {
    const calls = core(ABOUT, { standing: "approved", generated: true });
    render(<Tab />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    await waitFor(() =>
      expect(screen.getByTestId("artifact-standing")).toHaveTextContent("Script approved"),
    );
    expect(screen.queryByTestId("artifact-data")).toBeNull();
  });

  it("loads its page again when its approval changes in any window, and not for another's", async () => {
    const calls = core(ABOUT, { standing: "new" });
    render(<Tab />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));

    await emit(APPROVALS_CHANGED, { workspace: "alpha", folder: "other" });
    await emit(APPROVALS_CHANGED, { workspace: "beta", folder: "backlog-state" });
    await new Promise((settle) => setTimeout(settle, 20));
    expect(asked(calls, "artifact_open")).toHaveLength(1);

    await emit(APPROVALS_CHANGED, { workspace: "alpha", folder: "backlog-state" });
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(2));
    expect(asked(calls, "artifact_close").map((one) => one.args.label)).toEqual(["artifact-7"]);
  });

  it("tears its page down and opens it again when its template changes, not its data", async () => {
    const calls = core(ABOUT, { standing: "approved" });
    render(<Tab />);
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(1));
    const changed = (path: string) =>
      emit("plane-changed", {
        plane: "p",
        answers: [{ answer: "views" }],
        changes: [{ kind: "workspace", workspace: "alpha", persona: null, path }],
      });

    await changed("workspaces/alpha/artifacts/backlog-state/data.json");
    await changed("workspaces/alpha/artifacts/backlog-state/data/2026-11-09.json");
    await changed("workspaces/alpha/artifacts/other/index.html");
    await new Promise((settle) => setTimeout(settle, 20));
    expect(asked(calls, "artifact_open")).toHaveLength(1);

    await changed("workspaces/alpha/artifacts/backlog-state/app.js");
    await waitFor(() => expect(asked(calls, "artifact_open")).toHaveLength(2));
    expect(asked(calls, "artifact_close").map((one) => one.args.label)).toEqual(["artifact-7"]);
  });
});

describe("an artifact's view", () => {
  it("names a change to its template, never its data or another folder", () => {
    const about = templateOf("alpha", "state");
    const at = (path: string, workspace = "alpha") =>
      about({ kind: "workspace", workspace, persona: null, path });
    expect(at("workspaces/alpha/artifacts/state/index.html")).toBe(true);
    expect(at("workspaces/alpha/artifacts/state/js/app.js")).toBe(true);
    expect(at("workspaces/alpha/artifacts/state/data.json")).toBe(false);
    expect(at("workspaces/alpha/artifacts/state/data/a.json")).toBe(false);
    expect(at("workspaces/alpha/artifacts/statement/index.html")).toBe(false);
    expect(at("workspaces/alpha/artifacts/pins.toml")).toBe(false);
    expect(at("workspaces/beta/artifacts/state/index.html", "beta")).toBe(false);
  });

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
