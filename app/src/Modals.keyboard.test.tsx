// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ENDS_IT } from "./actions";
import type { PlaneId } from "./bindings";
import { AboutCharter } from "./About";
import { DeleteWorkspace } from "./DeleteWorkspace";
import { NewProject } from "./NewProject";
import { NewVault } from "./NewVault";
import { NewWorkspace } from "./NewWorkspace";
import { ApproveExtension } from "./ApproveExtension";
import { ApprovePlane } from "./ApprovePlane";
import { EndingChat } from "./EndingChat";
import { Extensions } from "./Extensions";
import { Health } from "./Doctor";
import { Palette } from "./Palette";
import { QuitWarning } from "./QuitWarning";
import { RelaunchAsk } from "./RelaunchAsk";
import { StartChat } from "./StartChat";
import { PinItem, UpdateItem } from "./Updates";
import { said } from "./a11y.testkit";
import { pressTab, type TabTakenBy } from "./ui/dialog/dialog.testkit";

/**
 * **What a keyboard can reach in each of the window's modal surfaces — one file, because the
 * answer was decided once for the window** (charter-app#186).
 *
 * When this file was written the window's modals were Radix's, two mechanisms decided where Tab
 * went inside one, and the defect lived in the gap between them.
 *
 * - **Radix's `FocusScope` handled the two EDGES of the scope and nothing else**
 *   (`@radix-ui/react-focus-scope`, `handleKeyDown`): on the first tabbable it acted on
 *   Shift+Tab and called `focus(last)` itself, on the last it acted on Tab and called
 *   `focus(first)`. In between it did not touch the event.
 * - **In between, the engine decides — and the engine is WebKit**, a WKWebView on macOS and
 *   WebKitGTK on Linux, because charter embeds the system WebView. WebKit leaves a form
 *   control out of the tab sequence unless macOS's full keyboard access is on. That is not a
 *   quirk of one runner: it is `HTMLFormControlElement::isKeyboardFocusable`, and it was
 *   measured in the real window in charter-app#176 before it was read in the source.
 *
 * So a `<button>` that was neither edge was reachable by neither, and every control in these
 * dialogs was a `<button>` — including Radix's checkbox and its radio rows, which rendered
 * `Primitive.button` rather than an `<input>`.
 *
 * **The first fix was the engine's own escape hatch and not a handler of ours.** WebKit's rule, in
 * full, since r263447 (`[popover] Improve focus handling`, 2023-04-29, shipped in Safari 17
 * and WebKitGTK 2.42):
 *
 * ```cpp
 * bool HTMLFormControlElement::isKeyboardFocusable(const FocusEventData& focusEventData) const
 * {
 *     if (!!tabIndexSetExplicitly())
 *         return Element::isKeyboardFocusable(focusEventData);
 *     return isFocusable() && document().frame()
 *         && document().frame()->eventHandler().tabsToAllFormControls(focusEventData);
 * }
 * ```
 *
 * **A `tabindex` that is written down is not consulted against full keyboard access at all.**
 * That is why Radix's radio rows were always reachable — its roving focus wrote one on them —
 * and why nothing else was. Every button in a modal has said `tabIndex={0}` since (the button
 * family's, `ui/pressable.tsx`), which is the platform's answer rather than charter's.
 * `docs/ui-primitives.md` holds the reasoning.
 *
 * **What decides it now is the dialog family itself** (#2910): it hears Tab on the window before
 * anything else does and moves the keyboard to the next stop in reading order, round from either
 * end (`useTabbing` in `ui/dialog/Dialog.tsx`, `ui/dialog/tabOrder.ts`), so every stop is reached
 * whatever the engine's sequence. The palette keeps Tab for itself, and React Aria's
 * `FocusScope` (`contain`) moves it there, round as well. **So the walks below test those two and
 * nothing else** (#6069): each press must be taken by the surface, and one that is left to the
 * engine fails the walk. Until #6069 a press left over went to a model of WebKit's sequence
 * (`inWebKitsTabSequence`, `tabSequence.ts`); no walk had reached it since the family took Tab,
 * so it was dropped here. The rule still has readers where the engine's sequence is what moves
 * the keyboard outside a dialog: leaving a terminal and the window's regions (`moveAlong`), and
 * their tests (`Window.keyboard.test.tsx`, `PanelHalves.window.test.tsx`).
 *
 * **Why this is measured here and not in a scenario — which was tried, in a real window, and
 * cannot be done.** The obvious objection to this whole file is that jsdom is not WebKit, so a
 * spec was written to walk the picker with `browser.keys(["Tab"])` and read
 * `document.activeElement` — an unmodified key and a DOM read, neither of which runs into what
 * charter-app#176 measured. It reported that Tab reached nothing at all, including the radio
 * rows, which carried an explicit `tabindex` from Radix's roving focus long before this change
 * and were never in doubt. That was the tell. Two plain text `<input>`s injected
 * into the running app settled it: **Tab does not move the focus between them either**, while
 * the keydown arrives unprevented. This driver dispatches a synthetic DOM event and performs no
 * default action of any kind, which is one fact wearing the three faces #176 caught it in.
 * `docs/ui-primitives.md` has the trace.
 *
 * So a scenario can neither confirm nor refute reachability, and the half it CAN prove is kept
 * in `picker.e2e.ts`: that the attribute the engine's rule needs survives the build and is on
 * the element in the shipped app, which is the one thing jsdom cannot see. That attribute is
 * now the second line of defence: the family's own Tab, walked here, is the first.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

/** Who takes Tab in every surface here but the palette. */
const FAMILY: TabTakenBy = "the dialog family";

