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
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { ChatSkill, ChatSkills, Moved } from "./bindings";
import { onAMac } from "./tabKeys";
import { theFrameNow } from "./frame.testkit";
import { onTheWire } from "./chatEvents.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetDismissals } from "./dismissals";

/**
 * **Use skill… follows its chat through a restart** (#6193). A restart is the same chat in a new
 * session, which takes the old one's place: the old session leaves the core's open chats, so a
 * pick still aimed at it was refused, and the dialog stayed up over the window. Restarted under
 * an open Use skill… (an owed restart lands once the turn ends, whatever is up), the dialog asks
 * the new session for its skills, and a pick types into the chat that runs now.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
  forgetDismissals();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 4,
  name: "claude 4",
  cwd: ALPHA,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  card: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

const SKILL: ChatSkill = {
  name: "deploy",
  source_label: null,
  level: "project",
  label: "project",
  description: "Deploys.",
  manual: false,
  takes_arguments: false,
  arguments: [],
  hint: null,
  hidden: null,
};

const SKILLS: ChatSkills = {
  on: true,
  known: true,
  program: [],
  plugins_off: [],
  own: null,
  skills: [SKILL],
  budget: null,
  repos: [],
  none_because: null,
  routing: null,
  missing: null,
};

/** Chat 4 mid-turn, then waiting for you: an owed restart runs once its turn ends. */
const RUNNING: Moved = {
  plane: PLANE,
  session: 4,
  state: "running",
  needs_you: false,
  queue: [],
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
  sequence: 1,
};
const WAITING: Moved = { ...RUNNING, state: "waiting", moved_at: 2, sequence: 2 };

/** The core: chat 4 owed a restart, which starts it again as session 9; the studio on. */
/** What the core refuses a pick with for a session no longer among its open chats (#6196). */
const NOT_RUNNING =
  "This chat isn't running now, so nothing was typed. Pick the skill again in the chat's tab.";

/** `held`, where given, is what the first `use_skill` waits for before the core refuses it. */
function core(held?: Promise<void>) {
  let picks = 0;
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "reopened_layout") return aTabForEachChat([CHAT], () => "alpha");
      if (cmd === "skill_studio_on") return true;
      if (cmd === "chat_skills") return SKILLS;
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT] }],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [4];
      if (cmd === "restart_chat")
        return { chat: { ...CHAT, session: 9, resumed: "c1" }, notices: [], not_yet: null };
      if (cmd === "use_skill") {
        picks += 1;
        if (held !== undefined && picks === 1)
          return held.then(() => Promise.reject(new Error(NOT_RUNNING)));
        return { kind: "typed", text: "/deploy" };
      }
      if (cmd === "about_charter") return new Promise(() => {});
      return null;
    },
    { shouldMockEvents: true },
  );
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args);
}

async function moved(payload: Moved) {
  await emit(...onTheWire("chat-moved", payload));
  theFrameNow();
}

/** ⌘⇧S on a Mac, Ctrl+Shift+S elsewhere: Use skill… on the chat in front. */
function useSkillKey() {
  const mac = onAMac();
  fireEvent.keyDown(window, { key: "S", shiftKey: true, metaKey: mac, ctrlKey: !mac });
}

describe("Use skill… through a restart (#6193)", () => {
  it("types into the chat's new session when the chat restarts under it", async () => {
    const asked = core();
    render(<App />);
    await screen.findByText("session 4");
    await act(async () => {
      await new Promise((settle) => setTimeout(settle, 0));
    });
    await act(() => moved(RUNNING));
    useSkillKey();
    const picker = await screen.findByRole("dialog", { name: "Use skill" });
    expect(await within(picker).findByText("deploy")).toBeTruthy();

    // The turn ends, so the owed restart runs: the chat is session 9 from here on.
    await act(() => moved(WAITING));
    await waitFor(() => expect(asked("restart_chat")).toHaveLength(1));
    await screen.findByText("session 9");
    await waitFor(() => expect(asked("chat_skills").map((one) => one.session)).toContain(9));

    const now = await screen.findByRole("dialog", { name: "Use skill" });
    await userEvent.click(await within(now).findByText("deploy"));

    await waitFor(() => expect(asked("use_skill")).toHaveLength(1));
    expect(asked("use_skill")[0]).toMatchObject({ session: 9, skill: "deploy" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Use skill" })).toBeNull());
  });

  it("keeps following when a pick sent to the old session is refused after the restart", async () => {
    let refuse: () => void = () => undefined;
    const asked = core(new Promise<void>((settle) => (refuse = settle)));
    render(<App />);
    await screen.findByText("session 4");
    await act(async () => {
      await new Promise((settle) => setTimeout(settle, 0));
    });
    await act(() => moved(RUNNING));
    useSkillKey();
    const picker = await screen.findByRole("dialog", { name: "Use skill" });
    // Picked before the restart: the pick is on its way to session 4 as the turn ends.
    await userEvent.click(await within(picker).findByText("deploy"));
    await waitFor(() => expect(asked("use_skill")).toHaveLength(1));
    await act(() => moved(WAITING));
    await screen.findByText("session 9");

    // The old session's refusal comes back last: it is not the dialog's any more.
    await act(async () => {
      refuse();
      await new Promise((settle) => setTimeout(settle, 0));
    });
    const now = await screen.findByRole("dialog", { name: "Use skill" });
    expect(within(now).queryByRole("alert")).toBeNull();
    await userEvent.click(await within(now).findByText("deploy"));
    await waitFor(() => expect(asked("use_skill")).toHaveLength(2));
    expect(asked("use_skill")[1]).toMatchObject({ session: 9, skill: "deploy" });
  });
});
