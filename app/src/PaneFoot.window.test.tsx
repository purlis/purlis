// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { Moved } from "./bindings";
import { onTheWire } from "./chatEvents.testkit";
import { CHAT_SKILLS_ASKED } from "./chatSkillsAsk";
import { forgetFootKept } from "./footKept";
import { aTabForEachChat } from "./laidOut.testkit";
import { askToEnd } from "./menus.testkit";

/**
 * **What a chat's pane foot says and what a press on it does, against the whole window** (#5204,
 * spec #5191 stories 33 to 41): `↳ from`, the skills warning and the sandbox moved here from the
 * head, and every part that can be pressed is a shortcut to a row of the chat's list.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" tabIndex={0}>
      session {session}
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  forgetFootKept();
  usage = USAGE;
  skillsOf = null;
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const USAGE = {
  context: { value: 62, tone: "ok" },
  cache: { value: 91, tone: "ok" },
  rebuilds: null,
  turns: [],
  cold: null,
};
let usage: unknown = USAGE;
/** What the core says of a chat's skills, with the Skill studio on, which the window asks once. */
let skillsOf: unknown = null;
/** How many times the core was asked for a gauge. */
let reads = 0;

const PARENT = {
  session: 3,
  name: "steward 3",
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: false,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: "steward 3",
  from: null,
  sandbox: "floor",
  sandbox_why: null,
  browser_sites: null,
  card: { name: "claude", title: "Claude Code", label: "What Claude Code can do here", lines: [] },
};

/** A task chat 3 asked for, in a tab of its own, on another harness. */
const TASK = {
  ...PARENT,
  session: 5,
  name: "fix-login",
  label: "fix-login",
  in_front: true,
  harness: "codex",
  profile: "codex",
  card: { name: "codex", title: "Codex", label: "What Codex can do here", lines: [] },
  from: {
    chat: 3,
    name: "steward 3",
    workspace: "alpha",
    task: true,
    tab: true,
    reported: false,
    unreported: false,
  },
};

/** A chat handed off from chat 3. */
const HANDED = {
  ...PARENT,
  session: 4,
  name: "claude 4",
  label: "work",
  in_front: true,
  from: {
    chat: 3,
    name: "steward 3",
    workspace: "alpha",
    task: false,
    tab: true,
    reported: false,
    unreported: false,
  },
};

function core(
  open: readonly (typeof PARENT | typeof HANDED | typeof TASK)[],
  stripOf: (session: number) => string = () => "alpha",
) {
  mockIPC(
    (cmd) => {
      if (cmd === "chat_usage") {
        reads++;
        return usage;
      }
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return open;
      if (cmd === "reopened_layout") return aTabForEachChat([...open], stripOf);
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [
            { name: "alpha", path: ALPHA, vision: "", todos: [], chats: open },
            { name: "beta", path: `${PLANE}/workspaces/beta`, vision: "", todos: [], chats: [] },
          ],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "asks_waiting") return { plane: PLANE, asks: [] };
      if (cmd === "mcp_failures") return [];
      if (cmd === "smart_closing") return [];
      if (cmd === "skill_drafts") return [];
      if (cmd === "skill_studio") return { on: false };
      if (cmd === "chat_skills") return skillsOf;
      if (cmd === "open_view") return { kind: "gone", why: "Not read in this test." };
      return null;
    },
    { shouldMockEvents: true },
  );
}

