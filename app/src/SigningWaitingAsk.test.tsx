// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Shown, SigningState, SigningWaiting } from "./bindings";
import { answerTaken } from "./asks";
import { Inbox } from "./Inbox";
import { SigningSetupAsked } from "./SigningRefusal";

/**
 * **One notice for lanes refused for unsigned commits** (#5633, ADR 0116 §3): the registry's one
 * Ask for them all, drawn in the Inbox once; Set up signing opens the dialog in the project's
 * window, which ends in Sign and retry all N; Sign and retry all sends each lane; Dismiss puts
 * the done notice away.
 */

const PLANE = "/plane";

const lane = (piece: string): SigningWaiting => ({
  ws: "smart-ide",
  repo: "purlis",
  piece,
  branch: piece,
  kind: "github",
  forge: "GitHub",
  host: "github.com",
  path: "purlis/purlis",
  commit: "0123abc0000000000000000000000000000000000",
  chat: 7,
  sent: false,
});

const ask = (options: { id: string; label: string }[], says: string): Shown => ({
  session: 0,
  ask: "signing-waiting",
  says,
  options: options.map((one) => ({ ...one, allows: false })),
  source: "signing",
  chain: ["purlis"],
  answer: { via: "signing" },
});

const THREE =
  "3 lanes can't merge: purlis/purlis only accepts signed commits\n- purlis · tab-names (tab-names)\n- purlis · empty-tab (empty-tab)\n- purlis · pane-foot (pane-foot)";

let state: SigningState;
let waiting: SigningWaiting[];
let sent: { cmd: string; args: Record<string, unknown> }[];

beforeEach(() => {
  state = {
    on: false,
    title: "purlis · studio",
    accounts: [
      {
        kind: "github",
        login: "octocat",
        host: "github.com",
        forge: "GitHub",
        may_add: true,
        verified_as: null,
        verifies: true,
      },
    ],
  };
  waiting = [lane("tab-names"), lane("empty-tab"), lane("pane-foot")];
  sent = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    switch (cmd) {
      case "signing_state":
        return structuredClone(state);
      case "signing_waiting":
        return structuredClone(waiting);
      case "signing_make_key":
        state = { ...state, on: true };
        return "SHA256:q3VbT1sM";
      case "signing_add_key":
        return "added";
      case "signing_check":
        return { verified: true, reason: "valid", email: "o@example.com", repo: "purlis/purlis" };
      case "signing_sign_and_retry":
        return waiting.map((one) => ({ ws: one.ws, repo: one.repo, piece: one.piece, signed: 1 }));
      default:
        return null;
    }
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const commandsSent = (cmd: string) => sent.filter((one) => one.cmd === cmd);

describe("the one notice for lanes waiting on signing", () => {
  it("is drawn once in the Inbox, listing every lane", async () => {
    render(
      <Inbox
        plane={PLANE}
        asks={[ask([{ id: "set-up", label: "Set up signing" }], THREE)]}
        onGo={() => {}}
        onLeave={() => {}}
        onIgnore={() => {}}
      />,
    );
    expect(screen.getAllByText(/3 lanes can't merge/)).toHaveLength(1);
    for (const piece of ["tab-names", "empty-tab", "pane-foot"])
      expect(document.body.textContent).toContain(`purlis · ${piece}`);
    await userEvent.click(screen.getByRole("button", { name: "Answer purlis" }));
    expect(await screen.findByRole("button", { name: "Set up signing" })).toBeTruthy();
  });

  it("Set up signing opens the dialog in the project's window, ending in Sign and retry all 3", async () => {
    render(<SigningSetupAsked plane={PLANE} />);
    const answered = await answerTaken(
      PLANE,
      ask([{ id: "set-up", label: "Set up signing" }], THREE),
      "set-up",
    );
    // It opened a dialog: the Ask still waits.
    expect(answered.opened).toBe(true);
    const dialog = await screen.findByRole("dialog", { name: "Set up signing" });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Make my key" }));
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Sign and retry all 3" }),
    );
    await waitFor(() => expect(commandsSent("signing_sign_and_retry").length).toBe(1));
    expect(commandsSent("signing_sign_and_retry")[0]?.args).toMatchObject({
      plane: PLANE,
      lanes: [
        { ws: "smart-ide", repo: "purlis", piece: "tab-names" },
        { ws: "smart-ide", repo: "purlis", piece: "empty-tab" },
        { ws: "smart-ide", repo: "purlis", piece: "pane-foot" },
      ],
    });
  });

  it("with lanes on two forges, Set up signing on GitLab sets up GitLab's account and retries its lanes only", async () => {
    state = {
      ...state,
      accounts: [
        ...state.accounts,
        {
          kind: "gitlab",
          login: "sam",
          host: "gitlab.example.org",
          forge: "GitLab",
          may_add: true,
          verified_as: null,
          verifies: true,
        },
      ],
    };
    waiting = [
      lane("tab-names"),
      {
        ...lane("empty-tab"),
        kind: "gitlab",
        forge: "GitLab",
        host: "gitlab.example.org",
        path: "team/purlis",
      },
    ];
    render(<SigningSetupAsked plane={PLANE} />);
    const answered = await answerTaken(
      PLANE,
      ask(
        [
          { id: "set-up:github:github.com", label: "Set up signing on GitHub" },
          { id: "set-up:gitlab:gitlab.example.org", label: "Set up signing on GitLab" },
        ],
        THREE,
      ),
      "set-up:gitlab:gitlab.example.org",
    );
    expect(answered.opened).toBe(true);
    const dialog = await screen.findByRole("dialog", { name: "Set up signing" });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Make my key" }));
    await userEvent.click(await within(dialog).findByRole("button", { name: "Sign and retry" }));
    await waitFor(() => expect(commandsSent("signing_sign_and_retry").length).toBe(1));
    expect(commandsSent("signing_add_key")[0]?.args).toMatchObject({
      forge: { kind: "gitlab", host: "gitlab.example.org" },
    });
    expect(commandsSent("signing_sign_and_retry")[0]?.args).toMatchObject({
      lanes: [{ ws: "smart-ide", repo: "purlis", piece: "empty-tab" }],
    });
  });

  it("Sign and retry all sends every lane still waiting, and none already sent", async () => {
    waiting = [lane("tab-names"), { ...lane("empty-tab"), sent: true }];
    const answered = await answerTaken(
      PLANE,
      ask([{ id: "retry-all", label: "Sign and retry all 1" }], THREE),
      "retry-all",
    );
    expect(answered.refused).toBeUndefined();
    expect(commandsSent("signing_sign_and_retry")[0]?.args).toMatchObject({
      lanes: [{ ws: "smart-ide", repo: "purlis", piece: "tab-names" }],
    });
  });

  it("once all were sent, Dismiss puts it away", async () => {
    await answerTaken(
      PLANE,
      ask([{ id: "dismiss", label: "Dismiss" }], "3 lanes signed and sent ✓"),
      "dismiss",
    );
    expect(commandsSent("signing_forget")).toEqual([
      { cmd: "signing_forget", args: { plane: PLANE, lane: null } },
    ]);
  });
});
