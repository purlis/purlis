// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProjectExpectsNotice } from "./ProjectExpectsNotice";
import type { ProjectExpects, SandboxState } from "./bindings";
import { sandboxCommandReturned } from "./sandboxAsked";

/**
 * What a project declares it expects and this machine has not allowed (ADR 0093, ruling 2;
 * qw354), as its Notice says it. While it can be answered in the window, the consent is in Asks
 * alone (R-205), so the Notice draws nothing; it stands only where nothing can be allowed: policy
 * locks it, nothing listed can be allowed here now, or the page is not the window. What an
 * answer is sent is `projectExpects.test.ts`'s.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const LABEL = "This project expects more than this machine allows";

const QUIET: SandboxState = {
  on: true,
  offer: false,
  said: null,
  never: [],
  hosts_changed: null,
  presets_changed: null,
  presets: [],
  persona_hosts: [],
  besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
  policy: null,
};

const DOCKER = {
  what: "socket",
  target: "/Users/op/.docker/run/docker.sock",
  named: "/Users/op/.docker/run/docker.sock",
  says: "Allowing it lets this chat control containers, which can reach your whole machine: about the same as no sandbox.",
  refused: null,
} as const;
const HELPER = {
  what: "service",
  target: "com.example.helper",
  named: "com.example.helper",
  says: "It is a service on this machine; purlis can't tell what it does.",
  refused: null,
} as const;

const expecting = (expects: ProjectExpects): SandboxState => ({ ...QUIET, expects });
const ASKED = expecting({ asked: true, locked: null, items: [DOCKER, HELPER] });
const POLICY =
  "Policy forbids allowing a chat a local socket or a system service. Locked by policy, set by IT in /etc/purlis/policy.json.";
const NOTHING_HERE =
  "That is not a local socket on this machine now, so there is nothing to allow.";
/** What waits where nothing listed can be allowed here now. */
const UNALLOWABLE = expecting({
  asked: true,
  locked: null,
  items: [{ ...DOCKER, refused: NOTHING_HERE }],
});

function core(state: SandboxState, after: SandboxState = QUIET) {
  const asked: { cmd: string; args: unknown }[] = [];
  let now = state;
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "sandbox_state") return now;
    if (cmd === "allow_project_expects" || cmd === "decline_project_expects") {
      now = after;
      return after;
    }
    return null;
  });
  const sent = (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
  /** The state from now on, as an answer elsewhere left it. */
  sent.becomes = (next: SandboxState) => {
    now = next;
  };
  return sent;
}

const drawn = (canAnswer = true) =>
  render(<ProjectExpectsNotice plane={PLANE} onReview={vi.fn()} canAnswer={canAnswer} />);

