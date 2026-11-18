// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatsReconnecting, RunnerLinkDown } from "./bindings";
import { RECONNECTING } from "./reconnecting";
import { QUIET_FOR_MS, RunnerLinkDownNotice } from "./RunnerLinkDownNotice";

/**
 * **A runner's link that stays down past a short while is said in a window Notice** (#7134,
 * ADR 0078 as amended 2026-11-18): its panes say Reconnecting… quietly at first; past
 * {@link QUIET_FOR_MS} the window says why, in the core's words, with Try now and Check pool, and that purlis
 * keeps linking to it again. It goes once the link is back.
 */

const PLANE = "/planes/one";
const WHY =
  "the connector did not connect: Docker on 45.140.203.77 didn't answer through SSH (it said: docker info failed)";

afterEach(() => {
  vi.useRealTimers();
  cleanup();
  clearMocks();
});

/** What the faked core was asked to link again now, by pool. */
let triedNow: string[] = [];

/** The core, faked at its command boundary: which runners' links are down now. */
function aCore(down: () => RunnerLinkDown[]) {
  triedNow = [];
  mockIPC(
    (cmd, args) => {
      if (cmd === "runner_links_down") return down();
      if (cmd === "runner_link_now") triedNow.push((args as { pool: string }).pool);
      return null;
    },
    { shouldMockEvents: true },
  );
}

const told = (said: Partial<ChatsReconnecting>) =>
  act(async () => {
    await emit(RECONNECTING, {
      plane: PLANE,
      sessions: [1],
      reconnection: "reconnecting",
      ...said,
    } satisfies ChatsReconnecting);
  });

const notice = () => document.querySelector(`[data-cause^="runner-link-down:${PLANE}"]`);
const nowSecs = () => Math.floor(Date.now() / 1000);

describe("a runner whose link stays down", () => {
  it("says nothing while it is a blip, then why, with Check pool, until it is back", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let down: RunnerLinkDown[] = [];
    aCore(() => down);
    const onCheckPool = vi.fn();
    render(<RunnerLinkDownNotice plane={PLANE} onCheckPool={onCheckPool} />);

    down = [{ pool: "docker", why: "the link to it ended", since: nowSecs(), state: "unreachable" }];
    await told({});
    expect(notice()).toBeNull();

    // A try to link again found another reason, still within the quiet while.
    down = [{ ...down[0], why: WHY }];
    await told({});
    expect(notice()).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(QUIET_FOR_MS + 1000);
    });
    await waitFor(() => expect(notice()).not.toBeNull());
    expect(notice()?.textContent).toContain("Docker on 45.140.203.77 didn't answer through SSH");
    expect(notice()?.textContent).toContain("purlis keeps linking to it again");
    await userEvent.click(screen.getByRole("button", { name: "Check pool docker" }));
    expect(onCheckPool).toHaveBeenCalledWith("docker");
    await userEvent.click(screen.getByRole("button", { name: "Try now" }));
    await waitFor(() => expect(triedNow).toEqual(["docker"]));

    down = [];
    await told({ reconnection: "reconnected" });
    await waitFor(() => expect(notice()).toBeNull());
  });

  it("says a refused pool at once, without Try now, since purlis stopped trying (#7187)", async () => {
    aCore(() => [
      { pool: "docker", why: "sign-in was refused", since: nowSecs(), state: "refused" },
    ]);
    const onCheckPool = vi.fn();
    render(<RunnerLinkDownNotice plane={PLANE} onCheckPool={onCheckPool} />);
    await waitFor(() => expect(notice()).not.toBeNull());
    expect(notice()?.textContent).toContain("purlis stopped trying");
    expect(notice()?.textContent).toContain("sign-in was refused");
    expect(screen.queryByRole("button", { name: "Try now" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Check pool docker" }));
    expect(onCheckPool).toHaveBeenCalledWith("docker");
  });

  it("is said at once to a window drawn long into the outage", async () => {
    aCore(() => [{ pool: "docker", why: WHY, since: nowSecs() - 6 * 60, state: "unreachable" }]);
    render(<RunnerLinkDownNotice plane={PLANE} onCheckPool={() => {}} />);
    await waitFor(() => expect(notice()).not.toBeNull());
    expect(notice()?.textContent).toContain("6 minutes");
  });

  it("is put away by Dismiss until the reason changes", async () => {
    let down: RunnerLinkDown[] = [{ pool: "docker", why: WHY, since: nowSecs() - 60, state: "unreachable" }];
    aCore(() => down);
    render(<RunnerLinkDownNotice plane={PLANE} onCheckPool={() => {}} />);
    await waitFor(() => expect(notice()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(notice()).toBeNull());

    await told({});
    expect(notice()).toBeNull();

    down = [{ ...down[0], why: "the connector did not connect: ssh: connection refused" }];
    await told({});
    await waitFor(() => expect(notice()).not.toBeNull());
  });

  it("is said again for the next outage after Dismiss once the link was back", async () => {
    let down: RunnerLinkDown[] = [{ pool: "docker", why: WHY, since: nowSecs() - 60, state: "unreachable" }];
    aCore(() => down);
    render(<RunnerLinkDownNotice plane={PLANE} onCheckPool={() => {}} />);
    await waitFor(() => expect(notice()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(notice()).toBeNull());

    down = [];
    await told({ reconnection: "reconnected" });
    down = [{ pool: "docker", why: WHY, since: nowSecs() - 60, state: "unreachable" }];
    await told({});
    await waitFor(() => expect(notice()).not.toBeNull());
  });

  it("reads again for its own project only", async () => {
    let reads = 0;
    aCore(() => {
      reads += 1;
      return [];
    });
    render(<RunnerLinkDownNotice plane={PLANE} onCheckPool={() => {}} />);
    await waitFor(() => expect(reads).toBe(1));
    await told({ plane: "/planes/two" });
    expect(reads).toBe(1);
    await told({});
    await waitFor(() => expect(reads).toBe(2));
  });
});
