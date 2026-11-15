// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  FAILING,
  HostFailingNotice,
  NOT_IN_THE_BACKGROUND,
  NotInTheBackgroundNotice,
} from "./HostFailingNotice";
import type { Failing, NotInTheBackground } from "./bindings";

/**
 * The window says its session host is failing once the supervisor's breaker trips (#2007): the
 * exits that tripped it, the host's last lines, and Try again.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const THREE: Failing = {
  exits: 3,
  within_minutes: 5,
  said: ["purlisd: the session host could not bind its socket"],
  stopped_by: "purlis",
};

function core(
  failing: Failing | null,
  notInTheBackground: NotInTheBackground | null = null,
): { asked: string[] } {
  const asked: string[] = [];
  mockIPC(
    (cmd) => {
      asked.push(cmd);
      if (cmd === "session_host_failing") return failing;
      if (cmd === "session_host_not_in_the_background") return notInTheBackground;
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked };
}

const notice = () => screen.findByRole("status", { name: "Session host failing" });

describe("the session host failing Notice", () => {
  it("says the breaker tripped, with the host's last lines and Try again", async () => {
    core(THREE);
    render(<HostFailingNotice />);
    expect(await notice()).toHaveTextContent(
      "purlis's session host stopped unexpectedly 3 times in 5 minutes, so purlis stopped starting it again. No chat starts until it runs.",
    );
    expect(screen.getByText("Its last log lines")).toBeInTheDocument();
    expect(screen.getByText(/could not bind its socket/)).not.toBeVisible();
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });

  it("says the system stopped starting it when the limit was the system's", async () => {
    core({ ...THREE, stopped_by: "system" });
    render(<HostFailingNotice />);
    expect(await notice()).toHaveTextContent(
      "Your system stopped starting purlis's session host after it stopped unexpectedly 3 times in 5 minutes. No chat starts until it runs.",
    );
  });

  it("starts the host again on Try again", async () => {
    const { asked } = core(THREE);
    render(<HostFailingNotice />);
    await notice();

    await userEvent.click(screen.getByRole("button", { name: "Try again" }));

    await waitFor(() => expect(asked).toContain("start_the_session_host_again"));
  });

  it("draws nothing while the host runs, and comes and goes as the core says", async () => {
    core(null);
    render(<HostFailingNotice />);
    await act(async () => {});
    expect(screen.queryByRole("status", { name: "Session host failing" })).not.toBeInTheDocument();

    await act(async () => {
      await emit(FAILING, THREE);
    });
    await notice();
    await act(async () => {
      await emit(FAILING, null);
    });

    await waitFor(() =>
      expect(
        screen.queryByRole("status", { name: "Session host failing" }),
      ).not.toBeInTheDocument(),
    );
  });
});

const REFUSED: NotInTheBackground = {
  why: "`launchctl bootstrap gui/501 …` failed: Input/output error",
};

const backgroundNotice = () =>
  screen.findByRole("status", { name: "Not running in the background" });

describe("the not running in the background Notice", () => {
  it("says in plain words what the person loses and what to check, with the system's words folded away", async () => {
    core(null, REFUSED);
    render(<NotInTheBackgroundNotice />);
    expect(await backgroundNotice()).toHaveTextContent(
      "purlis couldn't set itself up to start when you log in, so your chats keep running only until you log out or restart. Check that purlis is allowed to run in the background in your system settings (on a Mac: System Settings, General, Login Items). purlis tries again the next time it starts.",
    );
    expect(screen.getByText("Details")).toBeInTheDocument();
    expect(screen.getByText(/Input\/output error/)).not.toBeVisible();
  });

  it("comes when the core says so, and goes on Dismiss", async () => {
    core(null);
    render(<NotInTheBackgroundNotice />);
    await act(async () => {});
    expect(
      screen.queryByRole("status", { name: "Not running in the background" }),
    ).not.toBeInTheDocument();

    await act(async () => {
      await emit(NOT_IN_THE_BACKGROUND, REFUSED);
    });
    await backgroundNotice();
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));

    await waitFor(() =>
      expect(
        screen.queryByRole("status", { name: "Not running in the background" }),
      ).not.toBeInTheDocument(),
    );
  });
});