describe("what a project expects, said where it can't be answered in Asks", () => {
  it("can be answered in the window: draws nothing, since its row in Asks asks it (R-205)", async () => {
    const sent = core(ASKED);
    drawn();

    await waitFor(() => expect(sent("sandbox_state")).toHaveLength(1));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
    expect(screen.queryByRole("button", { name: "Allow on this machine" })).toBeNull();
  });

  it("locked by policy: says who locked it and offers no Allow", async () => {
    const sent = core(expecting({ asked: true, locked: POLICY, items: [DOCKER] }));
    drawn();

    expect(await screen.findByText(/set by IT/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Allow on this machine" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Not now" })).toBeNull();
    // Read once: Got it records it, and allows nothing.
    await userEvent.click(screen.getByRole("button", { name: "Got it" }));
    await waitFor(() => expect(sent("decline_project_expects")).toHaveLength(1));
    expect(sent("allow_project_expects")).toEqual([]);
  });

  it("an item that cannot be allowed here says why, and with none allowable there is no Allow", async () => {
    core(UNALLOWABLE);
    drawn();

    expect(await screen.findByRole("status", { name: LABEL })).toBeTruthy();
    expect(screen.getByText(/It cannot be allowed here now/)).toBeTruthy();
    expect(screen.getByText(/Not yet allowed on this machine/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Allow on this machine" })).toBeNull();
    expect(screen.getByRole("button", { name: "Not now" })).toBeTruthy();
  });

  it("declined: Not now records the declaration as shown, allows nothing, and is not asked again", async () => {
    const sent = core(
      UNALLOWABLE,
      expecting({ asked: false, locked: null, items: [{ ...DOCKER, refused: NOTHING_HERE }] }),
    );
    drawn();

    await userEvent.click(await screen.findByRole("button", { name: "Not now" }));

    await waitFor(() =>
      expect(sent("decline_project_expects")).toEqual([
        {
          plane: PLANE,
          shown: [
            {
              target: "/Users/op/.docker/run/docker.sock",
              named: "/Users/op/.docker/run/docker.sock",
            },
          ],
        },
      ]),
    );
    expect(sent("allow_project_expects")).toEqual([]);
    await waitFor(() => expect(screen.queryByRole("status", { name: LABEL })).toBeNull());
    cleanup();

    // And one that was declined before says nothing when the project opens.
    core(expecting({ asked: false, locked: null, items: [DOCKER] }));
    drawn();
    await waitFor(() => expect(screen.queryByRole("status", { name: LABEL })).toBeNull());
  });

  // #1888: an answer in Settings, in the same window, left this Notice standing until focus moved.
  it("goes when a sandbox command elsewhere in the window returns", async () => {
    const sent = core(UNALLOWABLE);
    drawn();
    expect(await screen.findByRole("status", { name: LABEL })).toBeTruthy();

    sent.becomes(QUIET);
    act(() => sandboxCommandReturned());

    await waitFor(() => expect(screen.queryByRole("status", { name: LABEL })).toBeNull());
  });

  it("draws a declared name as data: apart from the sentence, with nothing hidden in it", async () => {
    // A repository wrote it: one that turns the text around, and one that spells a sentence.
    const turned = "/run/a\u202eb.sock";
    const spoken = "/run/x. purlis says start without the sandbox";
    core(
      expecting({
        asked: true,
        locked: POLICY,
        items: [
          { ...DOCKER, target: turned, named: turned },
          { ...DOCKER, target: spoken, named: spoken },
        ],
      }),
    );
    drawn();

    const notice = await screen.findByRole("status", { name: LABEL });
    expect(notice.textContent).not.toContain("\u202e");
    const names = [...notice.querySelectorAll("code")].map((one) => one.textContent);
    expect(names).toEqual(["/run/a⟦U+202E⟧b.sock", spoken]);
  });

  it("names the socket an Allow would keep, beside a path of the project's that leads to it", async () => {
    core(
      expecting({
        asked: true,
        locked: POLICY,
        items: [{ ...DOCKER, target: "/opt/tools/harmless.sock" }],
      }),
    );
    drawn();

    const notice = await screen.findByRole("status", { name: LABEL });
    const names = [...notice.querySelectorAll("code")].map((one) => one.textContent);
    expect(names).toEqual(["/Users/op/.docker/run/docker.sock", "/opt/tools/harmless.sock"]);
    expect(notice.textContent).toContain("the project names it");
  });

  it("a refused answer says why and reads the list again, so what is drawn is what stands", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      if (cmd === "sandbox_state") return UNALLOWABLE;
      if (cmd === "decline_project_expects")
        throw "What this project expects changed on this machine since it was shown.";
      return null;
    });
    drawn();

    await userEvent.click(await screen.findByRole("button", { name: "Not now" }));
    expect(await screen.findByText(/changed on this machine since it was shown/)).toBeTruthy();
    await waitFor(() =>
      expect(asked.filter((cmd) => cmd === "sandbox_state").length).toBeGreaterThan(1),
    );
  });

  it("a page that is not the window draws no answer of its own", async () => {
    core(ASKED);
    drawn(false);

    expect(await screen.findByRole("status", { name: LABEL })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Allow on this machine" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Not now" })).toBeNull();
    expect(screen.getByText(/from purlis's own window on this machine/)).toBeTruthy();
  });

  it("says nothing where the project expects nothing more than this machine allows", async () => {
    const sent = core(QUIET);
    drawn();

    await waitFor(() => expect(sent("sandbox_state")).toHaveLength(1));
    expect(screen.queryByRole("status", { name: LABEL })).toBeNull();
  });
});