async function aPane(
  open: readonly (typeof PARENT | typeof HANDED | typeof TASK)[] = [PARENT, HANDED],
  stripOf?: (session: number) => string,
) {
  core(open, stripOf);
  render(<App />);
  await screen.findAllByTestId("pane");
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

const frames = () => [...document.querySelectorAll<HTMLElement>(".pane-frame")];
/** The pane in front, by what it shows: the handed-off chat's, unless it is alone. */
const frameOf = (session: number) =>
  frames().find((one) => one.textContent?.includes(`session ${session}`)) as HTMLElement;
const foot = (session = 4) =>
  frameOf(session).querySelector('[data-scope="pane-foot"][data-part="root"]') as HTMLElement;
/** The tab in front on the strip of tabs. */
const tabInFront = () =>
  document.querySelector('[data-strip="Tabs"] [role="tab"][aria-selected="true"]');
const head = (session = 4) => frameOf(session).querySelector(".pane-head") as HTMLElement;

describe("a chat's foot holds the rest of its facts (#5204)", () => {
  it("says harness · workspace, ↳ from and the sandbox, and the head says none of them", async () => {
    await aPane();

    await waitFor(() => expect(foot().querySelector(".pane-foot-from")).not.toBeNull());
    expect(foot().querySelector(".pane-foot-from")).toHaveTextContent("↳ from steward 3 · alpha");
    expect(foot().querySelector(".pane-foot-sandbox [data-sandbox]")).not.toBeNull();
    expect(head().textContent).not.toContain("↳ from");
    expect(head().querySelector("[data-sandbox]")).toBeNull();
  });

  it("is a group a screen reader hears naming the chat", async () => {
    await aPane();

    await waitFor(() => expect(foot().querySelector(".pane-where")).not.toBeNull());
    expect(foot()).toHaveAttribute("role", "group");
    expect(foot().getAttribute("aria-label")).toMatch(/work: where it runs and its context$/);
  });

  it("asks for nothing: every button on it opens something, and it holds no Notice", async () => {
    await aPane();

    await waitFor(() => expect(foot().querySelector(".pane-foot-from")).not.toBeNull());
    expect(foot().querySelector('[data-scope="notice"]')).toBeNull();
    expect(frameOf(4).querySelector(".pane-top")?.contains(foot())).toBe(false);
  });
});

describe("a press on a foot's part runs its row (#5204, stories 33 to 36)", () => {
  it("goes to the chat that started it, on ↳ from", async () => {
    await aPane();

    const from = await waitFor(() =>
      within(foot()).getByRole("button", { name: /↳ from steward 3/ }),
    );
    fireEvent.click(from);
    await waitFor(() => expect(tabInFront()).toHaveTextContent("steward 3"));
  });

  it("opens the harness's card, on the harness", async () => {
    await aPane();

    const harness = await waitFor(() =>
      within(foot()).getByRole("button", { name: "What Claude Code can do here" }),
    );
    fireEvent.click(harness);
    await waitFor(() => expect(tabInFront()).toHaveTextContent("What Claude Code can do here"));
  });

  it("opens the chat's Activity, on ctx", async () => {
    await aPane();

    const ctx = await waitFor(() => within(foot()).getByRole("button", { name: /^ctx 62%/ }));
    fireEvent.click(ctx);
    await waitFor(() => expect(tabInFront()).toHaveTextContent(/Activity/));
  });

  it("says its workspace in words where that workspace is the one focused: no press would go anywhere", async () => {
    await aPane();

    await waitFor(() => expect(foot().querySelector(".pane-where")).toHaveTextContent("alpha"));
    expect(within(foot()).queryByRole("button", { name: /^Focus workspace/ })).toBeNull();
  });

  it("focuses the workspace a chat is filed in, from a tab of another", async () => {
    await aPane([HANDED], () => "beta");

    const workspace = await waitFor(() =>
      within(foot()).getByRole("button", { name: /^Focus workspace alpha/ }),
    );
    expect(workspace).toHaveTextContent("alpha");
    fireEvent.click(workspace);
    // The tab was on beta's strip, which is no longer the one in front.
    await waitFor(() => expect(screen.queryByText("session 4")).toBeNull());
  });
});

describe("the foot's keyboard order (#5204, story 38)", () => {
  it("takes the foot's parts after the terminal, in the order they are drawn", async () => {
    await aPane();
    await waitFor(() => within(foot()).getByRole("button", { name: /^ctx 62%/ }));

    const user = userEvent.setup();
    screen
      .getAllByTestId("pane")
      .find((one) => one.textContent === "session 4")
      ?.focus();
    const heard: string[] = [];
    for (let at = 0; at < 3; at++) {
      await user.tab();
      heard.push(
        document.activeElement?.getAttribute("aria-label") ??
          document.activeElement?.textContent ??
          "",
      );
    }
    expect(heard[0]).toBe("What Claude Code can do here");
    expect(heard[1]).toMatch(/↳ from steward 3/);
    expect(heard[2]).toMatch(/^ctx 62%/);
  });
});

describe("a task's foot says the task (#5205, stories 21 and 22)", () => {
  it("says the task's harness and gauge, and ↳ from the chat that asked, with the path on the head", async () => {
    await aPane([PARENT, TASK]);

    await waitFor(() => expect(foot(5).querySelector(".pane-foot-from")).not.toBeNull());
    expect(head(5).querySelector(".pane-crumbs")).not.toBeNull();
    expect(foot(5).querySelector(".pane-where")).toHaveTextContent("Codex");
    expect(foot(5).querySelector(".pane-foot-from")).toHaveTextContent("↳ from steward 3");
    await within(foot(5)).findByTestId("chat-gauge");
    expect(head(5).querySelector(".chat-gauge")).toBeNull();
  });

  it("goes to the chat that asked, on ↳ from", async () => {
    await aPane([PARENT, TASK]);

    fireEvent.click(
      await waitFor(() => within(foot(5)).getByRole("button", { name: /↳ from steward 3/ })),
    );
    await waitFor(() => expect(tabInFront()).toHaveTextContent("steward 3"));
  });
});

describe("an ended chat's foot (#5205, stories 23 and 24)", () => {
  /** What the board says as chat 4 ends. */
  const DONE = {
    plane: PLANE,
    session: 4,
    state: "done",
    needs_you: false,
    queue: [],
    moved_at: 3,
    reports: [],
    refusals: [],
    children: [],
    sequence: 3,
  } as unknown as Moved;
  const ALONE = { ...HANDED, from: null };

  const gaugeFrozen = async () => {
    const gauge = await within(foot()).findByTestId("chat-gauge");
    expect(gauge.querySelector('[data-part="value"]')).toHaveAttribute("data-tone", "ended");
    expect(gauge.querySelector('[data-part="fill"]')).toHaveAttribute("data-tone", "ended");
    return gauge;
  };

  it("keeps its harness · workspace and its gauge as its program ends, greyed and marked ended, and reads no more", async () => {
    await aPane([ALONE]);
    await within(foot()).findByTestId("chat-gauge");

    await act(() => emit(...onTheWire("chat-moved", DONE)));

    await screen.findByRole("status", { name: /ended/i });
    const gauge = await gaugeFrozen();
    expect(foot().querySelector(".pane-foot-ended")).toHaveTextContent("ended");
    expect(foot().querySelector(".pane-where")).toHaveTextContent(/Claude Code\s*·\s*alpha/);
    expect(gauge).toHaveTextContent("ctx 62%");

    // Whatever the record says later, the gauge says what it said as the chat ended.
    usage = { ...USAGE, context: { value: 99, tone: "bad" } };
    const before = reads;
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(reads).toBe(before);
    expect(gauge).toHaveTextContent("ctx 62%");
  });

  it("keeps them after End…, though the core no longer lists the chat or answers for its usage", async () => {
    await aPane([ALONE]);
    await within(foot()).findByTestId("chat-gauge");
    usage = null;

    const asking = await askToEnd("work");
    fireEvent.click(within(asking).getByRole("button", { name: "Close" }));

    await screen.findByRole("status", { name: /ended/i });
    await gaugeFrozen();
    expect(foot().querySelector(".pane-where")).toHaveTextContent(/Claude Code\s*·\s*alpha/);
    expect(foot().querySelector(".pane-foot-ended")).toHaveTextContent("ended");
  });

  it("draws no gauge where none was known as it ended, and is still marked ended", async () => {
    usage = null;
    await aPane([ALONE]);

    await act(() => emit(...onTheWire("chat-moved", DONE)));

    await screen.findByRole("status", { name: /ended/i });
    expect(foot().querySelector(".pane-foot-ended")).toHaveTextContent("ended");
    expect(within(foot()).queryByTestId("chat-gauge")).toBeNull();
  });
});

describe("a narrow pane drops parts from its two lines, and has them back (#5206)", () => {
  /** How wide every pane is now, as the layout engine would say, and who to tell when it moves. */
  let wide = 900;
  const told: (() => void)[] = [];
  function panesOfAWidth(width: number) {
    wide = width;
    told.length = 0;
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(tell: (entries: unknown[], seen: unknown) => void) {
          told.push(() => tell([], this));
        }
        observe() {}
        disconnect() {}
        unobserve() {}
      },
    );
    const real = HTMLElement.prototype.getBoundingClientRect;
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      return this.classList.contains("pane-frame")
        ? ({ width: wide, height: 400, top: 0, left: 0, right: wide, bottom: 400 } as DOMRect)
        : real.call(this);
    });
  }
  const toWidth = (width: number) =>
    act(async () => {
      wide = width;
      for (const tell of told) tell();
    });
  const says = (part: string) => foot().querySelector(part) !== null;

  it("shrinks the skills warning to its ⚠, named all the same", async () => {
    skillsOf = {
      known: true,
      skills: [],
      none_because: null,
      routing: null,
      missing: null,
      changed: null,
      budget: {
        skills: 39,
        chars: 5900,
        manual: 0,
        most_skills: 30,
        most_chars: 6000,
        hidden: 0,
        still_over: null,
        pinned_by: [],
        pins_together: null,
        drops: null,
        twins: null,
        warns: "chars",
      },
    };
    panesOfAWidth(900);
    await aPane([{ ...HANDED, from: null }]);
    await waitFor(() =>
      expect(within(foot()).getByTestId("pane-skills")).toHaveTextContent("39 skills"),
    );

    await toWidth(449);
    const mark = within(foot()).getByTestId("pane-skills");
    expect(mark).not.toHaveTextContent("39 skills");
    expect(mark).toHaveAccessibleName(/^39 skills\./);

    await toWidth(900);
    expect(within(foot()).getByTestId("pane-skills")).toHaveTextContent("39 skills");
  });

  it("drops cache, then ↳ from, then the workspace, and brings each back as it widens", async () => {
    panesOfAWidth(900);
    await aPane();
    await waitFor(() => expect(says(".pane-foot-from")).toBe(true));
    await within(foot()).findByTestId("chat-gauge");
    await toWidth(900);
    expect(within(foot()).getByTestId("chat-gauge")).toHaveTextContent("cache");

    await toWidth(519);
    expect(within(foot()).getByTestId("chat-gauge")).not.toHaveTextContent("cache");
    expect(says(".pane-foot-from")).toBe(true);

    await toWidth(399);
    expect(says(".pane-foot-from")).toBe(false);
    expect(foot().querySelector(".pane-where")).toHaveTextContent(/alpha/);

    await toWidth(269);
    expect(foot().querySelector(".pane-where")).not.toHaveTextContent(/alpha/);
    // The harness and ctx never go.
    expect(foot().querySelector(".pane-where")).toHaveTextContent("Claude Code");
    expect(within(foot()).getByTestId("chat-gauge")).toHaveTextContent("ctx 62%");

    await toWidth(900);
    expect(foot().querySelector(".pane-where")).toHaveTextContent(/alpha/);
    expect(says(".pane-foot-from")).toBe(true);
    expect(within(foot()).getByTestId("chat-gauge")).toHaveTextContent("cache");
  });

  it("drops the head's words, its mark and Split down in the ruled order, from the same model", async () => {
    panesOfAWidth(900);
    await aPane();
    const buttons = () =>
      [...head().querySelectorAll(".pane-actions button")].map(
        (one) => one.getAttribute("aria-label") ?? "",
      );
    await waitFor(() => expect(buttons()).toContain("Split down"));

    await toWidth(379);
    expect(head().querySelector(".pane-state-mark")).toBeNull();
    expect(buttons()).toContain("Split down");
    await toWidth(299);
    expect(buttons()).not.toContain("Split down");

    await toWidth(900);
    expect(head().querySelector(".pane-state-mark")).not.toBeNull();
    expect(buttons()).toContain("Split down");
  });

  it("groups a task's head the same, with no Close, and drops its Split down too (#5203's window test)", async () => {
    panesOfAWidth(900);
    await aPane([PARENT, TASK]);
    const actions = () => head(5).querySelector(".pane-actions") as HTMLElement;
    const names = () =>
      [...actions().querySelectorAll("button")].map(
        (one) => one.getAttribute("aria-label") ?? one.textContent ?? "",
      );
    await waitFor(() => expect(names()).toContain("Split down"));
    expect(actions().querySelector('[data-scope="separator"]')).not.toBeNull();
    // The bell and ⋯, the divider, then the pane's own, with the task's minimise last.
    const at = names();
    expect(at.findIndex((one) => one.startsWith("More for"))).toBeLessThan(
      at.indexOf("Split right"),
    );

    await toWidth(299);
    expect(names()).not.toContain("Split down");
    expect(names()).toContain("Split right");
  });
});

