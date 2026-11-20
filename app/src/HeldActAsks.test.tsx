// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { HeldActAsks, HeldActRow, onceMoreSaid } from "./HeldActAsks";
import type { HeldActShown } from "./bindings";
import { CI_PUSH, EVERY_KIND, FORK, RELEASE, TAG } from "./HeldActAsks.fixtures";

/**
 * **Held forge acts in Asks** (one login F2; ADR 0128 D8): each ask shows every field the digest
 * covers, a press asks once more before it makes anything, the second press names the nonce the
 * first returned, and Don't makes nothing.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const noop = () => undefined;

describe("what an ask shows", () => {
  it.each(EVERY_KIND.map((act) => [act.kind, act] as const))(
    "a %s ask shows every row its digest covers and the long text under them",
    (_kind, act) => {
      render(
        <ul>
          <HeldActRow act={act} onPress={noop} onCancel={noop} onDont={noop} />
        </ul>,
      );
      const facts = screen.getByRole("list", { name: `What the ${act.kind} is` }) ?? undefined;
      for (const row of act.rows) {
        const term = within(facts).getByText(row.label);
        expect(term.closest("div")?.textContent).toContain(row.value);
      }
      if (act.body !== null) {
        const figure = screen.getByRole("figure", { name: act.body.label });
        expect(figure.textContent).toContain(act.body.text);
      } else {
        expect(screen.queryByRole("figure")).toBeNull();
      }
    },
  );

  it("names the chat, the kind and the repo, and draws a way to say no beside the press", () => {
    render(
      <ul>
        <HeldActRow act={RELEASE} onPress={noop} onCancel={noop} onDont={noop} />
      </ul>,
    );
    expect(screen.getByText("Chat 3 asked for a release in acme/app")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Make the release" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Don't" })).toBeTruthy();
  });

  it("draws the CI diff with its paths, as plain text", () => {
    render(
      <ul>
        <HeldActRow act={CI_PUSH} onPress={noop} onCancel={noop} onDont={noop} />
      </ul>,
    );
    const diff = screen.getByRole("figure", { name: "Changed lines" });
    expect(diff.textContent).toContain("+++ b/.github/workflows/ci.yml");
    expect(diff.textContent).toContain("-    runs-on: ubuntu-22.04");
    expect(diff.textContent).toContain("+    runs-on: ubuntu-24.04");
  });
});

/** The core, as a test stands it in: what it listed, and every command it was sent. */
function core(listed: readonly HeldActShown[], answers: Record<string, unknown>) {
  const sent: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "held_acts_waiting") return listed;
    if (cmd in answers) {
      const answer = answers[cmd];
      if (answer instanceof Error) throw answer.message;
      return answer;
    }
    return undefined;
  });
  return sent;
}

describe("the two presses", () => {
  it("the first press makes nothing and asks once more; the second names the nonce", async () => {
    const user = userEvent.setup();
    const sent = vi.fn();
    const first = {
      step: "asks-once-more",
      shown: TAG,
      nonce: "nonce-1",
    };
    const log = core([TAG], {
      press_held_act: (() => undefined) as never,
    });
    clearMocks();
    let calls = 0;
    mockIPC((cmd, args) => {
      log.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      sent(cmd);
      if (cmd === "held_acts_waiting") return calls > 1 ? [] : [TAG];
      if (cmd === "press_held_act") {
        calls += 1;
        return calls === 1 ? first : { step: "done", said: "Made the tag v1.4.0." };
      }
      return undefined;
    });
    render(<HeldActAsks plane={PLANE} />);
    await user.click(await screen.findByRole("button", { name: "Make the tag" }));
    expect(await screen.findByText(onceMoreSaid(TAG))).toBeTruthy();
    const firstPress = log.find((one) => one.cmd === "press_held_act");
    expect(firstPress?.args).toMatchObject({ ask: 1, digest: "digest-tag", nonce: null });

    await user.click(screen.getByRole("button", { name: "Yes, make the tag" }));
    expect(await screen.findByText("Made the tag v1.4.0.")).toBeTruthy();
    const presses = log.filter((one) => one.cmd === "press_held_act");
    expect(presses).toHaveLength(2);
    expect(presses[1].args).toMatchObject({ ask: 1, digest: "digest-tag", nonce: "nonce-1" });
  });

  it("Cancel takes the question back without sending anything, and the next press asks afresh", async () => {
    const user = userEvent.setup();
    const log = core([FORK], {
      press_held_act: { step: "asks-once-more", shown: FORK, nonce: "n" },
    });
    render(<HeldActAsks plane={PLANE} />);
    await user.click(await screen.findByRole("button", { name: "Fork it" }));
    await screen.findByText(onceMoreSaid(FORK));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByText(onceMoreSaid(FORK))).toBeNull());
    expect(log.filter((one) => one.cmd === "press_held_act")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Fork it" }));
    await waitFor(() =>
      expect(log.filter((one) => one.cmd === "press_held_act")).toHaveLength(2),
    );
    expect(log.filter((one) => one.cmd === "press_held_act")[1].args).toMatchObject({
      nonce: null,
    });
  });

  it("says the app's sentence where a press did nothing, and keeps the ask", async () => {
    const user = userEvent.setup();
    core([TAG], { press_held_act: new Error("the ask no longer shows what you pressed") });
    render(<HeldActAsks plane={PLANE} />);
    await user.click(await screen.findByRole("button", { name: "Make the tag" }));
    expect(
      await screen.findByText(/the ask no longer shows what you pressed/),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Make the tag" })).toBeTruthy();
  });

  it("Don't sends the ask's number and nothing else", async () => {
    const user = userEvent.setup();
    const log = core([RELEASE], { dont_held_act: true });
    render(<HeldActAsks plane={PLANE} />);
    await user.click(await screen.findByRole("button", { name: "Don't" }));
    await waitFor(() => expect(log.some((one) => one.cmd === "dont_held_act")).toBe(true));
    expect(log.find((one) => one.cmd === "dont_held_act")?.args).toMatchObject({ ask: 2 });
    expect(log.some((one) => one.cmd === "press_held_act")).toBe(false);
  });

  it("draws nothing where no act waits", async () => {
    core([], {});
    const { container } = render(<HeldActAsks plane={PLANE} />);
    await waitFor(() => expect(container.querySelector("li")).toBeNull());
    expect(screen.queryByRole("list", { name: "Held forge acts" })).toBeNull();
  });
});
