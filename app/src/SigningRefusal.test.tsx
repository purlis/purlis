// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SigningRetried, SigningState, SigningWaiting } from "./bindings";
import { SigningRefusal, SigningRefusalNotice } from "./SigningRefusal";

/**
 * **A forge's refusal of unsigned commits, on the lane and in the chat** (#5630, ADR 0116 §3):
 * named for what it is, with Set up signing, whose last step is Sign and retry, and then
 * "Signed and sent ✓". The chat says it can't set signing up itself.
 */

const PLANE = "p1";

const LANE: SigningWaiting = {
  ws: "smart-ide",
  repo: "purlis",
  piece: "tab-names",
  branch: "tab-names",
  kind: "github",
  forge: "GitHub",
  host: "github.com",
  path: "octo/hello",
  commit: "0123abc0000000000000000000000000000000000",
  chat: 7,
  sent: false,
};

const STATE: SigningState = {
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

let state: SigningState;
let waiting: SigningWaiting[];
let retried: SigningRetried[] | string;
let sent: { cmd: string; args: Record<string, unknown> }[];

beforeEach(() => {
  state = structuredClone(STATE);
  waiting = [structuredClone(LANE)];
  retried = [{ ws: LANE.ws, repo: LANE.repo, piece: LANE.piece, signed: 2 }];
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
        return {
          verified: true,
          reason: "valid",
          email: "octocat@example.com",
          repo: "octo/hello",
        };
      case "signing_sign_and_retry":
        if (typeof retried === "string") throw retried;
        return retried;
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

describe("SigningRefusal", () => {
  it("on a lane, names the refusal, sets signing up, and ends Signed and sent with Sign and retry", async () => {
    render(<SigningRefusal plane={PLANE} lane={LANE} on={false} where="lane" />);
    expect(screen.getByLabelText("GitHub only accepts signed commits here")).toBeTruthy();
    expect(document.body.textContent).toContain("octo/hello requires signed commits");
    expect(document.body.textContent).not.toMatch(/SSH|Ed25519|gpg/);
    await userEvent.click(screen.getByRole("button", { name: "Set up signing" }));

    const dialog = await screen.findByRole("dialog", { name: "Set up signing" });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Make my key" }));
    expect(await within(dialog).findByText("Verified on GitHub ✓")).toBeTruthy();
    // The check is of the repository the refusal came from.
    expect(commandsSent("signing_check")[0]?.args).toMatchObject({
      forge: { kind: "github", host: "github.com" },
      repo: "octo/hello",
    });
    await userEvent.click(within(dialog).getByRole("button", { name: "Sign and retry" }));

    expect(await screen.findByLabelText("Signed and sent ✓")).toBeTruthy();
    expect(document.body.textContent).toContain("2 commits re-signed, and tab-names sent again.");
    expect(commandsSent("signing_sign_and_retry")).toEqual([
      {
        cmd: "signing_sign_and_retry",
        args: { plane: PLANE, lanes: [{ ws: LANE.ws, repo: LANE.repo, piece: LANE.piece }] },
      },
    ]);
  });

  it("with signing on already, offers Sign and retry at once", async () => {
    render(<SigningRefusal plane={PLANE} lane={LANE} on where="lane" />);
    expect(document.body.textContent).toContain("nothing already pushed is rewritten");
    expect(screen.queryByRole("button", { name: "Set up signing" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Sign and retry" }));
    expect(await screen.findByLabelText("Signed and sent ✓")).toBeTruthy();
  });

  it("a lane refused again stays, with why", async () => {
    retried = [
      {
        ws: LANE.ws,
        repo: LANE.repo,
        piece: LANE.piece,
        signed: 0,
        why: "GitHub only accepts signed commits here: it refused the push of tab-names",
      },
    ];
    render(<SigningRefusal plane={PLANE} lane={LANE} on where="lane" />);
    await userEvent.click(screen.getByRole("button", { name: "Sign and retry" }));
    await waitFor(() => expect(document.body.textContent).toContain("It wasn't sent:"));
    expect(screen.queryByLabelText("Signed and sent ✓")).toBeNull();
    expect(screen.getByRole("button", { name: "Sign and retry" })).toBeTruthy();
  });

  it("in the chat, says the chat can't set signing up and that it's yours to do", async () => {
    render(<SigningRefusalNotice plane={PLANE} session={7} />);
    expect(await screen.findByLabelText("GitHub only accepts signed commits here")).toBeTruthy();
    expect(document.body.textContent).toContain("The chat can't set signing up itself");
  });

  it("in another chat, says nothing", async () => {
    render(<SigningRefusalNotice plane={PLANE} session={8} />);
    await waitFor(() => expect(commandsSent("signing_waiting").length).toBeGreaterThan(0));
    expect(screen.queryByLabelText("GitHub only accepts signed commits here")).toBeNull();
  });
});

describe("SigningRefusal, dismissed and failed elsewhere", () => {
  it("says why a Sign and retry from the notice failed, and Dismiss drops the lane", async () => {
    render(
      <SigningRefusal
        plane={PLANE}
        lane={{ ...LANE, why: "tab-names is on another branch now" }}
        on
        where="lane"
      />,
    );
    expect(document.body.textContent).toContain(
      "It wasn't sent: tab-names is on another branch now",
    );
    await userEvent.click(screen.getByRole("button", { name: /Dismiss/ }));
    await waitFor(() =>
      expect(commandsSent("signing_forget")).toEqual([
        {
          cmd: "signing_forget",
          args: { plane: PLANE, lane: { ws: LANE.ws, repo: LANE.repo, piece: LANE.piece } },
        },
      ]),
    );
  });
});
