// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { answerRowOf, answersOf } from "./ui/dialog/dialog.testkit";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { userEvent } from "@testing-library/user-event";
import type { StartOptions } from "./bindings";
import { StartChat } from "./StartChat";
import { profileCommand, profilePage } from "./settings/profileAddress";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ChatsHere, fixedChats } from "./chatState";

afterEach(cleanup);

const ONE: StartOptions = {
  profiles: [
    {
      name: "claude",
      kind: "claude",
      shown: "claude",
      source: "built-in",
      is_default: true,
      ready_to_type: true,
      harness: null,
      sandbox: null,
      approval: null,
    },
  ],
  refused: [],
  personas: ["steward", "release"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  ignore_fix_id: null,
  declares_none: true,
};

const options = (over: Partial<StartOptions> = {}): StartOptions => ({ ...ONE, ...over });

/** ⌘⏎ on a Mac, Ctrl+Enter elsewhere: the platform the test runs on. */
const MAC = navigator.platform.startsWith("Mac");
const SEND = { key: "Enter", metaKey: MAC, ctrlKey: !MAC };

function show(over: Partial<StartOptions> = {}, repo?: string) {
  const onStart = vi.fn();
  const onApprove = vi.fn();
  const onCancel = vi.fn();
  render(
    <StartChat
      options={options(over)}
      repo={repo}
      onStart={onStart}
      onApprove={onApprove}
      onCancel={onCancel}
    />,
  );
  return { onStart, onApprove, onCancel, user: userEvent.setup() };
}

/** The line under the picked persona's name, at the top of the details (R23). */
const whoLine = () => screen.getByTestId("start-chat-who").querySelector("p")?.textContent;

describe("a persona's one-line description (#1460)", () => {
  it("is said under the picked persona's name, after that it is the project's default (R23)", async () => {
    const { user } = show({
      personas: ["steward", "release", "ops"],
      persona_descriptions: { steward: "Scopes the work", ops: "Keeps the lights on" },
    });

    expect(whoLine()).toBe("Project default · Scopes the work");
    await user.click(screen.getByRole("option", { name: "ops" }));
    expect(whoLine()).toBe("Keeps the lights on");
    await user.click(screen.getByRole("option", { name: "release" }));
    expect(whoLine()).toBe("Persona of this project");
  });

  it("is nothing for an answer that carries none", () => {
    show({ persona_descriptions: undefined });

    expect(whoLine()).toBe("Project default");
  });
});

describe("a model choice is a profile and a model, billed to someone (#2036)", () => {
  const rows: Partial<StartOptions> = {
    profiles: [
      {
        ...ONE.profiles[0],
        model: "the harness's default",
        billed_to: "whatever Claude Code is signed in with · claude",
      },
      {
        ...ONE.profiles[0],
        name: "work",
        source: "this machine",
        is_default: false,
        model: "the harness's default",
        billed_to: "your key · Work Anthropic key",
      },
      {
        ...ONE.profiles[0],
        name: "team",
        kind: "codex",
        shown: "codex",
        source: "this machine",
        is_default: false,
        model: "the harness's default",
        billed_to: "your cloud account · team Azure",
      },
    ],
  };

  it("shows every choice's profile, model and what it is billed to", () => {
    show(rows);

    for (const [name, billed] of [
      ["claude", "whatever Claude Code is signed in with · claude"],
      ["work", "your key · Work Anthropic key"],
      ["team", "your cloud account · team Azure"],
    ]) {
      const row = screen.getByRole("radio", { name });
      expect(row).toHaveAccessibleDescription(new RegExp(`Model: the harness's default`));
      expect(row).toHaveAccessibleDescription(new RegExp(`Billed to ${billed}`));
    }
  });

  it("says nothing of a bill for an answer that carries none", () => {
    show();

    expect(screen.getByRole("radio", { name: "claude" })).not.toHaveAccessibleDescription(
      /billed to/i,
    );
  });
});

describe("a persona's own profile (#1445)", () => {
  const codex = { ...ONE.profiles[0], name: "codex", kind: "codex", shown: "codex" };
  const work = { ...ONE.profiles[0], name: "work", shown: "claude --work", is_default: false };
  const TWO: Partial<StartOptions> = {
    profiles: [ONE.profiles[0], { ...codex, is_default: false }, work],
    personas: ["steward", "release", "ops"],
    persona_profiles: { release: "codex" },
  };

  it("moves the harness to a persona's own profile when that persona is picked", async () => {
    const { onStart, user } = show(TWO);
    expect(screen.getByRole("radio", { name: /^claude/ })).toBeChecked();

    await user.click(screen.getByRole("option", { name: "release" }));

    expect(screen.getByRole("radio", { name: /^codex/ })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "Start chat" }));
    expect(onStart).toHaveBeenCalledWith("codex", "release", false, null, false, null);
  });

  it("says which profile is a persona's own under its name", async () => {
    const { user } = show(TWO);

    await user.click(screen.getByRole("option", { name: "release" }));
    expect(whoLine()).toBe("its profile is codex");
    await user.click(screen.getByRole("option", { name: "ops" }));
    expect(whoLine()).toBe("Persona of this project");
  });

  it("leaves the harness the person's to change after a persona is picked", async () => {
    const { onStart, user } = show(TWO);

    await user.click(screen.getByRole("option", { name: "release" }));
    await user.click(screen.getByRole("radio", { name: /^work/ }));
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("work", "release", false, null, false, null);
  });

  it("starts on the default persona's own profile", () => {
    show({ ...TWO, persona_profiles: { steward: "codex" } });

    expect(screen.getByRole("radio", { name: /^codex/ })).toBeChecked();
  });

  it("keeps a harness the person picked when a persona with its own profile is picked after", async () => {
    // Harness first, then persona: the Harness row is drawn above the Persona row.
    const { onStart, user } = show(TWO);

    await user.click(screen.getByRole("radio", { name: /^work/ }));
    await user.click(screen.getByRole("option", { name: "release" }));

    expect(screen.getByRole("radio", { name: /^work/ })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "Start chat" }));
    expect(onStart).toHaveBeenCalledWith("work", "release", false, null, false, null);
  });

  it("keeps a harness the person picked for a persona that names no profile", async () => {
    const { user } = show(TWO);

    await user.click(screen.getByRole("radio", { name: /^work/ }));
    await user.click(screen.getByRole("option", { name: "ops" }));

    expect(screen.getByRole("radio", { name: /^work/ })).toBeChecked();
  });

  it("goes back to the default when the persona picked next names no profile", async () => {
    // Persona with a profile, then one without: codex must not be left over from `release`.
    const { onStart, user } = show(TWO);

    await user.click(screen.getByRole("option", { name: "release" }));
    expect(screen.getByRole("radio", { name: /^codex/ })).toBeChecked();
    await user.click(screen.getByRole("option", { name: "ops" }));

    expect(screen.getByRole("radio", { name: /^claude/ })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "Start chat" }));
    expect(onStart).toHaveBeenCalledWith("claude", "ops", false, null, false, null);
  });

  it("goes back to the default when no persona is picked after one with a profile", async () => {
    const { user } = show(TWO);

    await user.click(screen.getByRole("option", { name: "release" }));
    await user.click(screen.getByRole("option", { name: "No persona" }));

    expect(screen.getByRole("radio", { name: /^claude/ })).toBeChecked();
  });

  it("leaves a harness something asked for where it is", async () => {
    const onStart = vi.fn();
    render(
      <StartChat
        options={options(TWO)}
        prefer="claude"
        onStart={onStart}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByRole("option", { name: "release" }));

    expect(screen.getByRole("radio", { name: /^claude/ })).toBeChecked();
  });

  it("moves nowhere for a profile the picker has no row for", async () => {
    const { user } = show({ ...TWO, persona_profiles: { release: "gone" } });

    await user.click(screen.getByRole("option", { name: "release" }));

    expect(screen.getByRole("radio", { name: /^claude/ })).toBeChecked();
  });
});