/** Which way a walk goes, and who must take each of its presses. */
type Walk = { shift?: boolean; by?: TabTakenBy };

/**
 * One Tab press, as the surface hears it, failing unless `by` took it (`pressTab`, #6069).
 *
 * The key is dispatched for real, so what moves the keyboard is the surface's own code and not a
 * model of it, and a press left to the engine fails the walk rather than being guessed at. Which
 * of the dialog family and React Aria's focus scope took it is told apart there, because React
 * Aria's dialog wraps round a scope of its own too: a walk that only watched the focus would
 * pass with the family's Tab gone.
 */
async function tab({ shift = false, by = FAMILY }: Walk = {}) {
  pressTab(shift, by);
  await act(async () => {});
}

/**
 * Every control this surface's keyboard can reach, starting from wherever it opened.
 *
 * Tab until the focus comes round to something already seen, which is what a real operator
 * does to find out what is there. A control the keyboard cannot get to is simply missing from
 * the answer, and the assertion says which.
 */
async function reachableByKeyboard({ shift = false, by = FAMILY }: Walk = {}): Promise<string[]> {
  const at = document.activeElement as HTMLElement;
  // **The elements, and the words only at the end** (charter-app#197). This used to remember
  // where it had been by what each control is CALLED, and that is a false positive waiting for
  // a dialog with two controls of the same name: `NewProject` grew a second directory picker,
  // its second `Browse…` read as a place already visited, the walk stopped there, and the
  // three controls after it were reported unreachable when every one of them was fine. A guard
  // that says a working dialog is broken is worse than no guard, because the obvious repair is
  // to weaken the expectation until it passes.
  const visited = [at];
  for (let press = 0; press < 30; press += 1) {
    await tab({ shift, by });
    const now = document.activeElement as HTMLElement | null;
    if (!now || now === document.body) break;
    // Round to a stop already seen, the one it started on included (a surface with one stop).
    // The family's Tab lands on a group's chosen item at once (`entryOf` in
    // `ui/dialog/tabOrder.ts`), so a press never lands short of a stop and needs a second.
    if (visited.includes(now)) break;
    visited.push(now);
  }
  return visited.map(said);
}

const PROFILES = [
  {
    name: "claude",
    kind: "claude" as const,
    shown: "claude",
    source: "built-in",
    is_default: true,
    ready_to_type: true,
    harness: null,
    sandbox: null,
    approval: null,
  },
];

