// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { closeFromTheRail } from "./test-strips";
import type { PlaneContribution } from "./bindings";

/** The project the status line names, found by its directory: the line draws the project's
 *  name, and its directory is the item's tooltip (qw357). */
const projectAt = (directory: string) =>
  vi.waitFor(() => {
    const item = document.querySelector<HTMLElement>(
      `[data-testid="status-project"][data-directory="${directory}"]`,
    );
    if (item === null) throw new Error(`the status line names no project at ${directory}`);
    return item;
  });

/**
 * The welcome, and the question in front of it.
 *
 * Two things are being pinned here and they are different in kind. The first is that a window
 * with no project is **usable** — it offers its three doors whatever the machine remembers
 * (#1812), and says which of the no-project states it is in. The second is the gate: a project
 * the operator has not approved is described and not opened, the approval carries back what
 * was on screen, and cancelling leaves the window exactly as it was.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

/** The Open door, by the name a screen reader reads for it. */
const OPEN_DOOR = "Open a folder on this machine…";

afterEach(() => {
  cleanup();
  clearMocks();
});

const SIDEBAR = {
  root: "/home/dev/plane",
  workspaces: [],
  personas: [],
  persona: null,
  unfiled: [],
};

/** What a project with something to declare contributes. */
const CONTRIBUTES: PlaneContribution = {
  plugins: [["superpowers@market", "true"]],
  env: [["ANTHROPIC_BASE_URL", "https://example.invalid"]],
  starts: [['{"program":"/bin/sh","args":[],"cwd":"/home/dev/plane"}', ""]],
  profiles: [],
  grants: [["ops", '{"kubectl":""}']],
  fonts: [],
};

/**
 * A core whose answers a test decides, with every call kept.
 *
 * `answers` is consulted first; anything it does not answer falls through to the shape the
 * window needs to come up at all.
 */
function core(answers: (cmd: string, args: Record<string, unknown>) => unknown) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    const answer = answers(cmd, given);
    if (answer !== undefined) return answer;
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    // A machine that remembers a project: the welcome is the same either way (#1812).
    if (cmd === "recent_planes")
      return {
        planes: [{ path: "/home/dev/elsewhere", name: "elsewhere", approved: true }],
        dropped: [],
        forgetful: null,
      };
    if (cmd === "plane_sidebar") return SIDEBAR;
    // Git is there (#1813), and this machine has been through the guided steps once (#1817),
    // so a door's act runs at once and its project comes to the front.
    if (cmd === "git_presence") return { kind: "present" };
    if (cmd === "onboarding_progress")
      return {
        step: "welcome",
        door: null,
        signing_in: false,
        git_missing: false,
        made: null,
        finished_once: true,
      };
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    return null;
  });
  return { asked };
}

/** Types a path into the welcome and presses Open. */
async function openByPath(path: string) {
  const person = userEvent.setup();
  await person.type(await screen.findByLabelText("Or type a path"), path);
  await person.click(screen.getByRole("button", { name: "Open project" }));
  return person;
}

