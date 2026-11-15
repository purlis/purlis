// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { MOVED_JUST_NOW } from "./InboxAsks";
import { TRUST_ALL_ASK, TRUST_CLONE_ASK, MADE } from "./trustAsks.testkit";
import type { CloneToVouch, Shown } from "./bindings";

/**
 * **The trust asks' rows in Asks** (T1-42; R-330, R-337): Trust this clone carries its control
 * on its row, and Trust these clones lists each clone with its remote and when it was made, with
 * Trust all and Choose…. Every answer that trusts waits for its row to settle (ADR 0097).
 */

const PLANE = "/home/dev/plane";

let vouched: unknown[] = [];

beforeEach(() => {
  vouched = [];
  vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ["setTimeout", "clearTimeout"] });
  mockIPC((cmd, args) => {
    if (cmd === "vouch_clone") {
      vouched.push(args);
      return (args as { place: string }).place;
    }
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

const press = userEvent.setup({ advanceTimers: (ms) => vi.advanceTimersByTime(ms) });

/** The person reads what was drawn for longer than an answer waits for its row to settle. */
const read = () => new Promise((done) => setTimeout(done, SETTLE_MS + 50));

/** {@link TRUST_ALL_ASK} with each of its clones as `change` makes it. */
function withClones(change: (clone: CloneToVouch) => CloneToVouch): Shown {
  const clones = TRUST_ALL_ASK.answer.via === "trust-clones" ? TRUST_ALL_ASK.answer.clones : [];
  return { ...TRUST_ALL_ASK, answer: { via: "trust-clones", clones: clones.map(change) } };
}

const rowOf = (source: string) =>
  document.querySelector<HTMLElement>(`[role="row"][data-source="${source}"]`) as HTMLElement;

describe("the Trust this clone row (R-330)", () => {
  it("the Trust this clone row carries its control", async () => {
    render(<Inbox plane={PLANE} asks={[TRUST_CLONE_ASK]} onGo={() => {}} onLeave={() => {}} />);
    const row = rowOf("trust-clone");

    expect(row).toHaveTextContent("Trust this clone so chats can use GitHub");
    // On its row, with nothing behind an Answer.
    expect(within(row).queryByRole("button", { name: /^Answer/ })).toBeNull();
    const trust = within(row).getByRole("button", { name: "Trust this clone" });
    await read();

    await press.click(trust);

    await waitFor(() =>
      expect(vouched).toEqual([
        { plane: PLANE, folder: "workspaces/alpha/api", place: "github.com/acme/api" },
      ]),
    );
  });
});

describe("the Trust these clones row (R-337)", () => {
  it("the clones ask offers Trust all and Choose and lists each remote and time", async () => {
    render(<Inbox plane={PLANE} asks={[TRUST_ALL_ASK]} onGo={() => {}} onLeave={() => {}} />);
    const row = rowOf("trust-clones");

    const clones = within(row).getByRole("list", { name: "Clones purlis did not clone" });
    const each = within(clones).getAllByRole("listitem");
    expect(each).toHaveLength(2);
    expect(each[0]).toHaveTextContent("workspaces/alpha/repos/api");
    expect(each[0]).toHaveTextContent("github.com/acme/api");
    expect(each[1]).toHaveTextContent("workspaces/beta/repos/web");
    expect(each[1]).toHaveTextContent("github.com/acme/web");
    // When each was made, as a time the person reads and a machine can.
    expect(each[0].querySelector("time")?.getAttribute("dateTime")).toBe(
      new Date(MADE.api * 1000).toISOString(),
    );
    expect(each[1].querySelector("time")?.getAttribute("dateTime")).toBe(
      new Date(MADE.web * 1000).toISOString(),
    );
    expect(within(row).getByRole("button", { name: "Trust all" })).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Choose…" })).toBeInTheDocument();
  });

  it("trusts only the clones chosen after Choose…", async () => {
    render(<Inbox plane={PLANE} asks={[TRUST_ALL_ASK]} onGo={() => {}} onLeave={() => {}} />);
    const row = rowOf("trust-clones");
    await read();

    await press.click(within(row).getByRole("button", { name: "Choose…" }));
    const chooser = await screen.findByRole("group", { name: "Choose the clones to trust" });
    await press.click(
      within(chooser).getByRole("checkbox", { name: /workspaces\/beta\/repos\/web/ }),
    );
    await read();
    await press.click(within(row).getByRole("button", { name: "Trust chosen" }));

    await waitFor(() =>
      expect(vouched).toEqual([
        { plane: PLANE, folder: "workspaces/beta/repos/web", place: "github.com/acme/web" },
      ]),
    );
  });

  it("trusts every clone it lists on Trust all", async () => {
    render(<Inbox plane={PLANE} asks={[TRUST_ALL_ASK]} onGo={() => {}} onLeave={() => {}} />);
    const row = rowOf("trust-clones");
    await read();

    await press.click(within(row).getByRole("button", { name: "Trust all" }));

    await waitFor(() =>
      expect(vouched).toEqual([
        { plane: PLANE, folder: "workspaces/alpha/repos/api", place: "github.com/acme/api" },
        { plane: PLANE, folder: "workspaces/beta/repos/web", place: "github.com/acme/web" },
      ]),
    );
  });
});

describe("Choose…'s ticks are the folder and remote ticked (Q274)", () => {
  it("drops a tick when its clone's remote changes under it", async () => {
    const view = render(
      <Inbox plane={PLANE} asks={[TRUST_ALL_ASK]} onGo={() => {}} onLeave={() => {}} />,
    );
    const row = rowOf("trust-clones");
    await read();
    await press.click(within(row).getByRole("button", { name: "Choose…" }));
    const chooser = await screen.findByRole("group", { name: "Choose the clones to trust" });
    await press.click(
      within(chooser).getByRole("checkbox", { name: /workspaces\/beta\/repos\/web/ }),
    );
    const moved = withClones((clone) =>
      clone.folder === "workspaces/beta/repos/web"
        ? { ...clone, place: "github.com/evil/web", places: ["github.com/evil/web"] }
        : clone,
    );

    view.rerender(<Inbox plane={PLANE} asks={[moved]} onGo={() => {}} onLeave={() => {}} />);

    expect(
      within(screen.getByRole("group", { name: "Choose the clones to trust" })).getByRole(
        "checkbox",
        { name: /workspaces\/beta\/repos\/web/ },
      ),
    ).not.toBeChecked();
    await read();
    await press.click(within(rowOf("trust-clones")).getByRole("button", { name: "Trust chosen" }));
    expect(vouched).toEqual([]);
  });
});

describe("when each clone was made (Q274)", () => {
  it("says the day as every age past a month is said", () => {
    render(<Inbox plane={PLANE} asks={[TRUST_ALL_ASK]} onGo={() => {}} onLeave={() => {}} />);

    const each = within(rowOf("trust-clones")).getAllByRole("listitem");

    expect(each[0].querySelector("time")).toHaveTextContent(/^28 Oct 2026$/);
  });

  it("says a clone's creation time is unknown where its disk keeps none", () => {
    const unknown = withClones((clone) => ({ ...clone, made: null }));
    render(<Inbox plane={PLANE} asks={[unknown]} onGo={() => {}} onLeave={() => {}} />);

    const each = within(rowOf("trust-clones")).getAllByRole("listitem");

    expect(each[0]).toHaveTextContent(/creation time unknown$/);
  });
});

describe("the settle guard covers Trust this clone, Trust all and Choose… (ADR 0097)", () => {
  it.each([
    ["trust-clone", "Trust this clone"],
    ["trust-clones", "Trust all"],
    ["trust-clones", "Choose…"],
  ])("holds %s's %s pressed during the settle window, and acts after it", async (source, label) => {
    render(
      <Inbox
        plane={PLANE}
        asks={[source === "trust-clone" ? TRUST_CLONE_ASK : TRUST_ALL_ASK]}
        onGo={() => {}}
        onLeave={() => {}}
      />,
    );
    const row = rowOf(source);
    const way = within(row).getByRole("button", { name: label });

    await press.click(way);

    expect(within(row).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(vouched).toEqual([]);
    expect(screen.queryByRole("group", { name: "Choose the clones to trust" })).toBeNull();

    await read();
    await press.click(way);

    if (label === "Choose…")
      expect(
        await screen.findByRole("group", { name: "Choose the clones to trust" }),
      ).toBeInTheDocument();
    else await waitFor(() => expect(vouched).not.toEqual([]));
  });

  it("holds Trust chosen pressed just after its boxes were drawn", async () => {
    render(<Inbox plane={PLANE} asks={[TRUST_ALL_ASK]} onGo={() => {}} onLeave={() => {}} />);
    const row = rowOf("trust-clones");
    await read();
    await press.click(within(row).getByRole("button", { name: "Choose…" }));
    const chooser = await screen.findByRole("group", { name: "Choose the clones to trust" });
    await press.click(
      within(chooser).getByRole("checkbox", { name: /workspaces\/alpha\/repos\/api/ }),
    );

    await press.click(within(row).getByRole("button", { name: "Trust chosen" }));

    expect(vouched).toEqual([]);
  });
});
