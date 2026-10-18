// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";

/**
 * **Opening a folder that is not a project offers to make it one** (#1822, spec #1810 story
 * 28), in the window as built, against a mocked core.
 *
 * What the core does with the folder — taken into this machine's project, nothing written into
 * it — is `purlis_core::firstrun`'s and is tested there. Here: the offer comes in place of a
 * refusal, Not now asks the core for nothing and comes back to the Open door, Make it a project
 * is Create a project's act with the template picked, and it ends at the trust question.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const OPEN_DOOR = "Open a folder on this machine…";
const FOLDER = "/home/dev/notes";
const LOCAL = "/home/dev/.local/share/purlis/local-project";

const FOUND = {
  harnesses: [],
  forges: [],
  templates: [
    { id: "go", title: "Go", summary: "A Go module." },
    { id: "rust", title: "Rust", summary: "A Cargo crate or workspace." },
  ],
  local_models: [],
};

const ASK = {
  path: LOCAL,
  contributes: { plugins: [], env: [], starts: [], profiles: [], grants: [], fonts: [] },
  changes: [],
  first: true,
};

/** A core in which `FOLDER` is in no project, unless `inAProject`; `fresh` is a machine that has
 *  not been through the guided steps (#1817), so the welcome is drawn inside them. */
function core(inAProject = false, fresh = false) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    switch (cmd) {
      case "plane_at_launch":
        return { plane: null, from: null, why: null };
      case "recent_planes":
        return { planes: [], dropped: [], forgetful: null };
      case "pick_project":
        return FOLDER;
      // Git is there (#1813), and this machine has been through the guided steps once (#1817).
      case "git_presence":
        return { kind: "present" };
      case "onboarding_progress":
        return {
          step: "welcome",
          door: null,
          signing_in: false,
          git_missing: false,
          made: null,
          finished_once: !fresh,
        };
      case "folder_is_a_project":
        return inAProject;
      case "first_run_found":
        return FOUND;
      case "template_that_fits":
        return "rust";
      case "open_plane":
        return { plane: FOLDER, ask: null };
      case "open_repo":
        return {
          opened: {
            opened: { plane: null, ask: ASK },
            workspace: "notes",
            cwd: `${LOCAL}/workspaces/notes/notes`,
            harness: null,
            none_installed: false,
            instructions: 0,
            forge_said: null,
            template: "go",
          },
          asks_forge: null,
        };
      case "opened_chats":
      case "chat_states":
      case "chats_that_would_not_start":
      case "running_sessions":
        return [];
      default:
        return null;
    }
  });
  return { calls: (cmd: string) => asked.filter((one) => one.cmd === cmd) };
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a folder in no project (#1822)", () => {
  it("is offered to be made one, with the template choice, in place of a refusal", async () => {
    const { calls } = core();
    render(<App />);

    await userEvent.setup().click(await screen.findByRole("button", { name: OPEN_DOOR }));

    const heading = await screen.findByRole("heading", { level: 1, name: "Make it a project?" });
    expect(document.activeElement).toBe(heading);
    expect(
      screen.getByText(/This folder isn't a purlis project yet\. Make it one\?/),
    ).toBeVisible();
    expect(screen.getByText(FOLDER)).toBeVisible();
    expect(await screen.findByRole("radio", { name: /Fits the folder/ })).toBeChecked();
    expect(screen.getByRole("radio", { name: /Go/ })).toBeInTheDocument();
    expect(await screen.findByText("Rust, by the files at the folder's top level.")).toBeVisible();
    // Nothing was opened, and nothing was made.
    expect(calls("open_plane")).toEqual([]);
    expect(calls("open_repo")).toEqual([]);
  });

  it("leaves the folder untouched on Not now, and gives the keyboard back to the Open door", async () => {
    const { calls } = core();
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: OPEN_DOOR }));
    await person.click(await screen.findByRole("button", { name: "Not now" }));

    await screen.findByRole("heading", { level: 1, name: "Welcome to purlis" });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: OPEN_DOOR }));
    expect(calls("open_plane")).toEqual([]);
    expect(calls("open_repo")).toEqual([]);
  });

  it("makes the project from the template picked, and asks the trust question", async () => {
    const { calls } = core();
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: OPEN_DOOR }));
    await person.click(await screen.findByRole("radio", { name: /Go/ }));
    await person.click(screen.getByRole("button", { name: "Make it a project" }));

    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("Open this project?");
    expect(calls("open_repo").map((one) => one.args)).toEqual([
      { path: FOLDER, template: { kind: "named", id: "go" }, forge: null },
    ]);
    expect(calls("open_plane")).toEqual([]);
  });

  it("is offered for a typed path as well", async () => {
    core();
    render(<App />);
    const person = userEvent.setup();

    await person.type(await screen.findByLabelText("Or type a path"), FOLDER);
    await person.click(screen.getByRole("button", { name: "Open project" }));

    expect(
      await screen.findByRole("heading", { level: 1, name: "Make it a project?" }),
    ).toBeInTheDocument();
  });

  it("is not offered for a folder in a project, which opens as before", async () => {
    const { calls } = core(true);
    render(<App />);

    await userEvent.setup().click(await screen.findByRole("button", { name: OPEN_DOOR }));

    await vi.waitFor(() =>
      expect(calls("open_plane").map((one) => one.args)).toEqual([{ path: FOLDER }]),
    );
    expect(screen.queryByRole("heading", { name: "Make it a project?" })).not.toBeInTheDocument();
  });

  it("goes back to the welcome step of the guided steps on Not now", async () => {
    const { calls } = core(false, true);
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: OPEN_DOOR }));
    await person.click(await screen.findByRole("button", { name: "Not now" }));

    expect(await screen.findByRole("button", { name: OPEN_DOOR })).toBeInTheDocument();
    await vi.waitFor(() =>
      expect(
        calls("keep_onboarding_progress").at(-1)?.args.progress as { step?: string } | undefined,
      ).toMatchObject({ step: "welcome" }),
    );
    expect(calls("open_repo")).toEqual([]);
  });
});