describe("the welcome's way in by path and recents", () => {
  it("opens a project the operator names, and the window then shows it", async () => {
    const { asked } = core((cmd) => {
      if (cmd === "open_plane") return { plane: "/home/dev/plane", ask: null };
      return undefined;
    });

    render(<App />);
    await openByPath("/home/dev/plane");

    expect(await projectAt("/home/dev/plane")).toBeInTheDocument();
    expect(asked.find((one) => one.cmd === "open_plane")?.args).toEqual({
      path: "/home/dev/plane",
    });
  });

  it("says what a project contributes instead of opening it, and opens nothing until asked", async () => {
    // ADR 0035. `.charter/app/reopen.json` is an execution input and a project is a
    // DIRECTORY, so "open this folder" must not be able to mean "run what is written in it".
    const { asked } = core((cmd) => {
      if (cmd === "open_plane")
        return {
          plane: null,
          ask: {
            path: "/home/dev/stranger",
            contributes: CONTRIBUTES,
            changes: [],
            first: true,
          },
        };
      return undefined;
    });

    render(<App />);
    await openByPath("/home/dev/stranger");

    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("Open this project?");
    expect(dialog).toHaveTextContent("superpowers@market");
    expect(dialog).toHaveTextContent("ANTHROPIC_BASE_URL=https://example.invalid");
    expect(dialog).toHaveTextContent("/bin/sh");
    expect(dialog).toHaveTextContent("Tools its personas may run without a prompt");
    expect(dialog).toHaveTextContent('ops {"kubectl":""}');
    // The path charter RESOLVED, because a picker pointed at a subfolder opens the project
    // above it and approving a directory you did not choose is the failure this prevents.
    expect(dialog).toHaveTextContent("/home/dev/stranger");
    expect(asked.some((one) => one.cmd === "approve_plane")).toBe(false);
  });

  it("carries the contribution that was shown back with the approval", async () => {
    // The approval is an answer to the question that was ASKED. Between the dialog reading
    // the project and the button being pressed, anything on the machine can rewrite its
    // settings or its record; the core checks this value against the disk again, and it can
    // only do that if the window hands back what it drew.
    const { asked } = core((cmd) => {
      if (cmd === "open_plane")
        return {
          plane: null,
          ask: { path: "/home/dev/stranger", contributes: CONTRIBUTES, changes: [], first: true },
        };
      if (cmd === "approve_plane") return "/home/dev/stranger";
      return undefined;
    });

    render(<App />);
    await openByPath("/home/dev/stranger");
    await userEvent.setup().click(await screen.findByRole("button", { name: "Open project" }));

    expect(asked.find((one) => one.cmd === "approve_plane")?.args).toEqual({
      path: "/home/dev/stranger",
      contributes: CONTRIBUTES,
    });
    expect(await projectAt("/home/dev/stranger")).toBeInTheDocument();
  });

  it("opens nothing when the operator cancels, and leaves the welcome where it was", async () => {
    const { asked } = core((cmd) => {
      if (cmd === "open_plane")
        return {
          plane: null,
          ask: { path: "/home/dev/stranger", contributes: CONTRIBUTES, changes: [], first: true },
        };
      return undefined;
    });

    render(<App />);
    await openByPath("/home/dev/stranger");
    await userEvent.setup().click(await screen.findByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(asked.some((one) => one.cmd === "approve_plane")).toBe(false);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Welcome to purlis");
  });

  it("draws the path box as a setting row, and its acts in the set's rows (#1719)", async () => {
    core(() => undefined);
    render(<App />);

    const box = await screen.findByLabelText("Or type a path");
    expect(box.closest(".ui-setting-row")).not.toBeNull();
    expect(box.closest("[data-setting]")?.getAttribute("data-setting")).toBe("open-by-path");
    expect(
      screen.getByRole("button", { name: "Open project" }).closest(".ui-setting-actions"),
    ).not.toBeNull();
    expect(document.querySelector(".doing")).toBeNull();
  });

  it("says the recent projects could not be read, and still opens by path (#1719)", async () => {
    core((cmd) => {
      if (cmd === "recent_planes") throw "the store is locked";
      return undefined;
    });
    render(<App />);

    expect(
      await screen.findByText("purlis could not read the recent projects: the store is locked"),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Or type a path")).toBeInTheDocument();
  });

  it("names the program purlis where it says what a project is (#630)", async () => {
    core(() => undefined);
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: "Welcome to purlis" });
    expect(
      screen.getByText(/A project is where purlis keeps your workspaces, personas and memory/),
    ).toBeInTheDocument();
  });

  it("reads differently when a project it already approved has started doing more", async () => {
    // An approval is consent to a CONTRIBUTION, not to a path, so a project that gains a
    // plugin after it was approved has been handed a grant nobody looked at. The words for
    // what changed are charter's own; the window does not describe them a second time.
    core((cmd) => {
      if (cmd === "open_plane")
        return {
          plane: null,
          ask: {
            path: "/home/dev/plane",
            contributes: CONTRIBUTES,
            changes: ["the plugin superpowers@market is new"],
            first: false,
          },
        };
      return undefined;
    });

    render(<App />);
    await openByPath("/home/dev/plane");

    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("This project has changed since you approved it");
    expect(dialog).toHaveTextContent("the plugin superpowers@market is new");
    expect(screen.getByRole("button", { name: "Open it anyway" })).toBeInTheDocument();
  });

  it("offers the projects this machine remembers, and opens one on a click", async () => {
    const { asked } = core((cmd) => {
      if (cmd === "recent_planes")
        return {
          planes: [
            { path: "/home/dev/one", name: "one", approved: true },
            { path: "/home/dev/two", name: "two", approved: false },
          ],
          dropped: [],
          forgetful: null,
        };
      if (cmd === "open_plane") return { plane: "/home/dev/one", ask: null };
      return undefined;
    });

    render(<App />);
    await userEvent.setup().click(await screen.findByRole("row", { name: /one/ }));

    expect(asked.find((one) => one.cmd === "open_plane")?.args).toEqual({ path: "/home/dev/one" });
    // The row for a project nobody has approved says so, and the approved one does not.
    expect(screen.queryAllByText("purlis will ask about this one")).toHaveLength(0);
  });

  it("drops a project that has moved with a line saying so, and never an error", async () => {
    // ADR 0034: the record is a convenience and the project is the truth. A welcome that
    // shows one fewer row and says why is usable; a dialog at launch is not.
    core((cmd) => {
      if (cmd === "recent_planes")
        return {
          planes: [],
          dropped: [],
          gone: [{ path: "/home/dev/gone", said: "/home/dev/gone is no longer there" }],
          forgetful: null,
        };
      return undefined;
    });

    render(<App />);

    expect(await screen.findByText("/home/dev/gone is no longer there")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("is a working screen on a machine that keeps no store at all", async () => {
    // Windows: `0600` has no expression there, so charter's guard refuses rather than
    // degrades (ADR 0031) and there is no record of anything. The app still opens projects.
    core((cmd) => {
      if (cmd === "recent_planes")
        return {
          planes: [],
          dropped: [],
          forgetful: "purlis keeps no machine store on this platform",
        };
      return undefined;
    });

    render(<App />);

    expect(await screen.findByText(/cannot remember projects on this machine/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: OPEN_DOOR })).toBeInTheDocument();
  });

  it("says why an open did not happen, in the core's own words, and stays open", async () => {
    core((cmd) => {
      if (cmd === "open_plane")
        throw new Error("/home/dev/notes is not a plane: purlis found no charter.toml");
      return undefined;
    });

    render(<App />);
    await openByPath("/home/dev/notes");

    expect(await screen.findByRole("alert")).toHaveTextContent("is not a plane");
    expect(screen.getByRole("button", { name: OPEN_DOOR })).toBeInTheDocument();
  });

  it("comes back to the welcome when the project is closed, and can open it again", async () => {
    // The way back. Closing is the window letting go of a project — its chats end and its
    // record is written into it — and nothing of it on disk goes, so the same one opens
    // again with everything still in it. Opening it again has to draw its chats again: a
    // window showing a project with no tabs and no way to get them is the state this whole
    // screen exists to remove.
    const { asked } = core((cmd) => {
      if (cmd === "plane_at_launch")
        return { plane: "/home/dev/plane", from: "/home/dev/plane", why: null };
      if (cmd === "open_plane") return { plane: "/home/dev/plane", ask: null };
      if (cmd === "opened_chats")
        return [
          {
            session: 1,
            name: "one",
            cwd: null,
            harness: null,
            in_front: true,
            resumed: null,
            fresh: null,
            profile: null,
            persona: null,
            unreported: null,
            guessed: null,
          },
        ];
      if (cmd === "close_plane") return null;
      return undefined;
    });

    render(<App />);
    const person = userEvent.setup();
    await projectAt("/home/dev/plane");
    // The `×` on the project's own tab, which IS the catalogue's row — one per project, the
    // way a chat tab's close is one per tab.
    await closeFromTheRail("plane");
    // It has a chat open, so it asks first (charter-app#239's ruling), and this answers it.
    await person.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: /^Close and end/,
      }),
    );

    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent("Welcome to purlis");
    expect(asked.some((one) => one.cmd === "close_plane")).toBe(true);

    await openByPath("/home/dev/plane");

    expect(await projectAt("/home/dev/plane")).toBeInTheDocument();
    // Its chats are drawn again, which is what a ref still naming the project it just let go
    // of would have silently prevented.
    await vi.waitFor(() =>
      expect(asked.filter((one) => one.cmd === "opened_chats").length).toBeGreaterThanOrEqual(2),
    );
  });

  it("draws nothing of the last project's chats before the next one has answered", async () => {
    // The window between the switch and the new project's first answer. It is an `await`
    // wide, and what fills it is whatever the last project said — so the fold that arrives
    // afterwards cannot be the only guard, and this is the one that holds in the meantime.
    core((cmd, args) => {
      if (cmd === "plane_at_launch")
        return { plane: "/home/dev/one", from: "/home/dev/one", why: null };
      if (cmd === "open_plane") return { plane: "/home/dev/two", ask: null };
      if (cmd === "chat_states")
        return args.plane === "/home/dev/one"
          ? [
              {
                plane: args.plane,
                session: 1,
                state: "waiting",
                needs_you: true,
                queue: [1],
                sequence: 1,
              },
            ]
          : // The second project never answers, so nothing can arrive to correct the screen.
            new Promise(() => undefined);
      // BOTH projects have a chat 1 — which is the whole point: a session number means
      // nothing without its project, and a screen with no second chat 1 on it could not
      // show the last project's state landing on this one's.
      if (cmd === "opened_chats")
        return [
          {
            session: 1,
            name: args.plane === "/home/dev/one" ? "one" : "two",
            cwd: null,
            harness: null,
            in_front: true,
            resumed: null,
            fresh: null,
            profile: null,
            persona: null,
            unreported: null,
            guessed: null,
          },
        ];
      if (cmd === "close_plane") return null;
      return undefined;
    });

    render(<App />);
    const person = userEvent.setup();
    await vi.waitFor(() =>
      expect(screen.getByRole("img", { name: "waiting on you" })).toBeInTheDocument(),
    );
    await closeFromTheRail("one");
    // It has a chat open, so it asks first (charter-app#239's ruling), and this answers it.
    await person.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: /^Close and end/,
      }),
    );
    await openByPath("/home/dev/two");

    expect(await projectAt("/home/dev/two")).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: "waiting on you" })).not.toBeInTheDocument();
  });

  it("draws nothing of the last project's chats once the next one has answered", async () => {
    // Every project numbers its chats from one, so "session 1 is waiting" is a sentence about
    // a pair and not about a number. The window used to keep what it had been told under the
    // next project's first answer — `underneath` never writes over what it finds, which is
    // right for a snapshot racing an event inside one project and wrong across two.
    const one = {
      session: 1,
      name: "one",
      cwd: null,
      harness: null,
      in_front: true,
      resumed: null,
      fresh: null,
      profile: null,
      persona: null,
      unreported: null,
      guessed: null,
    };
    let showing = "/home/dev/one";
    core((cmd) => {
      if (cmd === "plane_at_launch")
        return { plane: "/home/dev/one", from: "/home/dev/one", why: null };
      if (cmd === "open_plane") return { plane: "/home/dev/two", ask: null };
      if (cmd === "opened_chats") return [one];
      // The first project's chat 1 is waiting for the operator; the second project's is not.
      if (cmd === "chat_states")
        return showing === "/home/dev/one"
          ? [
              {
                plane: showing,
                session: 1,
                state: "waiting",
                needs_you: true,
                queue: [1],
                sequence: 1,
              },
            ]
          : [];
      if (cmd === "close_plane") {
        showing = "/home/dev/two";
        return null;
      }
      return undefined;
    });

    render(<App />);
    const person = userEvent.setup();
    await vi.waitFor(() =>
      expect(screen.getByRole("img", { name: "waiting on you" })).toBeInTheDocument(),
    );
    await closeFromTheRail("one");
    // It has a chat open, so it asks first (charter-app#239's ruling), and this answers it.
    await person.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: /^Close and end/,
      }),
    );
    await openByPath("/home/dev/two");

    await vi.waitFor(() =>
      expect(screen.queryByRole("img", { name: "waiting on you" })).not.toBeInTheDocument(),
    );
  });

  it("tells the core what it holds and which project it has in front", async () => {
    // The half charter-app#111 named as missing: every project numbers its chats from one,
    // so a window showing B would otherwise suppress a notification for A's chat 3 on the
    // strength of A's own answer. It is one call because it is one fact — and the tab strip
    // it carries is what the next cold launch puts back (ADR 0033).
    const { asked } = core((cmd) => {
      if (cmd === "plane_at_launch")
        return { plane: "/home/dev/plane", from: "/home/dev/plane", why: null };
      return undefined;
    });

    render(<App />);

    // The LAST thing it said, not the first: a window says nothing before the core has
    // answered which one the launch opened, and the answer that matters is the current one.
    await vi.waitFor(() =>
      expect(asked.filter((one) => one.cmd === "window_holds_planes").pop()?.args).toEqual({
        held: { planes: ["/home/dev/plane"], active: 0 },
      }),
    );
  });

  it("says a folder dialog that could not open, and stays quiet on a cancel (#1291)", async () => {
    let fails: string | undefined = "the folder dialog could not be opened";
    core((cmd) => {
      if (cmd === "pick_project") {
        if (fails !== undefined) throw fails;
        return null;
      }
      return undefined;
    });
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: OPEN_DOOR }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "the folder dialog could not be opened",
    );

    // A cancel after it says nothing, and leaves no old failure on screen.
    fails = undefined;
    await person.click(screen.getByRole("button", { name: OPEN_DOOR }));
    await vi.waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("gives a failed dialog's line to a newer refusal from a recent row (#1291)", async () => {
    // A recent row opens without the picker or the form. Its refusal is newer than the dialog's
    // failure, so it is the one said: never the old line in its place.
    core((cmd) => {
      if (cmd === "pick_project") throw "the folder dialog could not be opened";
      if (cmd === "open_plane")
        throw new Error("/home/dev/elsewhere is not a plane: purlis found no charter.toml");
      return undefined;
    });
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: OPEN_DOOR }));
    expect(await screen.findByRole("alert")).toHaveTextContent("could not be opened");

    await person.click(screen.getByRole("row", { name: /elsewhere/ }));
    await vi.waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("is not a plane"));
    expect(screen.getByRole("alert")).not.toHaveTextContent("could not be opened");
  });
});

