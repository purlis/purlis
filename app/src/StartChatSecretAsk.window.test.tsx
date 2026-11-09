// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { secretAsk } from "./secretAsk.testkit";

/**
 * **The Start dialog fills a start's missing credential, and the start goes on** (#5137, from
 * D-5074-2): a start refused on its profile credential's Secret ask draws that ask's body in the
 * dialog. Fill answers the one ask for that vault and key and starts again with no second press;
 * Not now declines it and says the core's words for a declined start.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

/** ⌘⏎ on a Mac, Ctrl+Enter elsewhere: the platform the test runs on. */
const MAC = navigator.platform.startsWith("Mac");
const CHORD = { key: "Enter", metaKey: MAC, ctrlKey: !MAC };

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const OPTIONS = {
  profiles: [
    {
      name: "work-key",
      kind: "claude",
      shown: "claude",
      source: "built-in",
      is_default: true,
      approval: null,
    },
    // A profile approved by no one yet: starting on it is an approval, a press of its own.
    {
      name: "codex-new",
      kind: "codex",
      shown: "codex",
      source: "charter.local.toml",
      is_default: false,
      approval: "new",
    },
  ],
  refused: [],
  personas: ["devops", "steward"],
  persona: "devops",
  persona_profiles: {},
  ignore_fix: null,
  declares_none: true,
};

const REFUSED =
  "this chat's profile names ANTHROPIC_API_KEY as 'anthropic' in vault 'providers', and it could " +
  "not be read (waiting on you for providers/anthropic; retry once answered), so nothing was " +
  "started; fill it from the Secret ask in the Inbox, then start the chat again";
const DECLINED =
  "this chat's profile names ANTHROPIC_API_KEY as 'anthropic' in vault 'providers', and it could " +
  "not be read (declined providers/anthropic), so nothing was started; start the chat again " +
  "to be asked for it again";

/** The start's ask, as the registry lists it: a start has no chat yet (D-5074-1). */
const ASK = (() => {
  const ask = secretAsk({ persona: "devops", vault: "providers", key: "anthropic" });
  return { ...ask, session: 0, ask: "secret:4", answer: { via: "secret" as const, id: 4 } };
})();

/** The window's core: the first start is refused on the ask while it is open; once it is
 *  answered, the ask is gone from the list and a start starts. `hold` keeps the answer, or the
 *  start after it, waiting until `release` is called; `refuse` refuses the answer in its words. */
function core(how: { hold?: "answer" | "start"; refuse?: string } = {}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let open = false;
  let opened = 0;
  let release: () => void = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  mockIPC(async (cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [],
        personas: ["devops", "steward"],
        persona: "devops",
        unfiled: [],
      };
    if (cmd === "running_sessions") return [];
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "start_details") return [];
    if (cmd === "start_options" || cmd === "start_options_first") return OPTIONS;
    if (cmd === "asks_waiting") return { plane: PLANE, asks: open ? [ASK] : [] };
    if (cmd === "answer_secret_ask") {
      if (how.hold === "answer") await held;
      if (how.refuse !== undefined) throw how.refuse;
      open = false;
      return null;
    }
    if (cmd === "start_chat") {
      if (!answered()) {
        open = true;
        throw {
          words: REFUSED,
          limit: null,
          secret_ask: {
            variable: "ANTHROPIC_API_KEY",
            vault: "providers",
            key: "anthropic",
            declined: DECLINED,
          },
        };
      }
      if (how.hold === "start") await held;
      return { session: ++opened };
    }
    return null;
  });
  const answered = () => asked.some(({ cmd }) => cmd === "answer_secret_ask");
  const of = (cmd: string) => asked.filter((one) => one.cmd === cmd);
  return { asked, of, release: () => release() };
}

/** The dialog's main answer, whatever it says now. */
const mainOf = (dialog: HTMLElement) => {
  const main = dialog.querySelector<HTMLElement>('[data-dialog-action="main"]');
  if (main === null) throw new Error("the dialog has no main answer");
  return main;
};