describe("a chat that starts in a repo (GL-1)", () => {
  it("starts on a new branch in that repo unless told otherwise", async () => {
    const { onStart, user } = show({}, "api");

    // Sentence case, as every label in the window (`docs/ui-copy.md`, #630).
    expect(screen.getByRole("checkbox", { name: "Start on a new branch in api" })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, true, null);
  });

  it("works on the branch the repo has checked out when the box is cleared", async () => {
    const { onStart, user } = show({}, "api");

    await user.click(screen.getByRole("checkbox", { name: /new branch in api/ }));
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, null);
  });

  it("asks nothing about a branch for a chat that does not start in a repo", () => {
    show();

    expect(screen.queryByRole("checkbox", { name: /new branch/ })).not.toBeInTheDocument();
  });

  it("draws two panes, the persona list beside the details, in a wide dialog at a fixed height (#2918)", () => {
    // Prototype B of #2798: the list on the left, the harness cards, the name and the boxes on
    // the right, each drawn by its family (`ui/list`, `ui/choice`, `ui/field`).
    show({}, "api");

    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("data-size", "wide");
    expect(dialog).toHaveAttribute("data-fixed-height");
    expect(dialog.style.getPropertyValue("--dialog-height")).toBe("620px");
    const [left, right] = [...dialog.querySelectorAll(".start-chat-panes > *")];
    expect(left).toContainElement(screen.getByRole("listbox", { name: "Persona" }));
    expect(right).toContainElement(screen.getByRole("radiogroup", { name: "Harness" }));
    expect(right).toContainElement(screen.getByRole("textbox", { name: "Name" }));
    expect(right).toContainElement(screen.getByRole("checkbox", { name: /purlis's footer/ }));
    // Harness cards with no radio circle (R24).
    expect(dialog.querySelectorAll('[data-scope="option-card"][data-part="root"]')).toHaveLength(1);
    expect(dialog.querySelector('[data-scope="radio"]')).toBeNull();
  });

  /** The dialog's last row of buttons, and the names of the buttons in it, in order. */
  const lastRow = () => {
    const dialog = screen.getByRole("dialog");
    return { row: answerRowOf(dialog), names: answersOf(dialog) };
  };

  it("ends in the dialog's footer, Cancel first and the start last (#2910)", () => {
    // The dialog rules (#2910, N2): every dialog's main action is in one place, last in its
    // footer, a form's as a question's.
    show({}, "api");

    const { row, names } = lastRow();
    expect(row).not.toBeNull();
    expect(names).toEqual(["Cancel", "Start chat"]);
    const start = screen.getByRole("button", { name: "Start chat" });
    expect(start).toHaveAttribute("data-variant", "primary");
    // A button and not a submit: Return in the Name box never starts a command.
    expect(start).toHaveAttribute("type", "button");
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveAttribute("type", "button");
  });

  it("holds the room of the words its start can reach here, and no more (R163, #3378 B1)", () => {
    // A choice moves and widens nothing: the start keeps the room of the longest words this
    // dialog can reach. One whose profiles ask no approval and run no sandbox is not drawn as
    // wide as "Approve and start without the sandbox".
    const roomOf = (start: HTMLElement) => ({
      kept: [...start.querySelectorAll('[data-scope="reserved"][data-part="kept"]')].map((one) =>
        one.getAttribute("data-text"),
      ),
      lock: start.querySelectorAll("svg").length,
    });
    show({}, "api");
    expect(roomOf(screen.getByRole("button", { name: "Start chat" }))).toEqual({
      kept: ["Start chat", "Starting…"],
      lock: 0,
    });
    cleanup();

    const asking = { ...ONE.profiles[0], name: "asks", shown: "claude --x", approval: "new" };
    const sandboxed = {
      ...ONE.profiles[0],
      sandbox: { state: "sandboxed", said: "", install: null, locked: null },
    } as const;
    show({ profiles: [sandboxed, { ...asking, is_default: false }] });
    const plain = roomOf(screen.getByRole("button", { name: "Start chat" }));
    expect(plain).toEqual({
      kept: [
        "Start chat",
        "Starting…",
        "Start without the sandbox",
        "Approve and start",
        "Approve and start without the sandbox",
      ],
      lock: 1,
    });
    fireEvent.click(screen.getByRole("radio", { name: "asks" }));
    expect(roomOf(screen.getByRole("button", { name: "Approve and start" }))).toEqual(plain);
  });

  it("ends in the same footer when the start approves a command first (#2910)", () => {
    show({
      profiles: [{ ...ONE.profiles[0], name: "claude", shown: "claude --x", approval: "new" }],
    });

    const { row, names } = lastRow();
    expect(row).not.toBeNull();
    expect(names).toEqual(["Cancel", "Approve and start"]);
    const approve = screen.getByRole("button", { name: "Approve and start" });
    // Approving is the form's act, not a confirm that ends something (D-1719-3): purlis can
    // take the chat back, and the command is said above it before the press (V69).
    // It grants the command its run: the grant's style, not the suggested answer's (ADR 0097 §6).
    expect(approve).toHaveAttribute("data-variant", "grants");
    expect(approve).not.toHaveAttribute("data-variant", "primary");
    expect(approve).not.toHaveAttribute("data-variant", "danger");
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
  });

  it("says nothing the first hour does not say", () => {
    // ADR 0072 §3: the picker is a first-hour surface, so it says branch and never the
    // words charter keeps for its own internals.
    show({}, "api");

    const said = screen.getByRole("dialog").textContent ?? "";
    expect(said).not.toMatch(/worktree|piece/i);
  });
});

describe("a start already running (GL-1)", () => {
  it("says so on Start, which cannot be pressed again", async () => {
    const onStart = vi.fn();
    render(
      <StartChat
        options={options()}
        starting
        onStart={onStart}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    const start = screen.getByRole("button", { name: "Starting…" });
    expect(start).toBeDisabled();
    await userEvent.setup().click(start);
    expect(onStart).not.toHaveBeenCalled();
  });
});

describe("the picker a chat starts from", () => {
  it("shows even when one profile is available, because one profile costs one Enter", () => {
    // Skipping it would bring back the harness nobody picked on a one-harness machine,
    // which is one of the two failures ADR 0022 exists to prevent.
    show();

    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /claude/ })).toBeChecked();
  });

  it("starts nothing on Escape", async () => {
    const { onCancel, onStart, user } = show();

    await user.keyboard("{Escape}");

    expect(onCancel).toHaveBeenCalled();
    expect(onStart).not.toHaveBeenCalled();
  });

  describe("the keyboard once it closes (#1600)", () => {
    // The window draws the dialog once the options are read, and closes it by no longer
    // drawing it: the whole dialog goes at once, as `PlaneView` does it.
    function Window() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            New tab
          </button>
          {open && (
            <StartChat
              options={options()}
              onStart={() => setOpen(false)}
              onApprove={vi.fn()}
              onCancel={() => setOpen(false)}
            />
          )}
        </>
      );
    }

    async function opened() {
      render(<Window />);
      const user = userEvent.setup();
      const plus = screen.getByRole("button", { name: "New tab" });
      plus.focus();
      await user.keyboard("{Enter}");
      await screen.findByRole("dialog", { name: "Start a chat" });
      return { plus, user };
    }

    it("goes back to the + that opened it on Escape", async () => {
      const { plus, user } = await opened();

      await user.keyboard("{Escape}");

      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      await waitFor(() => expect(plus).toHaveFocus());
    });

    it("goes back to the + on Cancel", async () => {
      const { plus, user } = await opened();

      await user.click(screen.getByRole("button", { name: "Cancel" }));

      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      await waitFor(() => expect(plus).toHaveFocus());
    });

    it("is left for the new chat on a start", async () => {
      const { plus, user } = await opened();

      await user.click(screen.getByRole("button", { name: "Start chat" }));

      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      await new Promise((resolve) => setTimeout(resolve, 10));
      expect(plus).not.toHaveFocus();
    });
  });

  it("starts on the profile and the persona that were picked", async () => {
    const { onStart, user } = show();

    await user.click(screen.getByRole("option", { name: "release" }));
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "release", false, null, false, null);
  });

  it("starts on the plane's own default persona when nobody picks another", async () => {
    const { onStart, user } = show();

    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, null);
  });

  it("can start a chat that adopts no persona at all", async () => {
    const { onStart, user } = show();

    await user.click(screen.getByRole("option", { name: "No persona" }));
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", null, false, null, false, null);
  });

  it("leaves the pane's footer blank unless this chat asks for charter's", async () => {
    // The default, and ADR 0029 keeps it: nobody's pane moves on an upgrade. The
    // box is drawn unticked and the start carries `false`.
    const { onStart, user } = show();

    expect(screen.getByRole("checkbox", { name: /purlis's footer/ })).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, null);
  });

  it("starts a chat that draws purlis's footer when the box is ticked", async () => {
    const { onStart, user } = show();

    await user.click(screen.getByRole("checkbox", { name: /purlis's footer/ }));
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", true, null, false, null);
  });

  it("starts under the name typed in the Name field (charter-app#254)", async () => {
    const { onStart, user } = show();

    await user.type(screen.getByRole("textbox", { name: /^Name/ }), "billing bug");
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, "billing bug", false, null);
  });

  it("starts on ⌘⏎ from the Name field, where a plain Return starts nothing (N26)", async () => {
    const { onStart, user } = show();
    const name = screen.getByRole("textbox", { name: /^Name/ });
    await user.type(name, "billing bug");
    fireEvent.keyDown(name, { key: "Enter" });
    expect(onStart).not.toHaveBeenCalled();
    fireEvent.keyDown(name, SEND);
    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, "billing bug", false, null);
    expect(screen.getByRole("button", { name: "Start chat" })).toHaveAttribute(
      "aria-keyshortcuts",
      MAC ? "Meta+Enter" : "Control+Enter",
    );
  });

  it("never approves a command on ⌘⏎: a grant is a press of its own button (ADR 0097 §6)", () => {
    const { onStart, onApprove } = show({
      profiles: [{ ...ONE.profiles[0], name: "claude", shown: "claude --x", approval: "new" }],
    });
    fireEvent.keyDown(screen.getByRole("button", { name: "Cancel" }), SEND);
    expect(onApprove).not.toHaveBeenCalled();
    expect(onStart).not.toHaveBeenCalled();
  });

  it("never starts without the sandbox on ⌘⏎, from Cancel or the Name field (ADR 0097 §6)", () => {
    const { onStart } = show({
      profiles: [
        {
          ...ONE.profiles[0],
          sandbox: {
            state: "refused",
            said: "bwrap is not installed.",
            install: null,
            locked: null,
          },
        },
      ],
    });
    expect(screen.getByRole("button", { name: "Start without the sandbox" })).toBeInTheDocument();
    const cancel = screen.getByRole("button", { name: "Cancel" });
    expect(cancel).toHaveFocus();
    fireEvent.keyDown(cancel, SEND);
    fireEvent.keyDown(screen.getByRole("textbox", { name: /^Name/ }), SEND);
    expect(onStart).not.toHaveBeenCalled();
  });

  it("never starts without the sandbox on ⌘⏎ once the opt-out is ticked (ADR 0097 §6)", async () => {
    const { onStart, user } = show({
      profiles: [
        {
          ...ONE.profiles[0],
          sandbox: { state: "sandboxed", said: "", install: null, locked: null },
        },
      ],
    });
    const optOut = screen.getByRole("checkbox", { name: "Start without the sandbox" });
    await user.click(optOut);
    expect(optOut).toBeChecked();
    expect(screen.getByRole("button", { name: "Start without the sandbox" })).toBeInTheDocument();
    fireEvent.keyDown(optOut, SEND);
    fireEvent.keyDown(screen.getByRole("button", { name: "Cancel" }), SEND);
    fireEvent.keyDown(screen.getByRole("textbox", { name: /^Name/ }), SEND);
    expect(onStart).not.toHaveBeenCalled();
    // Unticked, the chord starts the chat sandboxed again.
    await user.click(optOut);
    fireEvent.keyDown(screen.getByRole("textbox", { name: /^Name/ }), SEND);
    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, null);
  });

  it("starts under the default name when the Name field is only spaces", async () => {
    const { onStart, user } = show();

    await user.type(screen.getByRole("textbox", { name: /^Name/ }), "   ");
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, null);
  });

  it("says the Name field is optional and what an empty one means", () => {
    show();

    expect(screen.getByRole("textbox", { name: /^Name/ })).toHaveAccessibleDescription(
      /empty.*persona.*number/i,
    );
  });

  it("says what the footer adds, rather than leaving the box to be guessed at (R31)", () => {
    // The panels repeat most of it, and it says something about THIS chat that they say
    // only for the focused one.
    show();

    expect(
      screen.getByRole("checkbox", { name: "Draw purlis's footer in this chat" }),
    ).toHaveAccessibleDescription(
      "The footer names the chat's workspace, which the panels name only for the focused chat.",
    );
  });

  it("carries the footer choice through the approval, which is where a pick gets dropped", async () => {
    // The persona used to be dropped on exactly this path — a profile's first run. The
    // footer rides the same call and would be lost the same way.
    const { onApprove, user } = show({
      profiles: [
        {
          name: "work",
          kind: "claude",
          shown: "claude --model opus",
          source: "charter.local.toml",
          is_default: true,
          ready_to_type: true,
          harness: null,
          sandbox: null,
          approval: "new",
        },
      ],
    });

    await user.click(screen.getByRole("checkbox", { name: /purlis's footer/ }));
    await user.click(screen.getByRole("button", { name: "Approve and start" }));

    expect(onApprove).toHaveBeenCalledWith(
      "work",
      "steward",
      true,
      "claude --model opus",
      null,
      false,
      null,
    );
  });

  it("shows the command and asks, for a profile charter has not recorded running", async () => {
    // The file is gitignored, so an edit to it leaves no diff for a reviewer to catch —
    // which is why the ask is about the WORDS that are about to run, not the profile's name.
    const { onStart, onApprove, user } = show({
      profiles: [
        {
          name: "work",
          kind: "claude",
          shown: "CLAUDE_CONFIG_DIR=~/.claude-work claude --model opus",
          source: "charter.local.toml",
          is_default: true,
          ready_to_type: true,
          harness: null,
          sandbox: null,
          approval: "new",
        },
      ],
    });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "CLAUDE_CONFIG_DIR=~/.claude-work claude --model opus",
    );
    await user.click(screen.getByRole("button", { name: "Approve and start" }));

    // With the persona, not only the profile: the approve path used to drop the pick.
    expect(onApprove).toHaveBeenCalledWith(
      "work",
      "steward",
      false,
      "CLAUDE_CONFIG_DIR=~/.claude-work claude --model opus",
      null,
      false,
      null,
    );
    expect(onStart).not.toHaveBeenCalled();
  });

  it("shows a command longer than the display limit whole, and approves that line (#1014)", async () => {
    const long = `claude --note ${"x".repeat(200)} --and-then the-last-word (kind claude)`;
    const { onApprove, user } = show({
      profiles: [
        {
          name: "work",
          kind: "claude",
          shown: long,
          source: "charter.local.toml",
          is_default: true,
          ready_to_type: true,
          harness: null,
          sandbox: null,
          approval: "new",
        },
      ],
    });

    expect(screen.getByRole("alert").querySelector("code")?.textContent).toBe(long);
    await user.click(screen.getByRole("button", { name: "Approve and start" }));

    expect(onApprove).toHaveBeenCalledWith("work", "steward", false, long, null, false, null);
  });

  it("says a changed command is changed rather than new", () => {
    show({
      profiles: [
        {
          name: "work",
          kind: "claude",
          shown: "claude",
          source: "charter.local.toml",
          is_default: true,
          ready_to_type: true,
          harness: null,
          sandbox: null,
          approval: "changed",
        },
      ],
    });

    expect(screen.getByRole("alert")).toHaveTextContent("as it now stands");
  });

  it("says a project that declares nothing declares nothing, rather than looking broken", () => {
    show();

    // "project", the window's word (#602), never the retired "plane".
    expect(screen.getByText(/This project declares no profiles of its own/)).toBeInTheDocument();
    expect(screen.queryByText(/\bplane\b/)).not.toBeInTheDocument();
  });

  it("says when git would carry the file, because then every declared profile is refused", () => {
    show({ declares_none: false, ignore_fix: "charter reinit" });

    expect(screen.getByRole("alert")).toHaveTextContent("charter reinit");
  });

  it("names the profiles it will not use, so a missing row is never merely missing", async () => {
    const { user } = show({
      refused: [["bad-kind", "profile 'bad-kind' has kind opencodex, which is not a harness"]],
    });

    await user.click(screen.getByText("1 refused"));

    expect(screen.getByText(/has kind opencodex/)).toBeInTheDocument();
  });

  describe("its refusals' ways out (NO-8, #1233)", () => {
    function withWays(over: Partial<StartOptions>, fixing = false) {
      const onFix = vi.fn();
      const onOpenSettings = vi.fn();
      render(
        <StartChat
          options={options(over)}
          fixing={fixing}
          onStart={vi.fn()}
          onApprove={vi.fn()}
          onFix={onFix}
          onOpenSettings={onOpenSettings}
          onCancel={vi.fn()}
        />,
      );
      return { onFix, onOpenSettings, user: userEvent.setup() };
    }

    it("links each refused profile to its own page in Settings, its command focused (#1296)", async () => {
      const { onOpenSettings, user } = withWays({
        refused: [
          ["bad-kind", "profile 'bad-kind' has kind opencodex, which is not a harness"],
          ["team", "profile 'team' has no command"],
        ],
      });

      await user.click(screen.getByText("2 refused"));
      await user.click(screen.getByRole("button", { name: "Open team in Settings" }));

      // A profile with no page of its own (a committed one) lands on Harness & profiles: the
      // Settings tab takes an unknown sub-page to its parent group.
      expect(onOpenSettings).toHaveBeenCalledWith(profilePage("team"), profileCommand("team"));
      expect(screen.getByRole("button", { name: "Open bad-kind in Settings" })).toBeVisible();
    });

    it("fixes the file git would carry with the doctor's own fix, where one line cures it", async () => {
      const { onFix, user } = withWays({
        declares_none: false,
        persona_profiles: {},
        ignore_fix: "add /charter.local.toml to .gitignore",
        ignore_fix_id: "local-ignore",
      });

      await user.click(screen.getByRole("button", { name: "Add the ignore line" }));

      expect(onFix).toHaveBeenCalledWith("local-ignore");
    });

    it("says the fix is running and cannot be pressed twice", () => {
      withWays({ declares_none: false, ignore_fix: "x", ignore_fix_id: "local-ignore" }, true);

      expect(screen.getByRole("button", { name: "Adding the ignore line…" })).toBeDisabled();
    });

    it("offers no button where one ignore line would not cure it", () => {
      // A file git already tracks needs the operator's own `git rm --cached` first (FX-2).
      withWays({ declares_none: false, ignore_fix: "git rm --cached", ignore_fix_id: null });

      expect(screen.queryByRole("button", { name: /ignore line/ })).not.toBeInTheDocument();
      expect(screen.getByRole("alert")).toHaveTextContent("git rm --cached");
    });
  });

  it("puts Cancel under the key a stray Return finds, never the start", () => {
    // Starting a chat runs a command with nothing between the key and the exec.
    show();

    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
  });
  it("offers a chat with no persona at all when the plane declares none", () => {
    // A plane with no `personas/` at all is the ordinary first state, and `_shared` is the
    // store every persona reads rather than a persona anybody adopts — so a plane can
    // legitimately have nothing to list here. The picker still starts a chat.
    const { onStart, user } = show({ personas: [], persona: null });

    expect(screen.getByRole("option", { name: "No persona" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    return user.click(screen.getByRole("button", { name: "Start chat" })).then(() => {
      expect(onStart).toHaveBeenCalledWith("claude", null, false, null, false, null);
    });
  });

  describe("the markup underneath it", () => {
    // The dialog the operator opened read `claudeclaudeclaudebuilt-indefault`: five spans in
    // one `<label>`, with no rule laying them out and no association between the words and
    // the control. These pin what replaced it (`docs/ui-primitives.md`), and they are the
    // tests a hand-rolled version cannot pass.

    const WORK: StartOptions = options({
      profiles: [
        {
          name: "work",
          kind: "claude",
          shown: "CLAUDE_CONFIG_DIR=~/.claude-work claude --model opus",
          source: "charter.local.toml",
          is_default: true,
          ready_to_type: true,
          harness: null,
          sandbox: null,
          approval: null,
        },
        {
          name: "plain",
          kind: "claude",
          shown: "claude",
          source: "built-in",
          is_default: false,
          ready_to_type: true,
          harness: null,
          sandbox: null,
          approval: null,
        },
      ],
    });

    it("calls a row by its profile's name and not by every column in it", () => {
      // `name: "work"` is exact. It was `workclaudeCLAUDE_CONFIG_DIR=…charter.local.tomldefault`
      // — the whole row run together, which is what both a screen reader and the screen got.
      show(WORK);

      expect(screen.getByRole("radio", { name: "work" })).toBeInTheDocument();
      expect(screen.getByRole("radio", { name: "plain" })).toBeInTheDocument();
    });

    it("keeps the command line on the row as its description, not folded into its name", () => {
      // The words that are about to run still have to be readable beside the row that runs
      // them — moving them out of the name must not move them off the screen.
      show(WORK);

      expect(screen.getByRole("radio", { name: "work" })).toHaveAccessibleDescription(
        /CLAUDE_CONFIG_DIR=~\/\.claude-work claude --model opus/,
      );
      expect(
        screen.getByText("CLAUDE_CONFIG_DIR=~/.claude-work claude --model opus"),
      ).toBeInTheDocument();
    });

    it("picks the row the label was clicked on, because the label is tied to the control", () => {
      // The association is the whole point: the words were beside the control and named
      // nothing, so clicking them did nothing.
      const { onStart, user } = show(WORK);

      return user
        .click(screen.getByText("plain"))
        .then(() => user.click(screen.getByRole("button", { name: "Start chat" })))
        .then(() => {
          expect(onStart).toHaveBeenCalledWith("plain", "steward", false, null, false, null);
        });
    });

    it("moves between harnesses with the arrow keys, as one group and not five stops", async () => {
      const { onStart, user } = show(WORK);

      await user.click(screen.getByRole("radio", { name: "work" }));
      await user.keyboard("{ArrowDown}");
      await user.click(screen.getByRole("button", { name: "Start chat" }));

      expect(onStart).toHaveBeenCalledWith("plain", "steward", false, null, false, null);
    });

    it("starts nothing while an arrow key is held on the harnesses", async () => {
      // A held key repeats, and a radio's pick follows it: the pick is only ever held, and
      // the chat starts on Start (DS-3d).
      const { onStart, onApprove, user } = show(WORK);

      await user.click(screen.getByRole("radio", { name: "work" }));
      await user.keyboard("{ArrowDown>}");
      await new Promise((done) => setTimeout(done, 80));
      await user.keyboard("{/ArrowDown}");

      expect(screen.getByRole("radio", { name: "plain" })).toBeChecked();
      expect(onStart).not.toHaveBeenCalled();
      expect(onApprove).not.toHaveBeenCalled();
    });

    it("hides the window behind it from the keyboard and from the accessibility tree", () => {
      // Modal was a word in an attribute and a grey overlay. Nothing enforced it: the tab
      // strip behind the picker was reachable by Tab and listed by every role query, and one
      // of this repo's own tests was ending a chat through it while a chat was starting.
      const onStart = vi.fn();
      render(
        <>
          <button>a tab behind it</button>
          <StartChat options={options()} onStart={onStart} onApprove={vi.fn()} onCancel={vi.fn()} />
        </>,
      );

      expect(screen.queryByRole("button", { name: "a tab behind it" })).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    });
  });

  it("never preselects a persona it does not draw a row for", async () => {
    // The core filters `[persona] default` against the personas the plane has, so this
    // should be unreachable from the app. Pinned anyway: if it ever arrives, the picker
    // must not start a chat on a persona the operator cannot see or change.
    const { onStart, user } = show({ personas: ["release"], persona: "gone" });

    await user.click(screen.getByRole("button", { name: "Start chat" }));

    const [, persona] = onStart.mock.calls[0] as [string, string | null];
    expect(["release", null]).toContain(persona);
  });
});

describe("a chat in a project that runs every chat sandboxed (ADR 0067 §7, V78 a)", () => {
  const sandboxed = (sandbox: StartOptions["profiles"][number]["sandbox"]) =>
    options({ profiles: [{ ...ONE.profiles[0], sandbox }] });

  function showWith(sandbox: StartOptions["profiles"][number]["sandbox"], onInstall?: () => void) {
    const onStart = vi.fn();
    const onApprove = vi.fn();
    render(
      <StartChat
        options={sandboxed(sandbox)}
        onStart={onStart}
        onApprove={onApprove}
        onInstall={onInstall}
        onCancel={vi.fn()}
      />,
    );
    return { onStart, onApprove, user: userEvent.setup() };
  }

  it("says nothing of a sandbox the project has not turned on", () => {
    show();
    expect(screen.queryByRole("checkbox", { name: /without the sandbox/ })).not.toBeInTheDocument();
    expect(screen.queryByText(/sandbox/i)).not.toBeInTheDocument();
  });

  it("heads what it says about the sandbox, in every state of it", () => {
    for (const sandbox of [
      {
        state: "unsandboxed",
        said: "This system has no sandbox backend.",
        install: null,
        locked: null,
      },
      { state: "refused", said: "bwrap is not installed.", install: null, locked: null },
    ] as const) {
      showWith(sandbox);
      const region = screen.getByRole("region", { name: "Sandbox" });
      expect(region).toHaveTextContent(sandbox.said);
      cleanup();
    }
  });

  it("starts sandboxed unless the person ticks the box", async () => {
    const { onStart, user } = showWith({
      state: "sandboxed",
      said: "",
      install: null,
      locked: null,
    });

    expect(screen.getByRole("checkbox", { name: "Start without the sandbox" })).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, null);
  });

  it("starts this one chat without the sandbox, with the reason typed, and says so on the button", async () => {
    const { onStart, user } = showWith({
      state: "sandboxed",
      said: "",
      install: null,
      locked: null,
    });

    await user.click(screen.getByRole("checkbox", { name: "Start without the sandbox" }));
    await user.type(
      screen.getByRole("textbox", { name: "Why, if you want it recorded" }),
      "the build needs the network",
    );
    await user.click(screen.getByRole("button", { name: "Start without the sandbox" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, {
      reason: "the build needs the network",
    });
  });

  it("sends no reason when none was typed", async () => {
    const { onStart, user } = showWith({
      state: "sandboxed",
      said: "",
      install: null,
      locked: null,
    });

    await user.click(screen.getByRole("checkbox", { name: "Start without the sandbox" }));
    await user.click(screen.getByRole("button", { name: "Start without the sandbox" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, {
      reason: null,
    });
  });

  it("shows why the sandbox cannot be applied, and the one way on is said on the button", async () => {
    const why =
      "this project runs every chat sandboxed, and this machine cannot apply the sandbox: socat is not installed";
    const { onStart, user } = showWith({
      state: "refused",
      said: why,
      install: null,
      locked: null,
    });

    expect(screen.getByRole("alert")).toHaveTextContent(why);
    expect(screen.queryByRole("button", { name: "Start chat" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Start without the sandbox" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, {
      reason: null,
    });
  });

  it("shows the distribution's install command and types it in a shell tab only when asked", async () => {
    const onInstall = vi.fn();
    const { onStart, user } = showWith(
      {
        state: "refused",
        said: "socat is not installed",
        install: "sudo apt install socat",
        locked: null,
      },
      onInstall,
    );

    expect(screen.getByText("sudo apt install socat").tagName).toBe("CODE");
    await user.click(screen.getByRole("button", { name: "Type it in a shell tab" }));

    expect(onInstall).toHaveBeenCalledTimes(1);
    expect(onStart).not.toHaveBeenCalled();
  });

  it("says why a chat on a system with no backend starts without the sandbox", () => {
    const said =
      "This chat runs without the sandbox: purlis has no sandbox backend on Windows yet, so every chat here starts without it until one exists.";
    showWith({ state: "unsandboxed", said, install: null, locked: null });

    expect(screen.getByText(said)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start chat" })).toBeInTheDocument();
  });

  it("offers no opt-out where policy forbids it, and says who locked it (#1343)", async () => {
    const locked =
      "Policy forbids starting a chat without the sandbox. Locked by policy, set by Platform team in /etc/purlis/policy.json.";
    const { onStart, user } = showWith({ state: "sandboxed", said: "", install: null, locked });

    expect(
      screen.queryByRole("checkbox", { name: "Start without the sandbox" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Sandbox" })).toHaveTextContent(locked);
    await user.click(screen.getByRole("button", { name: "Start chat" }));
    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, null);
  });

  it("starts nothing where the sandbox cannot be applied and policy forbids the opt-out (#1343)", () => {
    const locked =
      "Policy forbids starting a chat without the sandbox. Locked by policy, set by Platform team in /etc/purlis/policy.json.";
    const { onStart } = showWith({
      state: "refused",
      said: "socat is not installed",
      install: null,
      locked,
    });

    expect(
      screen.queryByRole("button", { name: "Start without the sandbox" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start chat" })).toBeDisabled();
    expect(screen.getByRole("region", { name: "Sandbox" })).toHaveTextContent(locked);
    expect(onStart).not.toHaveBeenCalled();
  });

  it("starts nothing on a system with no backend where policy requires the sandbox, and names no opt-out (#1423)", () => {
    const said =
      "purlis has no sandbox backend on Windows yet, and policy requires the sandbox for every chat on this machine, so nothing was started.";
    const locked =
      "Policy forbids starting a chat without the sandbox. Locked by policy, set by Platform team in /etc/purlis/policy.json.";
    const { onStart } = showWith({ state: "refused", said, install: null, locked });

    const sandbox = screen.getByRole("region", { name: "Sandbox" });
    expect(sandbox).toHaveTextContent(said);
    // Who to ask, once: the policy and its owner.
    expect(sandbox).toHaveTextContent(locked);
    expect(sandbox.textContent?.split("Locked by policy")).toHaveLength(2);
    expect(sandbox).not.toHaveTextContent("new-chat picker");
    expect(screen.queryByRole("checkbox", { name: "Start without the sandbox" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Start without the sandbox" })).toBeNull();
    expect(screen.getByRole("button", { name: "Start chat" })).toBeDisabled();
    expect(onStart).not.toHaveBeenCalled();
  });

  it("carries the opt-out through the approval, and says so on its button", async () => {
    const onApprove = vi.fn();
    render(
      <StartChat
        options={options({
          profiles: [
            {
              ...ONE.profiles[0],
              approval: "new",
              sandbox: {
                state: "refused",
                said: "socat is not installed",
                install: null,
                locked: null,
              },
            },
          ],
        })}
        onStart={vi.fn()}
        onApprove={onApprove}
        onCancel={vi.fn()}
      />,
    );

    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Approve and start without the sandbox" }));

    expect(onApprove).toHaveBeenCalledWith("claude", "steward", false, "claude", null, false, {
      reason: null,
    });
  });
});

describe("a harness charter cannot wrap on this system in a sandboxed project", () => {
  it("shows Codex's refusal and starts it only without the sandbox", async () => {
    const said =
      "this project runs every chat sandboxed, and purlis runs Codex inside a sandbox of its own, which it can apply on macOS but not yet on Linux, so it was not started. Start this chat on a Claude Code profile.";
    const onStart = vi.fn();
    render(
      <StartChat
        options={options({
          profiles: [
            {
              ...ONE.profiles[0],
              name: "codex",
              kind: "codex",
              shown: "codex",
              sandbox: { state: "refused", said, install: null, locked: null },
            },
          ],
        })}
        onStart={onStart}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(said);
    expect(screen.queryByRole("button", { name: "Start chat" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Type it in a shell tab" }),
    ).not.toBeInTheDocument();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Start without the sandbox" }));

    expect(onStart).toHaveBeenCalledWith("codex", "steward", false, null, false, { reason: null });
  });
});

describe("a program the sandbox will not bind (V87g)", () => {
  it.each([
    [
      "a program where the chat can write",
      "this project runs every chat sandboxed, and the program lives where this chat can write: /work/acme/bin/claude, so it was not started sandboxed. Keep the program outside the project and outside what a chat may write.",
    ],
    [
      "a command naming a file where the chat can write",
      "this project runs every chat sandboxed, and this profile's command names /tmp/run.sh, which lies where this chat can write, so it was not started sandboxed. Keep every file the command names outside the project and outside what a chat may write.",
    ],
    [
      "a command word too long to check",
      "this project runs every chat sandboxed, and a word of this profile's command is longer than 4 KiB, which purlis does not check, so it was not started sandboxed. Keep what it says in a file outside the project and name that file instead.",
    ],
    [
      "a program named by a relative path",
      "this project runs every chat sandboxed, and this profile's program is a relative path, which would be found in a folder the chat can write, so it was not started sandboxed. Name the program by its full path.",
    ],
    [
      "a program that does not answer as Claude Code",
      "this project runs every chat sandboxed, and this profile's program does not answer as Claude Code, whose sandbox it was given, so it was not started sandboxed.",
    ],
  ])("shows %s, and starts it only without the sandbox", async (_, said) => {
    const onStart = vi.fn();
    render(
      <StartChat
        options={options({
          profiles: [
            {
              ...ONE.profiles[0],
              sandbox: { state: "refused", said, install: null, locked: null },
            },
          ],
        })}
        onStart={onStart}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(said);
    expect(screen.queryByRole("button", { name: "Start chat" })).not.toBeInTheDocument();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Start without the sandbox" }));

    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, { reason: null });
  });
});

describe("a profile nobody has approved, in a sandboxed project", () => {
  it("says its program is checked once it is approved, and still asks for the approval", () => {
    const said =
      "purlis checks this profile's program before it starts sandboxed, once the profile may start: approved, and declared in a file git does not carry.";
    render(
      <StartChat
        options={options({
          profiles: [
            {
              ...ONE.profiles[0],
              approval: "new",
              sandbox: { state: "sandboxed", said, install: null, locked: null },
            },
          ],
        })}
        onStart={vi.fn()}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByText(said)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Approve and start" })).toBeInTheDocument();
  });
});

describe("the Local file it names (#1719, #1340)", () => {
  const file = (which: "shared" | "local", name: string) => ({
    which,
    file: name,
    exists: true,
    text: "",
    refusals: [],
    parsed: true,
    fields: [],
    entries: [],
  });

  function inProject(over: Partial<StartOptions>, local: string | "unread") {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      if (cmd !== "project_settings") return null;
      if (local === "unread") throw new Error("no core here");
      return { shared: file("shared", "purlis.toml"), local: file("local", local) };
    });
    render(
      <ChatsHere.Provider value={{ ...fixedChats(), plane: "/projects/acme" }}>
        <StartChat
          options={options(over)}
          onStart={vi.fn()}
          onApprove={vi.fn()}
          onCancel={vi.fn()}
        />
      </ChatsHere.Provider>,
    );
    return { asked };
  }

  afterEach(clearMocks);

  it("names the file the core read where it says where to declare a profile", async () => {
    inProject({ declares_none: true }, "purlis.local.toml");

    const said = await screen.findByText(/This project declares no profiles of its own/);
    await waitFor(() => expect(said).toHaveTextContent("Declare your own in purlis.local.toml"));
    expect(said).not.toHaveTextContent("charter.local.toml");
  });

  it("names the file the core read where git would carry it", async () => {
    inProject(
      { declares_none: false, ignore_fix: "git rm --cached purlis.local.toml" },
      "purlis.local.toml",
    );

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/^git would carry purlis\.local\.toml, so every profile/);
  });

  it("says what the file is for when its name could not be read, and names no file it guessed", async () => {
    inProject({ declares_none: true }, "unread");

    const said = await screen.findByText(/Declare your own in the project's local settings file/);
    expect(said).not.toHaveTextContent(".toml");
  });

  it("asks nothing when no line names the file", () => {
    const { asked } = inProject({ declares_none: false }, "purlis.local.toml");

    expect(asked).not.toContain("project_settings");
  });
});

describe("the one-time note on a harness that lacks something (#1134, W10)", () => {
  const LINE = "Codex does not tell purlis when it is waiting for you.";
  const codex = {
    ...ONE.profiles[0],
    name: "codex",
    kind: "codex",
    shown: "codex",
    harness: {
      name: "codex",
      title: "Codex",
      label: "What Codex can do here",
      lines: [LINE],
      cannot_type: null,
    },
  };
  const NOTE = `You have not started a chat on Codex on this machine before. ${LINE}`;

  afterEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("says the card's line by Start, the first time, and not once a chat has started on it", async () => {
    const { user, onStart } = show({ profiles: [codex] });

    expect(screen.getByRole("note")).toHaveTextContent(NOTE);
    await user.click(screen.getByRole("button", { name: "Start chat" }));
    expect(onStart).toHaveBeenCalled();
    cleanup();

    show({ profiles: [codex] });
    expect(screen.queryByRole("note")).toBeNull();
  });

  it("is still said after a cancel: nothing started on it yet", async () => {
    const { user } = show({ profiles: [codex] });
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    cleanup();

    show({ profiles: [codex] });
    expect(screen.getByRole("note")).toHaveTextContent(NOTE);
  });

  it("is not said for a harness that lacks nothing", () => {
    show({ profiles: [{ ...codex, harness: { ...codex.harness, lines: [] } }] });

    expect(screen.queryByRole("note")).toBeNull();
  });

  it("is said again, and starts the chat, where this machine's storage cannot be read", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("no storage");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("no storage");
    });
    const { user, onStart } = show({ profiles: [codex] });

    expect(screen.getByRole("note")).toHaveTextContent(NOTE);
    await user.click(screen.getByRole("button", { name: "Start chat" }));
    expect(onStart).toHaveBeenCalled();
  });
});

describe("a profile its harness says is not signed in (#2035)", () => {
  const refused =
    "the profile 'work' is not signed in on this Mac. Nothing was started. It waits in the Inbox as Sign in work.";

  it("points to the Ask that signs it in, in the Inbox", async () => {
    const onShowSignIn = vi.fn();
    render(
      <StartChat
        options={ONE}
        trouble={refused}
        signIn="work"
        onShowSignIn={onShowSignIn}
        onStart={vi.fn()}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert").textContent).toBe(refused);
    expect(screen.getByText(/Sign in work waits in the Inbox/)).toBeTruthy();
    await userEvent.setup().click(screen.getByRole("button", { name: "Go to the Inbox" }));

    expect(onShowSignIn).toHaveBeenCalledOnce();
  });

  it("points nowhere for a refusal of anything else", () => {
    render(
      <StartChat
        options={ONE}
        trouble="the sandbox could not be applied. Nothing was started."
        onShowSignIn={vi.fn()}
        onStart={vi.fn()}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Go to the Inbox" })).toBeNull();
  });
});

describe("prototype B: the persona list beside the details (#2918)", () => {
  const MANY = [
    "steward",
    "devops",
    "domain-authority-master",
    "elastic-logs-master",
    "keycloak-master",
    "nestjs-developer",
    "security-reviewer",
    "ui-designer",
    "support-triage",
  ];
  const many = (over: Partial<StartOptions> = {}) => show({ personas: MANY, ...over });
  const filter = () => screen.getByRole("searchbox", { name: "Filter personas" });
  const named = (row: HTMLElement) => row.querySelector('[data-part="label"]')?.textContent;
  const chosen = () =>
    screen
      .getAllByRole("option")
      .filter((row) => row.getAttribute("aria-selected") === "true")
      .map(named);

  it("names where it starts and where its profiles come from (R8)", () => {
    render(
      <StartChat
        options={options()}
        place="smart-ide"
        onStart={vi.fn()}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("dialog")).toHaveAccessibleDescription(
      "In smart-ide · purlis's built-in profiles",
    );
  });

  it("filters a long list, its total in the placeholder (R11, R164)", () => {
    many();

    expect(filter()).toHaveAttribute("placeholder", `Filter ${MANY.length} personas`);
  });

  it("draws no filter over a short list", () => {
    show();

    expect(screen.queryByRole("searchbox")).toBeNull();
  });

  it("matches a case-insensitive part of the name, and hands the pick to the first match (R12, R13)", async () => {
    const { onStart, user } = many();

    await user.type(filter(), "MASTER");

    expect(screen.getAllByRole("option").map(named)).toEqual([
      "domain-authority-master",
      "elastic-logs-master",
      "keycloak-master",
    ]);
    expect(chosen()).toEqual(["domain-authority-master"]);
    await user.click(screen.getByRole("button", { name: "Start chat" }));
    expect(onStart).toHaveBeenCalledWith(
      "claude",
      "domain-authority-master",
      false,
      null,
      false,
      null,
    );
  });

  it("says so when no persona matches (R15)", async () => {
    const { user } = many();

    await user.type(filter(), "zzz");

    expect(screen.getByText("No persona is called “zzz”.")).toBeInTheDocument();
    // React Aria draws a list's empty state in an option of its own, which holds no persona.
    expect(screen.queryAllByRole("option").filter((row) => row.hasAttribute("data-key"))).toEqual(
      [],
    );
  });

  it("moves the pick with ↑ and ↓ from the filter, stopping at the ends (R16)", async () => {
    const { user } = many();
    await user.click(filter());

    await user.keyboard("{ArrowUp}");
    expect(chosen()).toEqual(["steward"]);
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(chosen()).toEqual(["domain-authority-master"]);
    await user.type(filter(), "ui-");
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(chosen()).toEqual(["ui-designer"]);
    expect(filter()).toHaveFocus();
  });

  it("starts on ⏎ in the filter (R17), but never approves a command on it", async () => {
    const { onStart, user } = many();
    await user.type(filter(), "keycloak{Enter}");
    expect(onStart).toHaveBeenCalledWith("claude", "keycloak-master", false, null, false, null);
    cleanup();

    const approving = many({
      profiles: [{ ...ONE.profiles[0], name: "claude", shown: "claude --x", approval: "new" }],
    });
    await approving.user.type(filter(), "keycloak{Enter}");
    expect(approving.onApprove).not.toHaveBeenCalled();
    expect(approving.onStart).not.toHaveBeenCalled();
  });

  it("chips the project's default persona (R20)", () => {
    many();

    const chip = (name: string) =>
      screen.getByRole("option", { name }).querySelector('[data-said="project default"]');
    expect(chip("steward")).not.toBeNull();
    expect(chip("devops")).toBeNull();
  });

  it("keeps the harness, the name and the footer box across a change of persona (R32)", async () => {
    const { onStart, user } = show({
      profiles: [ONE.profiles[0], { ...ONE.profiles[0], name: "work", is_default: false }],
    });

    await user.click(screen.getByRole("radio", { name: "work" }));
    await user.type(screen.getByRole("textbox", { name: "Name" }), "billing");
    await user.click(screen.getByRole("checkbox", { name: /purlis's footer/ }));
    await user.click(screen.getByRole("option", { name: "release" }));
    await user.click(screen.getByRole("button", { name: "Start chat" }));

    expect(onStart).toHaveBeenCalledWith("work", "release", true, "billing", false, null);
  });

  it("names the persona and the harness in the footer, with no icon beside an approval (R33, #2922)", () => {
    show();
    const summary = screen.getByTestId("start-chat-summary");
    // The icon is the harness's monogram here (`C`), drawn before its name.
    expect(summary).toHaveTextContent(/^steward on C?claude$/);
    expect(summary.querySelector(".harness-icon")).not.toBeNull();
    cleanup();

    show({
      profiles: [{ ...ONE.profiles[0], name: "claude", shown: "claude --x", approval: "new" }],
    });
    const asking = screen.getByTestId("start-chat-summary");
    expect(asking).toHaveTextContent("steward on claude");
    expect(asking.querySelector(".harness-icon")).toBeNull();
  });

  it("puts the persona's own placeholder in the Name field (R30)", async () => {
    const { user } = show();
    const name = screen.getByRole("textbox", { name: "Name" });

    expect(name.getAttribute("placeholder")).toMatch(/^steward\b/);
    await user.click(screen.getByRole("option", { name: "release" }));
    expect(name.getAttribute("placeholder")).toMatch(/^release\b/);
  });

  it("names the next chat's number in the placeholder, following the persona (R30)", async () => {
    const { user } = show({ next_number: 10 });
    const name = screen.getByRole("textbox", { name: "Name" });

    expect(name).toHaveAttribute("placeholder", "steward 10");
    await user.click(screen.getByRole("option", { name: "release" }));
    expect(name).toHaveAttribute("placeholder", "release 10");
    // With no persona, the chat is named after its harness.
    await user.click(screen.getByRole("option", { name: "No persona" }));
    expect(name).toHaveAttribute("placeholder", "claude 10");
  });

  it("says the number in words where the core sent none (an older answer)", () => {
    show();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveAttribute(
      "placeholder",
      "steward and its number",
    );
  });
});

describe("a pick in the persona list starts nothing; ⏎ on it does (R17)", () => {
  it("picks on a click, and starts on ⏎ from the list", async () => {
    const { onStart, user } = show();

    await user.click(screen.getByRole("option", { name: "release" }));
    expect(onStart).not.toHaveBeenCalled();
    expect(screen.getByRole("option", { name: "release" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    await user.keyboard("{ArrowUp}");
    expect(onStart).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, null, false, null);
  });

  it("starts on ⏎ on a harness card, and never from the Name field", async () => {
    const { onStart, user } = show();

    await user.type(screen.getByRole("textbox", { name: "Name" }), "x{Enter}");
    expect(onStart).not.toHaveBeenCalled();
    await user.click(screen.getByRole("radio", { name: "claude" }));
    await user.keyboard("{Enter}");
    expect(onStart).toHaveBeenCalledWith("claude", "steward", false, "x", false, null);
  });
});

describe("⏎ is a plain start and nothing more, from every place it starts (D-2918-3, #3378 B3)", () => {
  const plain = ONE.profiles[0];
  const states: [
    string,
    Partial<StartOptions>,
    ((user: ReturnType<typeof userEvent.setup>) => Promise<void>)?,
  ][] = [
    [
      "the profile asks for approval",
      { profiles: [{ ...plain, shown: "claude --x", approval: "new" }] },
    ],
    [
      "the person ticked Start without the sandbox",
      {
        profiles: [
          { ...plain, sandbox: { state: "sandboxed", said: "", install: null, locked: null } },
        ],
      },
      (user) => user.click(screen.getByRole("checkbox", { name: "Start without the sandbox" })),
    ],
    [
      "the sandbox is refused, so the start is the opt-out",
      {
        profiles: [
          {
            ...plain,
            sandbox: {
              state: "refused",
              said: "bwrap is not installed.",
              install: null,
              locked: null,
            },
          },
        ],
      },
    ],
    [
      "policy locks the sandbox it cannot apply",
      {
        profiles: [
          {
            ...plain,
            sandbox: {
              state: "refused",
              said: "bwrap is not installed.",
              install: null,
              locked: "Your administrator requires the sandbox.",
            },
          },
        ],
      },
    ],
  ];

  for (const [when, over, before] of states) {
    it(`starts nothing on ⏎ on a persona row when ${when}`, async () => {
      const { onStart, onApprove, user } = show(over);
      await before?.(user);
      await user.click(screen.getByRole("option", { name: "release" }));
      await user.keyboard("{Enter}");
      expect(onStart).not.toHaveBeenCalled();
      expect(onApprove).not.toHaveBeenCalled();
    });

    it(`starts nothing on ⏎ on a harness card when ${when}`, async () => {
      const { onStart, onApprove, user } = show(over);
      await before?.(user);
      await user.click(screen.getByRole("radio", { name: "claude" }));
      await user.keyboard("{Enter}");
      expect(onStart).not.toHaveBeenCalled();
      expect(onApprove).not.toHaveBeenCalled();
    });
  }

  it("starts once on a ⏎ let go of, and never on a held one's repeats", () => {
    const { onStart } = show();
    const row = screen.getByRole("option", { name: "release" });

    fireEvent.keyDown(row, { key: "Enter" });
    expect(onStart).not.toHaveBeenCalled();
    fireEvent.keyDown(row, { key: "Enter", repeat: true });
    fireEvent.keyDown(row, { key: "Enter", repeat: true });
    expect(onStart).not.toHaveBeenCalled();
    fireEvent.keyUp(row, { key: "Enter" });
    expect(onStart).toHaveBeenCalledTimes(1);
    // A repeat alone, its first press gone elsewhere, arms nothing.
    fireEvent.keyDown(row, { key: "Enter", repeat: true });
    fireEvent.keyUp(row, { key: "Enter" });
    expect(onStart).toHaveBeenCalledTimes(1);
  });

  it("starts on the row the keyboard is on, picked or not", () => {
    const { onStart } = show();
    const row = screen.getByRole("option", { name: "release" });

    fireEvent.keyDown(row, { key: "Enter" });
    fireEvent.keyUp(row, { key: "Enter" });

    expect(onStart).toHaveBeenCalledWith("claude", "release", false, null, false, null);
  });

  it("starts nothing on a double-click on a persona row", async () => {
    const { onStart, user } = show();

    await user.dblClick(screen.getByRole("option", { name: "release" }));

    expect(onStart).not.toHaveBeenCalled();
  });

  it("starts once on ⌘⏎ from the filter or a card, never twice", () => {
    const { onStart } = show({ personas: [...MANY_PERSONAS] });

    const filter = screen.getByRole("searchbox", { name: "Filter personas" });
    fireEvent.keyDown(filter, SEND);
    fireEvent.keyUp(filter, SEND);
    expect(onStart).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByRole("radio", { name: "claude" }), SEND);
    fireEvent.keyUp(screen.getByRole("radio", { name: "claude" }), SEND);
    expect(onStart).toHaveBeenCalledTimes(2);
  });

  it("starts nothing on Shift+⏎ or Alt+⏎ in the filter, on a composing ⏎, or with nothing matched", async () => {
    const { onStart, user } = show({ personas: [...MANY_PERSONAS] });
    const filter = screen.getByRole("searchbox", { name: "Filter personas" });

    for (const key of [
      { key: "Enter", shiftKey: true },
      { key: "Enter", altKey: true },
      { key: "Enter", keyCode: 229 },
      { key: "Enter", isComposing: true },
    ]) {
      fireEvent.keyDown(filter, key);
      fireEvent.keyUp(filter, { key: "Enter" });
    }
    await user.type(filter, "zzz{Enter}");

    expect(onStart).not.toHaveBeenCalled();
  });

  it("keeps the caret where it was as the list narrows (R168)", async () => {
    const { user } = show({ personas: [...MANY_PERSONAS] });
    const filter = screen.getByRole<HTMLInputElement>("searchbox", { name: "Filter personas" });

    await user.type(filter, "mastr");
    filter.setSelectionRange(4, 4);
    await user.type(filter, "e", { initialSelectionStart: 4, initialSelectionEnd: 4 });

    expect(filter).toHaveValue("master");
    expect(filter).toHaveFocus();
    expect(filter.selectionStart).toBe(5);
  });

  it("names the row ↑ and ↓ picked on the filter, for a screen reader", async () => {
    const { user } = show({ personas: [...MANY_PERSONAS] });
    const filter = screen.getByRole("searchbox", { name: "Filter personas" });

    await user.click(filter);
    await user.keyboard("{ArrowDown}");

    const picked = screen.getByRole("option", { name: "devops" });
    expect(filter).toHaveAttribute("aria-activedescendant", picked.id);
  });

  it("says a refusal at the top of the details, before the harness (#3378 B2)", () => {
    render(
      <StartChat
        options={options()}
        trouble="claude is not signed in on this device."
        onStart={vi.fn()}
        onApprove={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    const alert = screen.getByRole("alert");
    const harness = screen.getByRole("radiogroup", { name: "Harness" });
    expect(alert.compareDocumentPosition(harness) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByRole("region", { name: "steward" })).toContainElement(alert);
  });

  it("labels the Name box with a label a click focuses", async () => {
    const { user } = show();

    await user.click(screen.getByText("Name", { selector: "label" }));

    expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
  });
});

const MANY_PERSONAS = [
  "steward",
  "devops",
  "domain-authority-master",
  "elastic-logs-master",
  "keycloak-master",
  "nestjs-developer",
  "security-reviewer",
  "ui-designer",
];