describe("the foot's skills warning and studio chip (#5204, #5205, #5206)", () => {
  const PAST_ITS_BUDGET = {
    known: true,
    skills: [],
    none_because: null,
    routing: null,
    missing: null,
    changed: null,
    budget: {
      skills: 39,
      chars: 5900,
      manual: 0,
      most_skills: 30,
      most_chars: 6000,
      hidden: 0,
      still_over: null,
      pinned_by: [],
      pins_together: null,
      drops: null,
      twins: null,
      warns: "chars",
    },
  };

  it("runs the chat's Skills row on a press: the sidebar's Skills view, on this chat", async () => {
    skillsOf = PAST_ITS_BUDGET;
    await aPane([{ ...HANDED, from: null }]);

    const warning = await waitFor(() => within(foot()).getByTestId("pane-skills"));
    expect(warning).toHaveTextContent("39 skills");
    expect(warning).toHaveAccessibleName(/Near the skill budget/);
    // Its words for the press are its row's, as every part of the foot's are (#5366).
    // A verb, as a press says what it does (#5543, `docs/ui-copy.md`).
    expect(warning).toHaveAccessibleName(/\. Show skills of [^.]+\.$/);
    const asked: unknown[] = [];
    const hear = (event: Event) => asked.push((event as CustomEvent).detail);
    window.addEventListener(CHAT_SKILLS_ASKED, hear);
    fireEvent.click(warning);
    window.removeEventListener(CHAT_SKILLS_ASKED, hear);
    expect(asked).toEqual([{ plane: PLANE, session: 4 }]);
  });

  it("says a task's own skills on its foot", async () => {
    skillsOf = PAST_ITS_BUDGET;
    await aPane([PARENT, TASK]);

    await waitFor(() => within(foot(5)).getByTestId("pane-skills"));
  });

  it("opens what the Skill studio started it for, on the studio chip", async () => {
    await aPane([
      {
        ...HANDED,
        from: null,
        studio: {
          skill: "test-design",
          level: { at: "project", local: false },
          job: "draft",
          done: false,
        },
      } as unknown as typeof HANDED,
    ]);

    const chip = await waitFor(() => within(foot()).getByTestId("pane-studio"));
    expect(chip.tagName).toBe("BUTTON");
    fireEvent.click(chip);
    await waitFor(() => expect(tabInFront()).toHaveTextContent(/Skill studio/));
  });
});

