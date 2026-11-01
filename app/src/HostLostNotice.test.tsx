// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { HostLostNotice, LOST } from "./HostLostNotice";
import type { LostChat } from "./bindings";

/**
 * A chat whose program ended with its session host says so on its pane (#2007): how its run
 * was left, failed or hibernated, and Resume, its restart on its conversation in one click.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const FAILED: LostChat = {
  plane: PLANE,
  session: 7,
  left_in: "failed",
  resumable: true,
  with_the_runner: false,
};
const HIBERNATED: LostChat = { ...FAILED, left_in: "hibernated" };
const NO_CONVERSATION: LostChat = { ...FAILED, resumable: false };

/** A core holding `held` for chat 7. */
function core(held: LostChat | null) {
  const asked: { cmd: string; args: unknown }[] = [];
  let now = held;
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "chat_lost_with_the_host") return now;
      if (cmd === "put_away_lost_chat") {
        now = null;
        return null;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    pressed: () => asked.filter((one) => one.cmd !== "chat_lost_with_the_host"),
    lose: (one: LostChat) => {
      now = one;
    },
  };
}

const notice = () => screen.findByRole("status", { name: "Session host stopped" });

describe("the Notice of a chat lost with its session host", () => {
  it("says a chat that ended with a gone runner lost its conversation, with no Resume", async () => {
    core({ ...HIBERNATED, resumable: false, with_the_runner: true });
    render(<HostLostNotice plane={PLANE} session={7} onRestart={() => {}} />);
    const said = await screen.findByRole("status", { name: "Runner gone" });
    expect(said).toHaveTextContent(
      "This chat ended with its pool's runner, which is gone, and its conversation was kept on that runner. Start a new chat to go on.",
    );
    expect(said).not.toHaveTextContent("nothing was lost");
    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
  });

  it("says a chat lost mid-turn failed, with Resume", async () => {
    core(FAILED);
    render(<HostLostNotice plane={PLANE} session={7} onRestart={() => {}} />);
    expect(await notice()).toHaveTextContent(
      "purlis's session host stopped unexpectedly in the middle of this chat's turn, so that turn was lost. Resume starts it again on its conversation.",
    );
    expect(screen.getByRole("button", { name: "Resume" })).toBeInTheDocument();
  });

  it("says a chat that waited for you was hibernated and lost nothing", async () => {
    core(HIBERNATED);
    render(<HostLostNotice plane={PLANE} session={7} onRestart={() => {}} />);
    expect(await notice()).toHaveTextContent(
      "purlis's session host stopped unexpectedly while this chat was waiting for you, so nothing was lost.",
    );
  });

  it("resumes the chat in one click and puts the Notice away", async () => {
    const { pressed } = core(HIBERNATED);
    const restart = vi.fn();
    render(<HostLostNotice plane={PLANE} session={7} onRestart={restart} />);
    await notice();

    await userEvent.click(screen.getByRole("button", { name: "Resume" }));

    expect(restart).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(pressed()).toEqual([
        { cmd: "put_away_lost_chat", args: { plane: PLANE, session: 7 } },
      ]),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("status", { name: "Session host stopped" }),
      ).not.toBeInTheDocument(),
    );
  });

  it("offers no Resume for a chat with no conversation to resume", async () => {
    core(NO_CONVERSATION);
    render(<HostLostNotice plane={PLANE} session={7} onRestart={() => {}} />);
    expect(await notice()).toHaveTextContent("It named no conversation to resume");
    expect(screen.queryByRole("button", { name: "Resume" })).not.toBeInTheDocument();
  });

  it("draws nothing until the chat is lost, and the Notice when the core says so", async () => {
    const { lose } = core(null);
    render(<HostLostNotice plane={PLANE} session={7} onRestart={() => {}} />);
    await act(async () => {});
    expect(screen.queryByRole("status", { name: "Session host stopped" })).not.toBeInTheDocument();

    lose(FAILED);
    await act(async () => {
      await emit(LOST, FAILED);
    });

    expect(await notice()).toHaveTextContent("so that turn was lost");
  });
});