/** The three doors and the sign-in entry, by the names a screen reader reads for them. */
const DOORS = ["Create a project", "Get a project from your forge", OPEN_DOOR];
const SIGN_IN = "Sign in to GitHub or GitLab";

/** What `recent_planes` answers on each machine the doors must show on (#1812). */
const MACHINES: Record<string, unknown> = {
  "a fresh machine": { planes: [], dropped: [], gone: [], forgetful: null },
  "a machine that remembers a project that has gone": {
    planes: [],
    dropped: [],
    gone: [{ path: "/home/dev/gone", said: "/home/dev/gone is no longer there" }],
    forgetful: null,
  },
  "a machine that dropped a row it could not keep": {
    planes: [],
    dropped: ["purlis dropped /home/dev/odd from the recent projects: it is not a folder"],
    forgetful: null,
  },
  "a machine that keeps no store": {
    planes: [],
    dropped: [],
    forgetful: "purlis keeps no machine store on this platform",
  },
};

/** The doors drawn now, in order, by the name a screen reader reads for each. */
function doorsDrawn(): string[] {
  const list = screen.getByRole("list", { name: "Ways to start" });
  return within(list)
    .getAllByRole("button")
    .map(
      (one) =>
        DOORS.find((name) => within(list).queryByRole("button", { name }) === one) ??
        one.textContent ??
        "",
    );
}

