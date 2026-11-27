// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The prerequisite modal** (#7719; ADR 0130 §4, §8): the project's open policy prerequisite,
 * raised as an alert of the dialog family, with its words by state, its answers and their focus.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  PREREQUISITES_CHANGED,
  type PolicyPrerequisite,
  type PolicyState,
  type Prerequisites,
} from "./bindings";
import { SETTLE_MS } from "./noticeSettle";
import { WATCH } from "./projectPolicy.fixture";
import { forgetPolicies } from "./projectPolicy";
import {
  ACCEPT_IN_THE_WINDOW,
  AFTER_NEW_KEY_WHY,
  BROKEN_SUMMARY,
  BROKEN_WHY,
  KEPT_WHY,
  KEYCHAIN_REFUSED,
  PrerequisiteDialog,
  SHIPPED_WHY,
  INERT_CAP,
  acceptedSaid,
  forgetPrerequisiteAnswers,
  inertWords,
} from "./PrerequisiteDialog";
import { answersOf } from "./ui/dialog/dialog.testkit";
import { RAISED_ALERT_CHANGED } from "./ui/dialog/RaisedAlert";

const PLANE = "/home/dev/smart-ide";
const TITLE = "smart-ide's policy needs your answer";

/** The settle guard's clock, held: each test says when time passes. */
let clock = 10_000;
const passes = (ms: number) => {
  clock += ms;
};

beforeEach(() => {
  clock = 10_000;
  vi.spyOn(performance, "now").mockImplementation(() => clock);
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetPolicies();
  forgetPrerequisiteAnswers();
  fireEvent.blur(window);
  vi.restoreAllMocks();
});

/** Two families the committed file loosens, as the core lists what Accept would put in force. */
const LOOSER: PolicyState = {
  ...WATCH,
  digest: "d1",
  waiting: [
    { family: "guards.git", accepted: "enforce", committed: "log" },
    { family: "guards.merge", accepted: "log", committed: "off" },
  ],
};

/** An open policy prerequisite: Waiting on `LOOSER`, an acceptance kept, unless told otherwise. */
function entry(over: Partial<PolicyPrerequisite> = {}): Prerequisites["list"][number] {
  return {
    kind: "policy",
    state: "open",
    reasons: ["waiting"],
    digest: "d1",
    needs_new_key: false,
    after_new_key: false,
    nothing_kept: false,
    policy: LOOSER,
    ...over,
  };
}

const listOf = (...list: Prerequisites["list"]): Prerequisites => ({ plane: PLANE, list });

/**
 * The core as the modal asks it: `list` is what `project_prerequisites` answers, `accept` what a
 * press answers (a throw is its refusal). Answers each command asked, with its arguments.
 */
function core(
  list: () => Prerequisites,
  accept: (cmd: string) => unknown = () => ({ ...WATCH, digest: "d1" }),
) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "project_prerequisites") return list();
      if (cmd === "accept_project_policy" || cmd === "accept_project_policy_with_new_key")
        return accept(cmd);
      return null;
    },
    { shouldMockEvents: true },
  );
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

/** A line whose whole text is `text`, names isolated in it or not. */
const saying = (text: string) => (_: string, element: Element | null) =>
  element?.tagName === "P" && element.textContent === text;

const answer = (name: string) => screen.getByRole("button", { name });

/** Waits for the alert to open with the keyboard on its answer that grants nothing. */
async function opened(cancel = "Keep my current rules", name = TITLE) {
  const alert = await screen.findByRole("alertdialog", { name });
  await waitFor(() => expect(answer(cancel)).toHaveFocus());
  return alert;
}