async function startRefused() {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: "New chat" }));
  await user.click(await screen.findByRole("button", { name: /^Start chat/ }));
  const dialog = screen.getByRole("dialog", { name: "Start a chat" });
  const body = await within(dialog).findByRole("group", {
    name: "devops asks for anthropic from vault providers",
  });
  return { user, dialog, body };
}

describe("a start missing its profile credential, in the Start dialog (#5137)", () => {
  it("draws the Secret ask's body in the dialog, in place of the words that point to the Inbox", async () => {
    core();
    render(<App />);

    const { dialog, body } = await startRefused();

    expect(within(body).getByLabelText("Value of anthropic")).toBeTruthy();
    expect(dialog.textContent).not.toContain("fill it from the Secret ask in the Inbox");
    // What the start does with it, never what a chat hears; and its key is its profile's.
    expect(body.textContent).toContain("starts with the value in ANTHROPIC_API_KEY.");
    expect(within(body).queryByRole("button", { name: /^Change where/ })).toBeNull();
  });

  it("filled, answers the one ask for that vault and key and starts again with no second press", async () => {
    const { of } = core();
    render(<App />);

    const { user, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));

    await waitFor(() => expect(of("start_chat")).toHaveLength(2));
    expect(of("answer_secret_ask")).toHaveLength(1);
    expect(of("answer_secret_ask")[0].args).toMatchObject({
      plane: PLANE,
      id: 4,
      value: "fr3sh-anthropic",
    });
    expect(of("start_chat")[1].args).toMatchObject({ profile: "work-key", persona: "devops" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Start a chat" })).toBeNull());
    expect(await screen.findByTestId("pane")).toHaveTextContent("session 1");
  });

  it("answered in the dialog, the ask is read again and gone from the Inbox", async () => {
    const { asked } = core();
    render(<App />);

    const { user, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Start a chat" })).toBeNull());

    const answeredAt = asked.findIndex(({ cmd }) => cmd === "answer_secret_ask");
    await waitFor(() =>
      expect(asked.slice(answeredAt).some(({ cmd }) => cmd === "asks_waiting")).toBe(true),
    );
    expect(screen.queryByTestId("secret-ask")).toBeNull();
  });

  it("Not now declines it, starts nothing and says the core's words for a declined start", async () => {
    const { of } = core();
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.click(within(body).getByRole("button", { name: "Not now" }));

    await waitFor(() => expect(within(dialog).getByRole("alert")).toHaveTextContent(DECLINED));
    expect(of("answer_secret_ask")).toHaveLength(1);
    expect(of("answer_secret_ask")[0].args).toMatchObject({ id: 4, value: null });
    expect(of("start_chat")).toHaveLength(1);
    expect(within(dialog).queryByTestId("secret-ask")).toBeNull();
  });

  it("filled, starts with what the dialog shows now, not what it showed at the refusal", async () => {
    const { of } = core();
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.click(within(dialog).getByRole("option", { name: "steward" }));
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));

    await waitFor(() => expect(of("start_chat")).toHaveLength(2));
    expect(of("start_chat")[1].args).toMatchObject({ persona: "steward" });
  });

  it("filled while another profile is picked since, starts nothing and says whose key it filled", async () => {
    const { of, release } = core({ hold: "answer" });
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));
    await waitFor(() => expect(of("answer_secret_ask")).toHaveLength(1));
    await user.click(within(dialog).getByRole("radio", { name: /codex/ }));
    // While the fill is sent, the dialog never asks for it to be filled again.
    expect(dialog.textContent).not.toContain("Pick work-key again");
    release();

    expect(await within(dialog).findByText("Filled anthropic for work-key.")).toHaveAttribute(
      "role",
      "alert",
    );
    await new Promise((settle) => setTimeout(settle, 100));
    expect(of("approve_profile")).toEqual([]);
    expect(of("start_chat")).toHaveLength(1);
  });

  it("an answer the core refuses, with another profile picked since, is said all the same (#5670 review)", async () => {
    const { of, release } = core({
      hold: "answer",
      refuse: "the vault providers is locked: unlock it, then fill again",
    });
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));
    await waitFor(() => expect(of("answer_secret_ask")).toHaveLength(1));
    await user.click(within(dialog).getByRole("radio", { name: /codex/ }));
    release();

    await waitFor(() =>
      expect(
        within(dialog)
          .getAllByRole("alert")
          .some((one) =>
            one.textContent?.includes("the vault providers is locked: unlock it, then fill again"),
          ),
      ).toBe(true),
    );
  });

  it("a hint for the hidden ask's own boxes is not said once another profile is picked (#5670 review)", async () => {
    core();
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.click(within(body).getByRole("checkbox", { name: /^Remember/ }));
    const purpose = within(body).queryByRole("textbox", { name: /for/i });
    if (purpose) await user.clear(purpose);
    await user.click(within(body).getByRole("button", { name: "Fill" }));
    await waitFor(() => expect(dialog.textContent).toContain("or untick Remember."));
    await user.click(within(dialog).getByRole("radio", { name: /codex/ }));

    await waitFor(() => expect(within(dialog).queryByTestId("secret-ask")).toBeNull());
    expect(dialog.textContent).not.toContain("or untick Remember.");
  });

  it("an answer refused with another profile picked since says first where to answer it again (#5670 review)", async () => {
    const LOCKED = "the vault providers is locked: unlock it, then fill again";
    const { of, release } = core({ hold: "answer", refuse: LOCKED });
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));
    await waitFor(() => expect(of("answer_secret_ask")).toHaveLength(1));
    await user.click(within(dialog).getByRole("radio", { name: /codex/ }));
    release();

    await waitFor(() => {
      const said =
        within(dialog)
          .getAllByRole("alert")
          .find((one) => one.textContent?.includes(LOCKED))?.textContent ?? "";
      expect(said).toContain("Pick work-key again to fill anthropic here.");
      expect(said.indexOf("Pick work-key again")).toBeLessThan(said.indexOf(LOCKED));
    });
  });

  it("a pick of another profile drops the refused profile's ask, and picking it back draws it again (#5670)", async () => {
    core();
    render(<App />);

    const { user, dialog } = await startRefused();
    await user.click(within(dialog).getByRole("radio", { name: /codex/ }));

    await waitFor(() => expect(within(dialog).queryByTestId("secret-ask")).toBeNull());
    // The refusal still shown is said to be the refused profile's.
    // Said in the refusal's own alert, beside the words that point to the Inbox.
    const refusal = within(dialog)
      .getAllByRole("alert")
      .find((one) => one.textContent?.includes("fill it from the Secret ask in the Inbox"));
    expect(refusal).toHaveTextContent("Pick work-key again to fill anthropic here.");
    await user.click(within(dialog).getByRole("radio", { name: /work-key/ }));
    expect(
      await within(dialog).findByRole("group", {
        name: "devops asks for anthropic from vault providers",
      }),
    ).toBeTruthy();
  });

  it("holds ⏎ from a persona row, ⌘⏎ from a card and Start while its answer is sent (#5670)", async () => {
    const { of, release } = core({ hold: "answer" });
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));
    await waitFor(() => expect(of("answer_secret_ask")).toHaveLength(1));

    const row = within(dialog).getByRole("option", { name: "devops" });
    fireEvent.keyDown(row, { key: "Enter" });
    fireEvent.keyUp(row, { key: "Enter" });
    const card = within(dialog).getByRole("radio", { name: /work-key/ });
    fireEvent.keyDown(card, CHORD);
    fireEvent.keyUp(card, CHORD);
    const start = mainOf(dialog);
    expect(start).toBeDisabled();
    fireEvent.keyDown(within(dialog).getByRole("textbox", { name: /^Name/ }), CHORD);

    await new Promise((settle) => setTimeout(settle, 50));
    expect(of("start_chat")).toHaveLength(1);
    release();
    // The fill's own start goes on, once.
    await waitFor(() => expect(of("start_chat")).toHaveLength(2));
    await new Promise((settle) => setTimeout(settle, 50));
    expect(of("start_chat")).toHaveLength(2);
  });

  it("⏎ and ⌘⏎ on the ask's own radios neither start nor answer: they are no harness card (#5670 review)", async () => {
    const { of } = core();
    render(<App />);

    const { body } = await startRefused();
    // A value typed, so a chord that reached the ask's Fill would answer.
    await userEvent.setup().type(within(body).getByLabelText("Value of anthropic"), "fr3sh");
    const choice = within(body).getAllByRole("radio")[0];
    fireEvent.keyDown(choice, { key: "Enter" });
    fireEvent.keyUp(choice, { key: "Enter" });
    fireEvent.keyDown(choice, CHORD);
    fireEvent.keyUp(choice, CHORD);

    await new Promise((settle) => setTimeout(settle, 50));
    expect(of("start_chat")).toHaveLength(1);
    expect(of("answer_secret_ask")).toEqual([]);
  });

  it("while Not now is sent, Start is held and never says it is starting (#5670)", async () => {
    const { of, release } = core({ hold: "answer" });
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.click(within(body).getByRole("button", { name: "Not now" }));
    await waitFor(() => expect(of("answer_secret_ask")).toHaveLength(1));

    const start = mainOf(dialog);
    expect(start).toBeDisabled();
    expect(start.textContent).toContain("Start chat");
    expect(start.textContent).not.toContain("Starting");
    release();
    await waitFor(() => expect(within(dialog).getByRole("alert")).toHaveTextContent(DECLINED));
  });

  it("⌘⏎ in the value's box fills, and the start goes on once (#5670)", async () => {
    const { of } = core();
    render(<App />);

    const { user, body } = await startRefused();
    const box = within(body).getByLabelText("Value of anthropic");
    await user.type(box, "fr3sh-anthropic");
    fireEvent.keyDown(box, CHORD);

    await waitFor(() => expect(of("start_chat")).toHaveLength(2));
    expect(of("answer_secret_ask")).toHaveLength(1);
    expect(of("answer_secret_ask")[0].args).toMatchObject({ value: "fr3sh-anthropic" });
  });

  it("cancelled while its answer is sent, starts nothing", async () => {
    const { of, release } = core({ hold: "answer" });
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));
    await waitFor(() => expect(of("answer_secret_ask")).toHaveLength(1));
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Start a chat" })).toBeNull());
    release();

    await new Promise((settle) => setTimeout(settle, 100));
    expect(of("start_chat")).toHaveLength(1);
    expect(screen.queryByTestId("pane")).toBeNull();
  });

  it("an answer the core refuses says why, keeps the ask, and starts nothing", async () => {
    const { of } = core({ refuse: "the vault providers is locked: unlock it, then fill again" });
    render(<App />);

    const { user, dialog, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));

    await waitFor(() =>
      expect(within(dialog).getByRole("alert")).toHaveTextContent(
        "the vault providers is locked: unlock it, then fill again",
      ),
    );
    expect(within(dialog).getByTestId("secret-ask")).toBeTruthy();
    expect(of("start_chat")).toHaveLength(1);
  });

  it("answered, every list of the project's asks is read again before the start comes back", async () => {
    const { asked } = core({ hold: "start" });
    render(<App />);

    const { user, body } = await startRefused();
    await user.type(within(body).getByLabelText("Value of anthropic"), "fr3sh-anthropic");
    await user.click(within(body).getByRole("button", { name: "Fill" }));

    await waitFor(() => {
      const at = asked.findIndex(({ cmd }) => cmd === "answer_secret_ask");
      expect(at).toBeGreaterThan(-1);
      expect(asked.slice(at).some(({ cmd }) => cmd === "asks_waiting")).toBe(true);
    });
  });
});