describe("the welcome's doors (#1812)", () => {
  for (const [machine, recents] of Object.entries(MACHINES))
    it(`offers all three doors and the sign-in on ${machine}`, async () => {
      core((cmd) => (cmd === "recent_planes" ? recents : undefined));
      render(<App />);

      await screen.findByRole("heading", { level: 1, name: "Welcome to purlis" });
      expect(doorsDrawn()).toEqual(DOORS);
      expect(screen.getByRole("button", { name: SIGN_IN })).toBeInTheDocument();
    });

  it("offers them on a launch from a terminal outside any project, with the reason", async () => {
    core((cmd) => {
      if (cmd === "plane_at_launch")
        return { plane: null, from: "/home/dev/notes", why: "/home/dev/notes is in no project" };
      if (cmd === "recent_planes") return MACHINES["a fresh machine"];
      return undefined;
    });
    render(<App />);

    await screen.findByRole("heading", { level: 1, name: "purlis found no project here" });
    expect(screen.getByText("/home/dev/notes is in no project")).toBeInTheDocument();
    expect(doorsDrawn()).toEqual(DOORS);
    expect(screen.getByRole("button", { name: SIGN_IN })).toBeInTheDocument();
  });

  it("keeps Locate… and Forget beside a gone project, under the doors", async () => {
    core((cmd) =>
      cmd === "recent_planes"
        ? MACHINES["a machine that remembers a project that has gone"]
        : undefined,
    );
    render(<App />);

    expect(await screen.findByText("/home/dev/gone is no longer there")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Locate/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Forget/ })).toBeInTheDocument();
    expect(doorsDrawn()).toEqual(DOORS);
  });

  it("names each door with its title and describes it with its line", async () => {
    core(() => undefined);
    render(<App />);

    for (const name of DOORS) {
      const door = await screen.findByRole("button", { name });
      expect(door).toHaveAccessibleDescription(/\w/);
      expect(door).toHaveAttribute("tabindex", "0");
    }
    expect(screen.getByRole("button", { name: SIGN_IN })).toHaveAttribute("tabindex", "0");
  });

  it("is reached from the keyboard: Tab walks the doors, then the sign-in, then the path box", async () => {
    core(() => undefined);
    render(<App />);
    const person = userEvent.setup();

    const first = await screen.findByRole("button", { name: DOORS[0] });
    first.focus();
    for (const name of DOORS.slice(1)) {
      await person.tab();
      expect(document.activeElement).toBe(screen.getByRole("button", { name }));
    }
    await person.tab();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: SIGN_IN }));
    await person.tab();
    expect(document.activeElement).toBe(screen.getByLabelText("Or type a path"));
  });

  it("says the recent list is empty, and how it fills, on a machine that remembers nothing", async () => {
    core((cmd) => (cmd === "recent_planes" ? MACHINES["a fresh machine"] : undefined));
    render(<App />);

    expect(await screen.findByText("No recent projects yet")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Recent projects" })).not.toBeInTheDocument();
  });

  it("lists recent projects under a heading of their own, and no empty line", async () => {
    core(() => undefined);
    render(<App />);

    const recents = await screen.findByRole("region", { name: "Recent projects" });
    // The region is drawn before the list it holds is read: wait for the row itself.
    expect(await within(recents).findByRole("row", { name: /elsewhere/ })).toBeInTheDocument();
    expect(screen.queryByText("No recent projects yet")).not.toBeInTheDocument();
  });

  it("opens the folder the Open door's dialog picks", async () => {
    const { asked } = core((cmd) => {
      if (cmd === "pick_project") return "/home/dev/plane";
      if (cmd === "open_plane") return { plane: "/home/dev/plane", ask: null };
      return undefined;
    });
    render(<App />);

    await userEvent.setup().click(await screen.findByRole("button", { name: OPEN_DOOR }));

    await vi.waitFor(() =>
      expect(asked.find((one) => one.cmd === "open_plane")?.args).toEqual({
        path: "/home/dev/plane",
      }),
    );
  });

  it("goes to Create a project, and Back returns to the door it came from", async () => {
    core(() => undefined);
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: "Create a project" }));

    const heading = await screen.findByRole("heading", { level: 1, name: "Create a project" });
    // The keyboard lands on the page it went to, so a screen reader says where it is.
    expect(document.activeElement).toBe(heading);
    expect(screen.getByRole("button", { name: "Open a repo…" })).toBeInTheDocument();

    await person.click(screen.getByRole("button", { name: "Back" }));
    await screen.findByRole("heading", { level: 1, name: "Welcome to purlis" });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Create a project" }));
  });

  it("offers New project… from Create a project, for a project in a folder of its own", async () => {
    core(() => undefined);
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await person.click(await screen.findByRole("button", { name: "New project…" }));

    expect(await screen.findByRole("dialog", { name: "New project" })).toBeInTheDocument();
  });

  it("asks for a project's URL behind its door, and still offers the folder way", async () => {
    const { asked } = core((cmd) => (cmd === "pick_project" ? null : undefined));
    render(<App />);
    const person = userEvent.setup();

    await person.click(
      await screen.findByRole("button", { name: "Get a project from your forge" }),
    );

    await screen.findByRole("heading", { level: 1, name: "Get a project from your forge" });
    expect(screen.getByLabelText("Project URL")).toBeInTheDocument();
    // Nothing is cloned until the person presses Get project (#1815).
    expect(asked.some((one) => one.cmd === "get_project")).toBe(false);
    await person.click(screen.getByRole("button", { name: "Open a folder…" }));
    await vi.waitFor(() => expect(asked.some((one) => one.cmd === "pick_project")).toBe(true));

    await person.click(screen.getByRole("button", { name: "Back" }));
    expect(
      await screen.findByRole("button", { name: "Get a project from your forge" }),
    ).toHaveFocus();
  });

  it("offers the browser sign-in beside the token, and today's CLI sign-in under them", async () => {
    core((cmd) =>
      cmd === "first_run_found"
        ? {
            harnesses: [],
            forges: [
              { cli: "gh", forge: "github", title: "GitHub", installed: true, signed_in: false },
              { cli: "glab", forge: "gitlab", title: "GitLab", installed: false, signed_in: false },
            ],
            templates: [],
          }
        : undefined,
    );
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: SIGN_IN }));

    await screen.findByRole("heading", { level: 1, name: SIGN_IN });
    expect(screen.queryByText(/comes in a later update/)).not.toBeInTheDocument();
    expect(
      screen.getByText(/Sign in to github.com or gitlab.com through your browser/),
    ).toBeInTheDocument();
    // Both ways in, side by side, the browser's first (#729, ruling Q3).
    const ways = screen
      .getAllByRole("button")
      .map((button) => button.textContent)
      .filter((name) => name?.startsWith("Sign in with"));
    expect(ways).toEqual(["Sign in with GitHub", "Sign in with GitLab", "Sign in with a token…"]);
    const found = await screen.findByRole("list", { name: "On this machine" });
    expect(
      within(found)
        .getAllByRole("listitem")
        .map((row) => row.textContent),
    ).toEqual([
      "gh: installed, not signed in; only needed to work with GitHub Sign in to GitHub",
      "glab: not installed; only needed to work with GitLab",
    ]);
  });

  it("signs in with a token from its page, and says who it signed in as", async () => {
    const { asked } = core((cmd) => {
      if (cmd === "first_run_found") return { harnesses: [], forges: [], templates: [] };
      if (cmd === "forge_token_help")
        return {
          host: "gitlab.com",
          scopes: ["api"],
          create_at: "https://gitlab.com/-/user_settings/personal_access_tokens",
        };
      if (cmd === "forge_sign_in_with_token")
        return {
          id: "01J9ACCOUNT",
          kind: "gitlab",
          host: "gitlab.com",
          login: "octocat",
          method: "token",
          signed_in: true,
          scopes: ["api"],
          wider: [],
        };
      return undefined;
    });
    render(<App />);
    const person = userEvent.setup();

    await person.click(await screen.findByRole("button", { name: SIGN_IN }));
    await person.click(await screen.findByRole("button", { name: "Sign in with a token…" }));
    const dialog = await screen.findByRole("dialog", { name: "Sign in with a token" });
    await person.click(within(dialog).getByRole("radio", { name: /GitLab/ }));
    await person.type(within(dialog).getByLabelText("Token"), "pasted-for-the-welcome");
    await person.click(within(dialog).getByRole("button", { name: "Sign in" }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      "Signed in to gitlab.com as octocat.",
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    const sent = asked.find((one) => one.cmd === "forge_sign_in_with_token");
    expect(sent?.args).toMatchObject({ kind: "gitlab", host: "gitlab.com" });
  });

  it("says a sign-in's own refusal on its page, and not one from before it opened", async () => {
    const { asked } = core((cmd) => {
      if (cmd === "first_run_found")
        return {
          harnesses: [],
          forges: [
            { cli: "gh", forge: "github", title: "GitHub", installed: true, signed_in: false },
          ],
          templates: [],
        };
      if (cmd === "open_repo") throw "/home/dev/papers is not the top level of a git repo";
      if (cmd === "open_local_project") throw "purlis could not make the project on this machine";
      return undefined;
    });
    render(<App />);
    const person = userEvent.setup();

    // A refusal on Create a project, left there.
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await person.type(await screen.findByLabelText("Or type the repo's path"), "/home/dev/papers");
    await person.click(screen.getByRole("button", { name: "Open repo" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("not the top level");
    await person.click(screen.getByRole("button", { name: "Back" }));

    await person.click(await screen.findByRole("button", { name: SIGN_IN }));
    await screen.findByRole("list", { name: "On this machine" });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    await person.click(screen.getByRole("button", { name: "Sign in to GitHub" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "purlis could not make the project on this machine",
    );
    expect(asked.filter((one) => one.cmd === "open_local_project")).toHaveLength(1);
  });
});