describe("the prerequisite modal", () => {
  it("raises an open policy prerequisite with its title, why-line, summary and link, the keyboard on the answer that grants nothing, and Return accepts nothing", async () => {
    const asked = core(() => listOf(entry()));
    render(<PrerequisiteDialog plane={PLANE} canAccept onOpenPolicy={() => {}} />);
    const alert = await opened();
    expect(alert).toHaveAccessibleDescription(KEPT_WHY);
    expect(answersOf(alert)).toEqual(["Keep my current rules", "Accept"]);
    const rows = within(alert).getByRole("list", { name: "What Accept puts in force" });
    expect(
      within(rows)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual([
      "guards.git from enforce to log: what it would refuse runs, and is logged.",
      "guards.merge from log to off: what it would refuse runs, and is not logged.",
    ]);
    expect(alert).toHaveTextContent("Accept loosens 2 rules:");
    expect(within(alert).getByRole("button", { name: "Open the policy in Settings" })).toBeTruthy();
    passes(SETTLE_MS);
    // Return from the alert itself, or its summary, never finds Accept (`returnFinds`).
    const body = within(alert).getByRole("list", { name: "What Accept puts in force" });
    body.tabIndex = -1;
    for (const from of [alert, body]) {
      from.focus();
      await userEvent.keyboard("{Enter}");
    }
    expect(asked("accept_project_policy")).toEqual([]);
  });

  it("reads Work in strict mode under Broken, with everything Accept puts in force", async () => {
    core(() => listOf(entry({ reasons: ["broken"] })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened("Work in strict mode");
    expect(alert).toHaveAccessibleDescription(BROKEN_WHY);
    expect(alert).toHaveTextContent(BROKEN_SUMMARY);
    expect(alert).not.toHaveTextContent("Accept loosens");
  });

  it("reads Work in strict mode where the policy is Broken and Waiting both", async () => {
    core(() => listOf(entry({ reasons: ["broken", "waiting"] })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    await opened("Work in strict mode");
    expect(screen.queryByRole("button", { name: "Keep my current rules" })).toBeNull();
  });

  it("says each why-line by state: the shipped default where nothing was kept, the new key only after one", async () => {
    core(() => listOf(entry({ nothing_kept: true })));
    const first = render(<PrerequisiteDialog plane={PLANE} canAccept />);
    expect(await opened()).toHaveAccessibleDescription(SHIPPED_WHY);
    first.unmount();

    core(() => listOf(entry({ reasons: ["broken"], after_new_key: true })));
    const second = render(<PrerequisiteDialog plane={PLANE} canAccept />);
    expect(await opened("Work in strict mode")).toHaveAccessibleDescription(
      `${AFTER_NEW_KEY_WHY} ${BROKEN_WHY}`,
    );
    second.unmount();

    core(() => listOf(entry({ reasons: ["broken"], digest: "d2" })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened("Work in strict mode");
    expect(alert).toHaveAccessibleDescription(BROKEN_WHY);
    expect(alert).not.toHaveTextContent(AFTER_NEW_KEY_WHY);
  });

  it("chooses the answer that grants nothing on Esc, and does not raise it again for the same digest", async () => {
    const asked = core(() => listOf(entry()));
    const first = render(<PrerequisiteDialog plane={PLANE} canAccept />);
    await opened();
    passes(SETTLE_MS);
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(asked("accept_project_policy")).toEqual([]);
    first.unmount();

    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    await waitFor(() => expect(asked("project_prerequisites")).toHaveLength(2));
    await act(async () => {});
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("never raises an entry the core already reads as deferred", async () => {
    const asked = core(() => listOf(entry({ state: "deferred" })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    await waitFor(() => expect(asked("project_prerequisites")).toHaveLength(1));
    await act(async () => {});
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("redraws a policy change while it is open with the new entry's summary, and Accept names the new digest", async () => {
    const asked = core(() => listOf(entry()));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened();
    passes(SETTLE_MS);
    const moved: PolicyState = {
      ...LOOSER,
      digest: "d2",
      waiting: [{ family: "network", accepted: "enforce", committed: "log" }],
    };
    await act(() => emit(PREREQUISITES_CHANGED, listOf(entry({ digest: "d2", policy: moved }))));
    await waitFor(() => expect(alert).toHaveTextContent("network from enforce to log"));
    expect(alert).not.toHaveTextContent("guards.git");
    expect(alert).toHaveTextContent("Accept loosens 1 rule:");
    passes(SETTLE_MS);
    await userEvent.click(answer("Accept"));
    await waitFor(() =>
      expect(asked("accept_project_policy")).toEqual([{ plane: PLANE, shown: "d2" }]),
    );
  });

  it("accepts the shown digest on its own plane, closes, gives the keyboard back and says Policy accepted", async () => {
    const asked = core(() => listOf(entry()));
    const before = document.createElement("button");
    before.textContent = "Where the keyboard was";
    document.body.append(before);
    before.focus();
    try {
      render(<PrerequisiteDialog plane={PLANE} canAccept />);
      await opened();
      passes(SETTLE_MS);
      await userEvent.click(answer("Accept"));
      await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
      expect(asked("accept_project_policy")).toEqual([{ plane: PLANE, shown: "d1" }]);
      await waitFor(() => expect(before).toHaveFocus());
      expect(screen.getByText(saying(acceptedSaid("smart-ide")))).toBeTruthy();
    } finally {
      before.remove();
    }
  });

  it("accepts nothing where the digest moved, says so and stays up", async () => {
    let digest = "d1";
    core(
      () => listOf(entry({ digest })),
      () => {
        digest = "d2";
        throw "the project's policy changed since it was shown, so nothing was accepted; look at it again";
      },
    );
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened();
    passes(SETTLE_MS);
    await userEvent.click(answer("Accept"));
    await waitFor(() =>
      expect(within(alert).getByRole("alert")).toHaveTextContent(
        "the project's policy changed since it was shown, so nothing was accepted",
      ),
    );
    expect(screen.getByRole("alertdialog", { name: TITLE })).toBeTruthy();
    expect(screen.queryByText(saying(acceptedSaid("smart-ide")))).toBeNull();
  });

  it("reads Make a new key and accept under a foreign key; a Keychain refusal stays up with its sentence, and a second press works", async () => {
    let refuse = true;
    const asked = core(
      () => listOf(entry({ needs_new_key: true, reasons: ["broken"] })),
      () => {
        if (refuse) {
          refuse = false;
          throw "purlis could not keep your acceptance on this machine: the Keychain refused";
        }
        return { ...WATCH, digest: "d1" };
      },
    );
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened("Work in strict mode");
    expect(answersOf(alert)).toEqual(["Work in strict mode", "Make a new key and accept"]);
    expect(alert).not.toHaveTextContent(/update|earlier build|another program/i);
    passes(SETTLE_MS);
    await userEvent.click(answer("Make a new key and accept"));
    await waitFor(() =>
      expect(within(alert).getByRole("alert")).toHaveTextContent(KEYCHAIN_REFUSED),
    );
    expect(asked("accept_project_policy")).toEqual([]);
    await userEvent.click(answer("Make a new key and accept"));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(asked("accept_project_policy_with_new_key")).toHaveLength(2);
  });

  it("shows no Accept on a linked page and says where to accept", async () => {
    core(() => listOf(entry()));
    render(<PrerequisiteDialog plane={PLANE} canAccept={false} />);
    const alert = await opened();
    expect(answersOf(alert)).toEqual(["Keep my current rules"]);
    expect(alert).toHaveTextContent(ACCEPT_IN_THE_WINDOW);
  });

  it("closes unanswered from the Settings link, opens the policy, and answers nothing", async () => {
    const asked = core(() => listOf(entry()));
    const openPolicy = vi.fn();
    const first = render(<PrerequisiteDialog plane={PLANE} canAccept onOpenPolicy={openPolicy} />);
    await opened();
    await userEvent.click(answer("Open the policy in Settings"));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await waitFor(() => expect(openPolicy).toHaveBeenCalledTimes(1));
    expect(asked("accept_project_policy")).toEqual([]);
    first.unmount();
    // Unanswered: the prerequisite is still open, and a window drawn again raises it.
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    await opened();
  });

  it("closes when another window's answer empties the list (the event, not a poll)", async () => {
    const asked = core(() => listOf(entry()));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    await opened();
    await act(() => emit(PREREQUISITES_CHANGED, listOf()));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(asked("project_prerequisites")).toHaveLength(1);
  });

  it("hears no other project's list", async () => {
    core(() => listOf(entry()));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    await opened();
    await act(() => emit(PREREQUISITES_CHANGED, { plane: "/home/dev/other", list: [] }));
    expect(screen.getByRole("alertdialog", { name: TITLE })).toBeTruthy();
  });

  it("draws free text from the file stripped of control and bidi characters, in code font", async () => {
    const tricky: PolicyState = {
      ...LOOSER,
      waiting: [
        {
          family: "dispatch.denies st\u202Eeward\u0007 -> dev\u2066ops",
          accepted: "denied",
          committed: "lifted",
        },
      ],
      locked: ["guards.git\u200F"],
    };
    core(() => listOf(entry({ policy: tricky })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened();
    const row = within(alert).getByRole("listitem");
    const code = row.querySelector("code");
    expect(code?.textContent).toBe("dispatch.denies steward -> devops");
    for (const hidden of ["\u202E", "\u0007", "\u2066"])
      expect(row.textContent).not.toContain(hidden);
    expect(alert).toHaveTextContent("guards.git stays enforced whatever you accept.");
    expect(alert.textContent).not.toContain("\u200F");
  });

  it("shows two prerequisites one after another, each saying where it stands, never stacked", async () => {
    core(() => listOf(entry(), entry({ digest: "d9", reasons: ["broken"] })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const first = await opened();
    expect(screen.getAllByRole("alertdialog")).toHaveLength(1);
    expect(first).toHaveTextContent("1 of 2");
    passes(SETTLE_MS);
    await userEvent.keyboard("{Escape}");
    const second = await opened("Work in strict mode");
    expect(screen.getAllByRole("alertdialog")).toHaveLength(1);
    expect(second).toHaveTextContent("2 of 2");
  });

  describe("after an Accept", () => {
    /** Accepts d1, with the core's emptied list heard `before` or after the press returns. */
    async function acceptWithTheListEmptied(before: boolean) {
      let list = listOf(entry());
      core(
        () => list,
        async () => {
          list = listOf();
          if (before) await emit(PREREQUISITES_CHANGED, list);
          return { ...WATCH, digest: "d1" };
        },
      );
      render(<PrerequisiteDialog plane={PLANE} canAccept />);
      await opened();
      passes(SETTLE_MS);
      await userEvent.click(answer("Accept"));
      await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
      await waitFor(() => expect(screen.getByText(saying(acceptedSaid("smart-ide")))).toBeTruthy());
      if (!before) await act(() => emit(PREREQUISITES_CHANGED, listOf()));
    }

    for (const before of [true, false])
      it(`raises the same digest again when it reopens, the emptied list heard ${before ? "before" : "after"} the press returned`, async () => {
        await acceptWithTheListEmptied(before);
        await act(() =>
          emit(
            PREREQUISITES_CHANGED,
            listOf(entry(), entry({ digest: "d9", reasons: ["broken"] })),
          ),
        );
        expect(await opened()).toHaveTextContent("1 of 2");
      });

    it("says new chats follow it, and a running chat once it restarts (until #7729)", async () => {
      core(() => listOf(entry()));
      render(<PrerequisiteDialog plane={PLANE} canAccept />);
      await opened();
      passes(SETTLE_MS);
      await userEvent.click(answer("Accept"));
      const said = await screen.findByText(saying(acceptedSaid("smart-ide")));
      expect(said).toHaveTextContent(/New chats follow it/);
      expect(said).toHaveTextContent(/restart/);
      expect(said).not.toHaveTextContent("Guards follow it now");
    });
  });

  it("gives a new-key press's own error, not the Keychain's, where the key was made and the accept failed", async () => {
    let made = false;
    core(
      () => listOf(entry({ needs_new_key: !made, reasons: ["broken"] })),
      () => {
        made = true;
        throw "purlis could not keep your acceptance on this machine: the disk is full";
      },
    );
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened("Work in strict mode");
    passes(SETTLE_MS);
    await userEvent.click(answer("Make a new key and accept"));
    await waitFor(() =>
      expect(within(alert).getByRole("alert")).toHaveTextContent("the disk is full"),
    );
    expect(alert).not.toHaveTextContent(KEYCHAIN_REFUSED);
  });

  it("says what Accept changes for each kind of row", async () => {
    const kinds: PolicyState = {
      ...LOOSER,
      waiting: [
        { family: "guards.git", accepted: "enforce", committed: "log" },
        { family: "guards.git", accepted: "log", committed: "off" },
        { family: "merges through the forge route", accepted: "held", committed: "log" },
        { family: "sandbox", accepted: "full", committed: "floor" },
        { family: "sandbox", accepted: "floor", committed: "off" },
        { family: "dispatch.denies steward -> devops", accepted: "denied", committed: "lifted" },
        { family: "lineage through the forge route", accepted: "denied", committed: "allowed" },
        { family: "protected branch release/*", accepted: "protected", committed: "lifted" },
      ],
    };
    core(() => listOf(entry({ policy: kinds })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened();
    const said = within(alert)
      .getAllByRole("listitem")
      .map((one) => one.textContent?.split(": ")[1]);
    expect(said).toEqual([
      "what it would refuse runs, and is logged.",
      "what it would refuse runs, and is not logged.",
      "chats merge through the forge route.",
      "chats run with the floor only.",
      "chats run with no sandbox.",
      "that dispatch is no longer refused.",
      "a chat may push the branch of a task it dispatched once that task ends.",
      "chats may push to that branch through the forge route.",
    ]);
  });

  const sameDigest: [string, Partial<PolicyPrerequisite>][] = [
    ["nothing kept", { nothing_kept: true }],
    [
      "its rows",
      {
        policy: {
          ...LOOSER,
          waiting: [{ family: "network", accepted: "enforce", committed: "off" }],
        },
      },
    ],
  ];
  for (const [what, over] of sameDigest)
    it(`says it changed when ${what} changes under the same digest while it is up`, async () => {
      core(() => listOf(entry()));
      render(<PrerequisiteDialog plane={PLANE} canAccept />);
      const alert = await opened();
      passes(SETTLE_MS);
      await act(() => emit(PREREQUISITES_CHANGED, listOf(entry(over))));
      await waitFor(() => expect(alert).toHaveTextContent(RAISED_ALERT_CHANGED));
    });

  it("strips invisible letters and combining marks, keeps an accented name, and isolates each name", async () => {
    const tricky: PolicyState = {
      ...LOOSER,
      waiting: [
        {
          family: "guardsㅤ.gitᅟ⠀ﾠ́⃝",
          accepted: "enf̶orce",
          committed: "log",
        },
      ],
      locked: ["réseau"],
    };
    core(() => listOf(entry({ policy: tricky })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened();
    const row = within(alert).getByRole("listitem");
    expect(row).toHaveTextContent("guards.git from enforce to log");
    for (const hidden of ["ㅤ", "ᅟ", "⠀", "ﾠ", "́", "⃝", "̶"])
      expect(alert.textContent).not.toContain(hidden);
    expect(alert).toHaveTextContent("réseau stays enforced");
    expect([...row.querySelectorAll("bdi")].map((one) => one.textContent)).toEqual([
      "guards.git",
      "enforce",
      "log",
    ]);
  });

  it("keeps the vowel signs of other scripts, so names that differ in them stay apart", () => {
    const names = ["कमल", "कमला", "कोमल", "कमाल", "हिंदी", "தமிழ்", "বাংলা", "สมชัย"];
    const shown = names.map(inertWords);
    expect(shown).toEqual(names.map((name) => name.normalize("NFC")));
    expect(new Set(shown).size).toBe(names.length);
    // A pile of marks on one letter is cut to three.
    expect([...inertWords(`क${"ि".repeat(9)}`)]).toHaveLength(4);
  });

  it("cuts each name of a row on its own, so a long asking persona leaves the target in view", async () => {
    const long = "a".repeat(500);
    const policy: PolicyState = {
      ...LOOSER,
      waiting: [
        { family: `dispatch.denies ${long} -> devops`, accepted: "denied", committed: "lifted" },
      ],
    };
    core(() => listOf(entry({ policy })));
    render(<PrerequisiteDialog plane={PLANE} canAccept />);
    const alert = await opened();
    const code = within(alert).getByRole("listitem").querySelector("code");
    expect(code?.textContent).toBe(`dispatch.denies ${"a".repeat(INERT_CAP - 1)}… -> devops`);
  });

  it("cuts and isolates a long right-to-left project name in the title and the Notice", async () => {
    const plane = `/home/dev/${"ש".repeat(200)}`;
    const cut = `${"ש".repeat(INERT_CAP - 1)}…`;
    mockIPC(
      (cmd) => {
        if (cmd === "project_prerequisites") return { plane, list: [entry()] };
        if (cmd === "accept_project_policy") return { ...WATCH, digest: "d1" };
        return null;
      },
      { shouldMockEvents: true },
    );
    render(<PrerequisiteDialog plane={plane} canAccept />);
    const alert = await opened("Keep my current rules", `${cut}'s policy needs your answer`);
    expect(alert.querySelector("code bdi")?.textContent).toBe(cut);
    passes(SETTLE_MS);
    await userEvent.click(answer("Accept"));
    const said = await screen.findByText(saying(acceptedSaid(cut)));
    expect(said.querySelector("bdi")?.textContent).toBe(cut);
  });
});