function picker() {
  render(
    <StartChat
      options={{
        profiles: PROFILES,
        refused: [["broken", "declares no command"]],
        // Two, the second picked: the list's one stop is the persona picked, not its first row.
        personas: ["builder", "steward"],
        persona: "steward",
        persona_profiles: {},
        ignore_fix: null,
        ignore_fix_id: null,
        declares_none: false,
      }}
      onStart={() => {}}
      onApprove={() => {}}
      onCancel={() => {}}
    />,
  );
}

describe("what a keyboard reaches in the window's modal surfaces", () => {
  it("reaches Start in the picker, which ADR 0022 makes the only way a chat begins", async () => {
    // **The defect charter-app#186 was filed for, and the measurement is narrower than the
    // ticket's wording — which is the reason to take it.** The picker's controls are a harness
    // radio group, a persona radio group, a footer checkbox, the refused list's `<summary>`,
    // then `Cancel` and `Start`. The radio groups carried a `tabindex` from Radix's roving focus
    // and a `<summary>` is not a form control, so those were in WebKit's sequence; the
    // checkbox and both answers, all three `<button>`s with nothing written down, were not.
    //
    // Walked before the attribute went on, this dialog answered: **Tab from `Cancel` moved
    // nowhere at all** — there is nothing after it in the engine's sequence and `Cancel` is
    // not an edge, so neither mechanism had anything to say — while **Shift+Tab eventually
    // reached `Start`**, five presses backwards through the whole form and out the far side by
    // Radix's first edge, as the dialogs were then. So the ticket's `Start` was reachable in
    // the strict sense and unreachable in every sense that matters: not by the key an operator
    // presses, and only by walking a dialog backwards. The footer checkbox (ADR 0029's one
    // choice) was reachable by neither. What this test pins is the plain thing: **Tab,
    // forwards, reaches all seven**, the Name field (charter-app#254) among them. The dialog
    // ends in its footer (#2910), `Cancel` then `Start`, so Tab from the focused `Cancel` is
    // `Start`, then round to the top of the form and down it in reading order. The dialog
    // moves the keyboard itself (`ui/dialog/tabOrder.ts`), so every stop is reached whatever
    // the engine's own sequence.
    picker();

    expect(await reachableByKeyboard()).toEqual([
      'button "Cancel"',
      'button "Start chat"',
      // The persona list, left of the details (#2918): one stop, the persona picked.
      'option "steward"',
      'radio "claude"',
      // The Name field (charter-app#254): an `<input>`, which every engine puts in the sequence.
      'input "Name"',
      'checkbox "Draw purlis\'s footer in this chat"',
      'summary "1 refused"',
    ]);
  });

  it("reaches all seven the other way too, which is the half that half-worked before", async () => {
    // Shift+Tab was the only direction that went anywhere in this dialog, and it went most of
    // the way: everything but the footer checkbox, by the engine's sequence as far as the
    // first edge and by Radix's `focus(last)` after that. Asserted because "the fix did not
    // cost the direction that used to work" is not something the forward test can say, and
    // because the wrap round the first edge is the dialog family's own (`ui/dialog/tabOrder.ts`)
    // rather than the engine's.
    picker();

    expect(await reachableByKeyboard({ shift: true })).toEqual([
      'button "Cancel"',
      'summary "1 refused"',
      'checkbox "Draw purlis\'s footer in this chat"',
      'input "Name"',
      'radio "claude"',
      'option "steward"',
      'button "Start chat"',
    ]);
  });

  it("reaches the approve-and-start answer too, which is the same button under another name", async () => {
    // A profile whose command line charter has not seen draws `Approve and start` where
    // `Start` was. It is a different element, so it is worth one assertion of its own rather
    // than an assumption that the branch above covers it.
    render(
      <StartChat
        options={{
          profiles: [{ ...PROFILES[0], approval: "new", shown: "claude --dangerous" }],
          refused: [],
          personas: [],
          persona: null,
          persona_profiles: {},
          ignore_fix: null,
          ignore_fix_id: null,
          declares_none: false,
        }}
        onStart={() => {}}
        onApprove={() => {}}
        onCancel={() => {}}
      />,
    );

    expect(await reachableByKeyboard()).toEqual([
      'button "Cancel"',
      'button "Approve and start"',
      'option "No persona"',
      'radio "claude"',
      'input "Name"',
      'checkbox "Draw purlis\'s footer in this chat"',
    ]);
  });

  it("starts a chat from the keyboard alone, which is the whole of what #186 is about", async () => {
    // The end of the route, not a step of it: reach `Start` by Tab and press it. jsdom is the
    // only place the press can be tested at all — a scenario's synthesised `Enter` does not
    // activate a focused button (charter-app#176) — so the reach and the press are asserted
    // together here rather than split across two rigs that each hold half a claim.
    const started: unknown[] = [];
    render(
      <StartChat
        options={{
          profiles: PROFILES,
          refused: [],
          personas: [],
          persona: null,
          persona_profiles: {},
          ignore_fix: null,
          ignore_fix_id: null,
          declares_none: false,
        }}
        onStart={(profile, persona, footer) => started.push({ profile, persona, footer })}
        onApprove={() => {}}
        onCancel={() => {}}
      />,
    );

    // Cancel is focused on opening and `Start` stands just after it in the footer (#2910) —
    // one Tab, and it is the press the engine used to refuse.
    await tab();
    expect(screen.getByRole("button", { name: "Start chat" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");

    expect(started).toEqual([{ profile: "claude", persona: null, footer: false }]);
  });

  it("reaches the Runs on choice in the picker and picks from it by the keyboard alone (#4536)", async () => {
    // Where the chat runs is a choice of this start, so it is reached as every other one is:
    // by Tab, opened by Enter, moved by the arrows and picked by Enter, and Start carries it.
    const started: unknown[] = [];
    render(
      <StartChat
        options={{
          profiles: PROFILES,
          refused: [],
          personas: [],
          persona: null,
          persona_profiles: {},
          ignore_fix: null,
          ignore_fix_id: null,
          declares_none: false,
        }}
        runsOn={{
          pool: "builds",
          level: "project",
          at_root: false,
          pools: ["builds"],
          ran_on: ["builds"],
        }}
        onStart={(...args) => started.push(args[6])}
        onApprove={() => {}}
        onCancel={() => {}}
      />,
    );

    expect(await reachableByKeyboard()).toEqual([
      'button "Cancel"',
      'button "Start chat"',
      'option "No persona"',
      'radio "claude"',
      'button "As set (pool builds) Runs on"',
      'input "Name"',
      'checkbox "Draw purlis\'s footer in this chat"',
    ]);

    const runsOn = screen.getByRole("button", { name: /Runs on$/ });
    act(() => runsOn.focus());
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("option", { name: "This machine" });
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await waitFor(() => expect(runsOn).toHaveTextContent("This machine"));
    expect(started).toEqual([]);

    act(() => screen.getByRole("button", { name: "Start chat" }).focus());
    await userEvent.keyboard("{Enter}");
    expect(started).toEqual([{ pool: null }]);
  });

  it("reaches every row's Review and Remove in the extensions dialog", async () => {
    // **The worst of them, and the one nobody had looked at.** `Add an extension…` is the
    // scope's first edge and `Done` is its last, which left every installed row's two buttons
    // in the middle. Reviewing or removing an extension was a mouse-only act on the one
    // surface in the window that exists to be a consent decision (ADR 0041).
    mockIPC((cmd) => {
      if (cmd === "installed_extensions")
        return {
          extensions: [
            {
              id: "solarized",
              name: "Solarized",
              path: "/home/dev/ext/solarized",
              standing: "trusted",
              refused: null,
              themes_in_force: [],
              theme_trouble: [],
              ask: {
                id: "solarized",
                name: "Solarized",
                path: "/home/dev/ext/solarized",
                declares: [],
                fingerprint: "a".repeat(64),
                first: false,
                runs_as_you: "runs as you do",
                fingerprint_note: "fingerprinted",
                state_note: null,
              },
            },
          ],
          built_in_themes: ["purlis-dark"],
          dropped: [],
          unreadable: null,
        };
      if (cmd === "theme_in_force") return { theme: null, complaints: [], follows_system: false };
      return null;
    });
    render(<Extensions onClose={() => {}} />);
    await screen.findByRole("button", { name: "Review" });

    expect(await reachableByKeyboard()).toEqual([
      'button "Add an extension…"',
      'button "Review"',
      'button "Remove"',
      'button "Done"',
    ]);
  });

  it("reaches Check again in the doctor, past the summary that was hiding it", async () => {
    // Three tabbables whenever a row is unchecked — the `<summary>`, then the answer bar's
    // `Close` and `Check again` (#1210) — and `Check again` was once the one in the middle. The `<summary>` is what makes this dialog
    // a good witness: it is not a form control, so the engine always stopped at it, and the
    // hole was on the far side of it.
    const rows = [
      {
        name: "git",
        status: "ok" as const,
        detail: "found",
        hint: "install git",
        checked: true,
        settings: null,
        fix: null,
      },
      {
        name: "tmux",
        status: "warn" as const,
        detail: "not checked (not ported)",
        hint: "a later build checks this",
        checked: false,
        settings: null,
        fix: null,
      },
    ];
    render(
      <Health
        doctor={{
          running: false,
          run: () => {},
          report: { rows, app_rows: [], full: false, path: "/usr/bin:/bin" },
        }}
      />,
    );
    await userEvent.click(screen.getByTestId("status-doctor"));
    await screen.findByRole("dialog");

    expect(await reachableByKeyboard()).toEqual([
      'summary "Not checked by this build (1)"',
      'button "Close"',
      'button "Check again"',
    ]);
  });

  it("reaches a doctor row's way into Settings", async () => {
    // SE-22: a row whose fix is a setting carries a button into its Settings group.
    render(
      <Health
        doctor={{
          running: false,
          run: () => {},
          report: {
            rows: [
              {
                name: "charter.toml",
                status: "warn",
                detail: "plane.mod in charter.toml is not read",
                hint: "Fix or remove it",
                checked: true,
                settings: "project.saving",
                fix: null,
              },
            ],
            app_rows: [],
            full: false,
            path: "/usr/bin:/bin",
          },
        }}
        onOpenSettings={() => {}}
      />,
    );
    await userEvent.click(screen.getByTestId("status-doctor"));
    await screen.findByRole("dialog");

    expect(await reachableByKeyboard()).toEqual([
      'button "Fix it in Settings"',
      'button "Close"',
      'button "Check again"',
    ]);
  });

  it("reaches Install in the update offer, which was the one act that ends every chat", async () => {
    // The channel radios are the scope's first edge and `Close` is its last, so `Install` and
    // `Check now` sat between them. Installing is the act that ends every running chat
    // (`INSTALL_ENDS_SESSIONS`), and it could only be reached with a mouse.
    render(
      <UpdateItem
        updates={{
          state: {
            kind: "offered",
            offer: { version: "0.2.0", current: "0.1.0", channel: "stable", notes: "Fixes." },
          },
          channel: "stable",
          check: () => {},
          install: () => {},
          choose: () => {},
          restart: () => {},
        }}
      />,
    );
    await userEvent.click(screen.getByTestId("status-update"));
    await screen.findByRole("dialog");

    expect(await reachableByKeyboard()).toEqual([
      'radio "stable"',
      'button "Close"',
      'button "Check now"',
      'button "Install 0.2.0"',
    ]);
  });

  it("reaches Restart to update, and both answers of the ask about a chat mid-turn", async () => {
    // The restart ends every chat, so it is the same kind of act Install was — and the ask in
    // front of it has the safe answer first.
    render(
      <UpdateItem
        updates={{
          state: { kind: "installed", version: "0.2.0" },
          channel: "stable",
          check: () => {},
          install: () => {},
          choose: () => {},
          restart: () => {},
        }}
        chats={[{ key: "a/1", name: "ide.1", harness: "claude", cwd: null, state: "running" }]}
      />,
    );
    await userEvent.click(screen.getByTestId("status-update"));
    await screen.findByRole("dialog");

    // What to put back after the restart is answered here, one stop like the channel (#1078).
    expect(await reachableByKeyboard()).toEqual([
      'radio "stable"',
      'radio "Reopen every chat"',
      'button "Close"',
      'button "Restart to update"',
    ]);

    await userEvent.click(screen.getByRole("button", { name: "Restart to update" }));
    await screen.findByRole("alertdialog");
    await waitFor(() => expect(screen.getByRole("button", { name: "Wait" })).toHaveFocus());

    expect(await reachableByKeyboard()).toEqual(['button "Wait"', 'button "Restart now"']);
  });

  it("reaches both answers of every two-answer dialog, forwards", async () => {
    // These five were already whole when the dialogs were Radix's, and for a reason that was
    // never about them: their two answers WERE the two edges Radix handled, so Shift+Tab from
    // the first was Radix's own `focus()` call. That made "the keyboard works here" a property
    // of the NUMBER of buttons, which a third control would have taken away in silence — and
    // it left plain Tab, the key an operator actually presses, doing nothing at all. Both are
    // why the attribute went on a dialog that did not appear to need it. The dialog family's
    // own Tab, through every stop, is what holds them now.
    const two: [string, () => void, string[]][] = [
      [
        "the quit warning",
        () =>
          void render(
            <QuitWarning
              chats={[
                {
                  key: "a",
                  name: "session 1",
                  harness: "claude",
                  cwd: "/home/dev/plane",
                  state: "running",
                },
              ]}
              onQuit={() => {}}
              onCancel={() => {}}
            />,
          ),
        ['button "Cancel"', 'button "Quit purlis"'],
      ],
      [
        "the first-open prompt",
        () =>
          void render(
            <ApprovePlane
              ask={{
                path: "/home/dev/plane",
                first: true,
                changes: [],
                contributes: {
                  plugins: [],
                  env: [],
                  starts: [],
                  profiles: [],
                  grants: [],
                  fonts: [],
                },
              }}
              onApprove={() => {}}
              onCancel={() => {}}
            />,
          ),
        ['button "Cancel"', 'button "Open project"'],
      ],
      [
        "the extension prompt",
        () =>
          void render(
            <ApproveExtension
              ask={{
                id: "solarized",
                name: "Solarized",
                path: "/home/dev/ext/solarized",
                declares: ["a theme"],
                fingerprint: "a".repeat(64),
                first: true,
                runs_as_you: "runs as you do",
                fingerprint_note: "fingerprinted",
                state_note: null,
              }}
              onApprove={() => {}}
              onCancel={() => {}}
            />,
          ),
        ['button "Cancel"', 'button "Trust it"'],
      ],
      [
        "the question before a chat ends",
        () =>
          void render(
            <EndingChat
              offer={{
                id: "chat.end:1",
                title: "End chat steward 1…",
                available: true,
                reason: "",
                does: { verb: "endChat", session: 1 },
                note: ENDS_IT,
              }}
              smart={{ available: true, why: null, close_first: false }}
              onEnd={() => {}}
              onSmartClose={() => {}}
              onCancel={() => {}}
            />,
          ),
        // From the default the operator ruled (ADR 0064): Smart close, on a chat with turns.
        ['button "Smart close"', 'button "Cancel"', 'button "Close"'],
      ],
      [
        "the question a relaunch asks",
        () =>
          void render(
            <RelaunchAsk
              question={{
                projects: [{ plane: "/home/dev/plane", chats: 2, views: 0 }],
                after_update: false,
              }}
              nameOf={(plane) => plane}
              onAnswer={() => {}}
            />,
          ),
        ['button "Reopen all sessions"', 'button "Start fresh"'],
      ],
      [
        "the question a relaunch asks",
        () =>
          void render(
            <RelaunchAsk
              question={{
                projects: [{ plane: "/home/dev/plane", chats: 2, views: 0 }],
                after_update: false,
              }}
              nameOf={(plane) => plane}
              onAnswer={() => {}}
            />,
          ),
        ['button "Reopen all sessions"', 'button "Start fresh"'],
      ],
    ];
    for (const [what, show, answers] of two) {
      show();
      expect(await reachableByKeyboard(), what).toEqual(answers);
      cleanup();
    }
  });

  it("reaches the one control of every surface that has one", async () => {
    // A surface with a single tabbable is its own first and last stop, so the dialog family's
    // Tab, round from the last stop to the first, keeps it reachable whatever the engine does,
    // as Radix's edges did before it. They are here because the claim this file makes is
    // about the window and not about the dialogs that happened to be interesting: a list that
    // leaves surfaces out is a list the next person has to re-derive.

    render(
      <PinItem
        pin={{
          drift: true,
          pinned: "0.1.0",
          brought: "0.2.0",
          said: ["the plane pins an older charter"],
        }}
        again={() => {}}
      />,
    );
    await userEvent.click(screen.getByTestId("status-pin"));
    await screen.findByRole("dialog");
    expect(await reachableByKeyboard()).toEqual(['button "Close"']);
    cleanup();

    // **About charter**, which arrived with the title bar. It is here on the day it was
    // written rather than after somebody noticed, which is the whole argument this file makes:
    // the hole is not a mistake anybody made, it is what a modal in a WebView does by default.
    // Its command is not mocked and it does not need to be — a dialog that could not read the
    // changelog draws the refusal. Since FR-14 (#609) it is no longer a one-control surface:
    // the three help links are drawn whatever the core answered, and each is reachable. The
    // refusal has Read again beside it (#1296).
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    await screen.findByRole("dialog");
    await screen.findByRole("button", { name: "Read again" });
    // The licences (FM-3) are one link, to their tab (#1719). The walk starts on
    // Close, where the dialog puts the keyboard (#1719). Copy diagnostics (#4774) is the
    // footer's other button, beside Close.
    expect(await reachableByKeyboard()).toEqual([
      'button "Close"',
      'button "Copy diagnostics"',
      'button "Read again"',
      'a "Discussions"',
      'a "report a bug"',
      'a "how to get help"',
      'a "their licences"',
    ]);
  });

  it("reaches the three surfaces that arrived while this was being measured", async () => {
    // **#172 landed the workspace lifecycle between the survey and this branch, and every one
    // of its dialogs had the defect.** That is the argument for a file like this one rather
    // than a fix per dialog: the hole is not a mistake anybody made, it is what a modal in a
    // WebView does by default, so a new surface has it on the day it is written.
    //
    // `NewProject` is the sharpest of the three. Only its folder box was in the engine's
    // sequence at all — a text `<input>` — and it is the scope's first edge, with `Cancel` the
    // last. `Browse…`, the checkbox that decides whether charter writes into a repository the
    // operator already has, and `Create project` were all in between, reachable by nothing.
    //
    // **And it grew a control after the sweep, which is the second half of the argument**
    // (charter-app#197, ADR 0035's adopt default). A second directory picker arrived with no
    // `tabIndex={0}` on its `Browse…`, because the sweep was over by the time it was written —
    // so the class of defect came back on the one dialog that had just been fixed for it. It
    // is the case for a walk that enumerates rather than a rule somebody has to remember.
    //
    // The two `Browse…` buttons carry `aria-label`s naming the box each one fills, which is
    // what stops a screen reader announcing the same three words for two different pickers —
    // and it is also why this walk can tell them apart in its answer.
    //
    // FR-4 put the repository first and the two-directory form under Advanced, a `<details>`
    // whose summary is in the walk; opened, its controls are too. T1-24 put the Protection
    // choice between the repo and Advanced.
    render(
      <NewProject
        making={false}
        onCreate={() => {}}
        onOpenRepo={() => {}}
        opening={false}
        onCancel={() => {}}
      />,
    );
    // Answered first: both dialogs disable their create button until they have been, and a
    // disabled control is out of the tab sequence everywhere and rightly so. The state worth
    // measuring is the one where the answer can be given.
    await userEvent.type(screen.getByLabelText("Repo"), "/where/the/repo/is");
    await userEvent.click(screen.getByText("Advanced"));
    await userEvent.type(screen.getByLabelText("Folder"), "/where/it/goes");
    // The walk starts where the keyboard is, the folder box, and comes round to it.
    // One footer for both of its forms (#2910): with Advanced open, its main action is Create
    // project.
    expect(await reachableByKeyboard()).toEqual([
      'input "Folder"',
      'button "Browse for the folder"',
      'input "Repo to adopt"',
      'button "Browse for the repo to adopt"',
      'checkbox "Make this repo itself the project"',
      'button "Cancel"',
      'button "Create project"',
      'input "Repo"',
      'button "Browse for the repo"',
      // T1-24's Protection row: a radio group is one stop, its chosen radio, and arrows move
      // within it.
      'radio "Basic sandbox (recommended)"',
      'summary "Advanced"',
    ]);
    cleanup();

    // Two text boxes were reachable and the button that acts on them was not.
    // Its repo picker asks the forge as it opens (ADR 0055), answered here with one repo.
    mockIPC((cmd) => {
      if (cmd === "reachable_repos")
        return { repos: [{ name: "svc", path: "acme/svc", description: "" }], trouble: [] };
      return null;
    });
    render(
      <NewWorkspace
        plane="plane"
        planeId={"p1" as PlaneId}
        making={false}
        onCreate={() => {}}
        onCancel={() => {}}
      />,
    );
    await userEvent.type(screen.getByLabelText("Name"), "svc");
    await screen.findByRole("checkbox", { name: "svc" });
    expect(await reachableByKeyboard()).toEqual([
      'input "Name"',
      'textarea "What it is for (optional)"',
      'input "Filter repos"',
      'button "Refresh"',
      'checkbox "svc"',
      'checkbox "Live"',
      'button "Cancel"',
      'button "Create workspace"',
    ]);
    cleanup();

    // Two answers, so the edges covered it — in one direction, and only while there are two.
    render(
      <DeleteWorkspace
        workspace="svc"
        atRisk={[]}
        deleting={false}
        onDelete={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(await reachableByKeyboard()).toEqual(['button "Cancel"', 'button "Delete workspace"']);
  });

  it("reaches every answer of New vault, its provider rows as one stop", async () => {
    // DS-3c (#1175) moved this dialog onto the settings set; the walk is the proof that the
    // move kept its keyboard order. The provider list is a radio group, one Tab stop whose
    // rows are reached with the arrows, and 1Password's own set-up (#1527) joins the walk
    // when it is the chosen one: how it signs in (one stop), the token's box and its button.
    render(
      <NewVault
        plane="/home/dev/plane"
        making={false}
        onCreate={() => {}}
        onMade={() => {}}
        onCancel={() => {}}
      />,
    );
    await userEvent.type(screen.getByLabelText("Name"), "ops");
    await userEvent.click(screen.getByRole("radio", { name: "1Password" }));
    await userEvent.click(screen.getByLabelText("Service-account token"));
    expect(await reachableByKeyboard()).toEqual([
      'input "Service-account token"',
      'button "Use this token"',
      'button "Cancel"',
      'input "Name"',
      'radio "1Password"',
      'radio "A service-account token"',
    ]);
  });

  it("keeps Tab on the palette's box, its one stop, by React Aria's focus scope", async () => {
    // The palette has an anatomy of its own, so the dialog family's Tab stands back from it
    // (`own` in `useTabbing`): the key reaches the box, and a focus scope of React Aria's takes
    // it there and keeps the keyboard on the box rather than letting it out to the window. That
    // scope is the one React Aria's `useDialog` asks its `Overlay` for, and `NotModal` draws no
    // other (#6093).
    // It was once the control case for WebKit's own sequence — a text `<input>` is in it
    // whatever full keyboard access says — and the walk no longer consults the engine (#6069).
    render(<Palette offers={[]} onRun={() => ({ ok: true }) as const} />);
    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog");

    expect(await reachableByKeyboard({ by: "the focus scope" })).toEqual([
      'combobox "Run an action"',
    ]);
  });

  it("keeps the keyboard inside the surface when there is nowhere further to go", async () => {
    // The trap, asserted once rather than relied on by every walk above. `reachableByKeyboard`
    // reads a repeat as "it has come round", and a focus that escaped to the document would
    // read the same way — so this is what separates the two: Tab off the last stop (`Start`,
    // last in the footer, #2910) lands on the first, by the dialog's own move, and never on
    // `<body>`.
    picker();
    const start = screen.getByRole("button", { name: "Start chat" });
    await act(async () => start.focus());

    await tab();

    expect(document.activeElement).not.toBe(document.body);
    await waitFor(() => expect(screen.getByRole("option", { name: "steward" })).toHaveFocus());
  });
});