describe("what an ended chat's foot keeps, when its pane is drawn anew (#5205, story 26)", () => {
  it("keeps its gauge and harness · workspace after its tab goes behind and comes back", async () => {
    const DONE = {
      plane: PLANE,
      session: 4,
      state: "done",
      needs_you: false,
      queue: [],
      moved_at: 3,
      reports: [],
      refusals: [],
      children: [],
      sequence: 3,
    } as unknown as Moved;
    await aPane([{ ...HANDED, from: null }, PARENT]);
    await within(foot()).findByTestId("chat-gauge");
    await act(() => emit(...onTheWire("chat-moved", DONE)));
    await screen.findByRole("status", { name: /ended/i });
    usage = null;

    const tabs = within(document.querySelector('[data-strip="Tabs"]') as HTMLElement);
    fireEvent.click(tabs.getByRole("tab", { name: /steward 3/ }));
    await waitFor(() => expect(frameOf(4)).toBeUndefined());
    fireEvent.click(tabs.getByRole("tab", { name: /work/ }));

    const gauge = await waitFor(() => within(foot()).getByTestId("chat-gauge"));
    expect(gauge).toHaveTextContent("ctx 62%");
    expect(foot().querySelector(".pane-where")).toHaveTextContent(/Claude Code\s*·\s*alpha/);
  });
});
