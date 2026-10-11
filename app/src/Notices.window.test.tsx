import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
import type { NotStarted } from "./bindings";
import { countsInStatusBar, IMPORTANCE } from "./Notice";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";
import { findStripNamed } from "./test-strips";

/**
 * **A project's Notices, in its Inbox, against the whole window** (NO-2 #1229, rulings V91i,
 * V91j; #1695: the band under the strip folded into the Inbox).
 *
 * - A Dismiss lasts until the Notice's cause changes: it is kept on this machine, in the layout
 *   file beside the window's other preferences, so a relaunch keeps it; and it clears itself
 *   once the core answers without the cause, so a return shows the Notice again.
 * - Every Notice is listed in the Inbox, the most important first.
 *
 * A relaunch is simulated the way the app does one: what the window last wrote to the layout
 * file is handed to the next window as it is created (`windowprefs.ts`).
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";

type Asked = { cmd: string; args: unknown };

/** One chat as the core reports it. */
function chat(
  session: number,
  name: string,
  said: { resumed?: string; fresh?: string; guessed?: string } = {},
) {
  return {
    session,
    name,
    cwd: `${PLANE}/workspaces/alpha`,
    harness: said.fresh ? "claude" : null,
    in_front: session === 1,
    resumed: said.resumed ?? null,
    fresh: said.fresh ?? null,
    profile: null,
    persona: null,
    unreported: null,
    guessed: said.guessed ?? null,
    pinned: false,
  };
}

/**
 * **The machine's layout file**, as the core keeps it: the window's layout as it last wrote it,
 * and the dismissals, which only `set_dismissed` writes, one project at a time, and which a
 * `write_layout` leaves as they are (`purlis_core::windowprefs`).
 */
const disk: { layout?: Record<string, unknown>; dismissed: Record<string, string[]> } = {
  dismissed: {},
};

/**
 * The core: a project with `alpha`, the chats it puts back, the pins whose workspace is gone,
 * and the chats that would not start. `pins` makes the store's read never answer, refuse, or
 * answer without being sure; `chats` makes the read of the chats put back refuse.
 */
function core(
  open: ReturnType<typeof chat>[] = [chat(1, "one")],
  world: {
    gone: string[];
    wouldNotStart?: NotStarted[];
    pins?: "hangs" | "refuses" | "unsure";
    chats?: "refuses";
  } = { gone: [] },
): { asked: Asked[] } {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [
          {
            name: "alpha",
            path: `${PLANE}/workspaces/alpha`,
            vision: "",
            todos: [],
            chats: open,
          },
        ],
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "opened_chats") {
      if (world.chats === "refuses") throw "the project's record could not be read";
      return open;
    }
    if (cmd === "chat_states") return [];
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    if (cmd === "chats_that_would_not_start") return world.wouldNotStart ?? [];
    if (cmd === "running_sessions") return [];
    if (cmd === "plane_pins") {
      if (world.pins === "hangs") return new Promise(() => undefined);
      if (world.pins === "refuses") throw "the machine store could not be read";
      // `pins_in` with a rename in flight, or `workspaces/` not there: it names nothing gone
      // and says it is not sure.
      if (world.pins === "unsure")
        return { project: false, workspaces: [], missing: [], order: world.gone, certain: false };
      return {
        project: false,
        workspaces: [],
        missing: [...world.gone],
        order: [...world.gone],
        certain: true,
      };
    }
    if (cmd === "write_layout") {
      // The core keeps the file's dismissals over any a window sends.
      const layout = JSON.parse(given.text as string) as Record<string, unknown>;
      disk.layout = Object.fromEntries(
        Object.entries(layout).filter(([field]) => field !== "dismissed"),
      );
      return null;
    }
    if (cmd === "pin_workspace" && given.pinned === false) {
      world.gone = world.gone.filter((one) => one !== given.workspace);
      return null;
    }
    if (cmd === "set_dismissed") {
      const causes = given.causes as string[];
      const others = Object.entries(disk.dismissed).filter(([one]) => one !== given.plane);
      disk.dismissed = Object.fromEntries(
        causes.length > 0 ? [...others, [given.plane as string, causes]] : others,
      );
      return null;
    }
    return null;
  });
  return { asked };
}

/** Quits, and launches again on the layout file as the core left it, handed to the next window
 *  as it is created. */
function relaunch() {
  cleanup();
  clearMocks();
  forgetThisLaunch();
  const found = disk.layout !== undefined || Object.keys(disk.dismissed).length > 0;
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: {
      path: "layout.json",
      found,
      document: found
        ? {
            ...(disk.layout ?? { version: 1, regions: [] }),
            ...(Object.keys(disk.dismissed).length > 0
              ? { dismissed: structuredClone(disk.dismissed) }
              : {}),
          }
        : null,
      trouble: null,
    },
    theme: { path: "theme.json", found: false, document: null, trouble: null },
  };
}

/** What the window asked the core to keep for this project, last. */
const keptLast = (asks: Asked[]) =>
  (asks.filter((one) => one.cmd === "set_dismissed").at(-1)?.args as { causes?: string[] })?.causes;

beforeEach(() => {
  disk.layout = undefined;
  disk.dismissed = {};
  forgetThisLaunch();
});

afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

/** The Inbox's Notices, top to bottom, by cause. */
const listed = () =>
  [...document.querySelectorAll(".notice-list [data-cause]")].map((one) =>
    one.getAttribute("data-cause"),
  );
/** The Inbox, shown: pressed only where it is not, since a press on the view in front puts it
 *  away. */
const inbox = async () => {
  if (screen.queryByRole("tabpanel", { name: "Inbox" }) === null)
    await userEvent.click(
      within(await screen.findByRole("tablist", { name: "Attention" })).getByRole("tab", {
        name: "Inbox",
      }),
    );
  return screen.findByRole("tabpanel", { name: "Inbox" });
};
const dormant = (name: string) => screen.findByText(new RegExp(`^${name} is gone, kept dormant`));
const noticeOf = (text: HTMLElement) => text.closest("[data-cause]") as HTMLElement;
const dismiss = async (text: HTMLElement) => {
  await inbox();
  await userEvent.click(within(noticeOf(text)).getByRole("button", { name: "Dismiss" }));
};
/** Lets every answer in flight land. */
const settle = () => new Promise((done) => setTimeout(done, 50));

describe("a dismissed Notice", () => {
  it("stays dismissed after a relaunch on this machine", async () => {
    const { asked } = core([chat(1, "one")], { gone: ["delta"] });
    render(<App />);
    await dismiss(await dormant("delta"));
    expect(screen.queryByText(/^delta is gone/)).toBeNull();
    await waitFor(() => expect(keptLast(asked)).toBeDefined());

    relaunch();
    core([chat(1, "one")], { gone: ["delta"] });
    render(<App />);
    await findStripNamed("Workspaces");
    await settle();

    expect(screen.queryByText(/^delta is gone/)).toBeNull();
  });

  it("shows again once its cause has gone and come back", async () => {
    // Launch one: dismissed. Launch two: the workspace is back, so the store answers without the
    // pin, and the dismissal is let go. Launch three: gone again, and the Notice is drawn.
    let { asked } = core([chat(1, "one")], { gone: ["delta"] });
    render(<App />);
    await dismiss(await dormant("delta"));
    await waitFor(() => expect(keptLast(asked)).toBeDefined());

    relaunch();
    ({ asked } = core([chat(1, "one")], { gone: [] }));
    render(<App />);
    await findStripNamed("Workspaces");
    await waitFor(() => expect(keptLast(asked)).toEqual([]));
    expect(disk.dismissed).toEqual({});

    relaunch();
    core([chat(1, "one")], { gone: ["delta"] });
    render(<App />);
    expect(await dormant("delta")).toBeInTheDocument();
  });

  it("is not let go by a launch whose store did not answer, or was not sure", async () => {
    // Before the store answers, when it cannot, and when it answers without being sure (a
    // rename between its steps, `workspaces/` not there), the window holds no pins at all: that
    // is not knowing, not the pin having gone, so nothing is let go (D-NO2-10).
    let { asked } = core([chat(1, "one")], { gone: ["delta"] });
    render(<App />);
    await dismiss(await dormant("delta"));
    await waitFor(() => expect(keptLast(asked)).toBeDefined());

    for (const pins of ["hangs", "refuses", "unsure"] as const) {
      relaunch();
      ({ asked } = core([chat(1, "one")], { gone: ["delta"], pins }));
      render(<App />);
      await findStripNamed("Workspaces");
      await settle();
    }

    relaunch();
    core([chat(1, "one")], { gone: ["delta"] });
    render(<App />);
    await findStripNamed("Workspaces");
    await settle();
    expect(screen.queryByText(/^delta is gone/)).toBeNull();
  }, 20_000); // Five launches: given the time five take on a loaded machine.

  it("about what a relaunch did to a chat stays dismissed while it keeps doing it", async () => {
    const fresh = [chat(1, "one", { fresh: "its conversation was not found" })];
    const { asked } = core(fresh, { gone: [] });
    render(<App />);
    const note = await screen.findByText(/came back as a new chat/);
    expect(noticeOf(note).getAttribute("data-cause")).toBe("chat-fresh:1");
    await dismiss(note);
    await waitFor(() => expect(keptLast(asked)).toBeDefined());

    relaunch();
    core(fresh, { gone: [] });
    render(<App />);
    await findStripNamed("Workspaces");
    await settle();
    expect(screen.queryByText(/came back as a new chat/)).toBeNull();
  });

  it("about a chat is not let go by a launch that could not read the chats", async () => {
    const fresh = [chat(1, "one", { fresh: "its conversation was not found" })];
    const { asked } = core(fresh, { gone: [] });
    render(<App />);
    await dismiss(await screen.findByText(/came back as a new chat/));
    await waitFor(() => expect(keptLast(asked)).toEqual(["chat-fresh:1"]));

    relaunch();
    core(fresh, { gone: [], chats: "refuses" });
    render(<App />);
    await findStripNamed("Workspaces");
    await settle();

    relaunch();
    core(fresh, { gone: [] });
    render(<App />);
    await findStripNamed("Workspaces");
    await settle();
    expect(screen.queryByText(/came back as a new chat/)).toBeNull();
  });

  it("about a chat is let go once it comes back otherwise, and shows when it returns", async () => {
    const fresh = [chat(1, "one", { fresh: "its conversation was not found" })];
    let { asked } = core(fresh, { gone: [] });
    render(<App />);
    await dismiss(await screen.findByText(/came back as a new chat/));
    await waitFor(() => expect(keptLast(asked)).toEqual(["chat-fresh:1"]));

    relaunch();
    ({ asked } = core([chat(1, "one")], { gone: [] }));
    render(<App />);
    await waitFor(() => expect(keptLast(asked)).toEqual([]));

    relaunch();
    core(fresh, { gone: [] });
    render(<App />);
    expect(await screen.findByText(/came back as a new chat/)).toBeInTheDocument();
  });

  it("about a resumed chat is kept by its conversation, so another one shows", async () => {
    // D-NO2-9: the occurrence is the conversation the chat was resumed by.
    const resumed = (conversation: string) => [chat(1, "one", { resumed: conversation })];
    const { asked } = core(resumed("conv-a"), { gone: [] });
    render(<App />);
    const note = await screen.findByText(/was resumed/);
    expect(noticeOf(note).getAttribute("data-cause")).toBe("chat-resumed:1:conv-a");
    await dismiss(note);
    await waitFor(() => expect(keptLast(asked)).toEqual(["chat-resumed:1:conv-a"]));

    relaunch();
    core(resumed("conv-a"), { gone: [] });
    render(<App />);
    await findStripNamed("Workspaces");
    await settle();
    expect(screen.queryByText(/was resumed/)).toBeNull();

    relaunch();
    core(resumed("conv-b"), { gone: [] });
    render(<App />);
    expect(await screen.findByText(/was resumed/)).toBeInTheDocument();
  });

  it("is kept beside another window's, in another project", async () => {
    // D-NO2-1 as amended: each window writes only its own project's list, through the core,
    // and never the whole map it read at launch.
    const { asked } = core([chat(1, "one")], { gone: ["delta"] });
    render(<App />);
    await dormant("delta");
    // Another window, on another project, dismissed something after this one launched.
    disk.dismissed["/home/dev/other"] = ["pin-dormant:theirs"];
    await dismiss(await dormant("delta"));
    await waitFor(() => expect(keptLast(asked)).toEqual(["pin-dormant:delta"]));

    const kept = asked.filter((one) => one.cmd === "set_dismissed");
    expect(kept.every((one) => (one.args as { plane: string }).plane === PLANE)).toBe(true);
    const layouts = asked.filter((one) => one.cmd === "write_layout");
    expect(layouts.some((one) => (one.args as { text: string }).text.includes("dismissed"))).toBe(
      false,
    );
    expect(disk.dismissed).toEqual({
      [PLANE]: ["pin-dormant:delta"],
      "/home/dev/other": ["pin-dormant:theirs"],
    });
  });

  it("about an event is not kept: the next occurrence shows", async () => {
    // A chat that did not start is an event of this launch; its Dismiss ends that occurrence.
    core([chat(1, "one")], {
      gone: [],
      wouldNotStart: [{ id: "two", name: "two", why: "no profile", approval: null }],
    });
    render(<App />);
    await dismiss(await screen.findByText(/did not start/));
    await settle();

    relaunch();
    core([chat(1, "one")], {
      gone: [],
      wouldNotStart: [{ id: "two", name: "two", why: "no profile", approval: null }],
    });
    render(<App />);
    expect(await screen.findByText(/did not start/)).toBeInTheDocument();
  });
});

describe("the Notices in the Inbox (#1695)", () => {
  /** Four Notices: trouble (a chat that did not start), two dormant pins, and news about a chat
   *  that came back new — drawn by the window in another order than their importance. */
  const four = () => {
    core([chat(1, "one", { fresh: "its conversation was not found" })], {
      gone: ["able", "baker"],
      wouldNotStart: [{ id: "two", name: "two", why: "no profile", approval: null }],
    });
    render(<App />);
  };

  it("are every one listed, the most important first, with no +N more", async () => {
    four();
    const shown = await inbox();
    await waitFor(() =>
      expect(listed()).toEqual([
        "chat-did-not-start:two",
        "pin-dormant:able",
        "pin-dormant:baker",
        "chat-fresh:1",
      ]),
    );
    expect(within(shown).getByRole("region", { name: "Notices" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /more$/ })).toBeNull();
    // Each is a whole Notice, with its ways out.
    const listedOne = document.querySelector('.notice-list [data-cause="pin-dormant:baker"]');
    expect(within(listedOne as HTMLElement).getByRole("button", { name: "Forget" })).toBeTruthy();
    // And nothing stands under the strip any more.
    expect(document.querySelector(".notice-band-stack")).toBeNull();
  });

  it("keep the focus where it is when a new one arrives", async () => {
    // F2: a Notice already in its place is not moved, so a button the operator is on keeps
    // the focus while another arrives below it.
    core([chat(1, "one", { fresh: "its conversation was not found" })], {
      gone: ["able"],
      wouldNotStart: [{ id: "two", name: "two", why: "no profile", approval: null }],
    });
    render(<App />);
    await inbox();
    const trouble = noticeOf(await screen.findByText(/did not start/));
    const theirs = within(trouble).getByRole("button", { name: "Dismiss" });
    theirs.focus();

    fireEvent.click(
      within(noticeOf(await dormant("able"))).getByRole("button", { name: "Forget" }),
    );
    await screen.findByText(/^Forgot the pin to able/);

    expect(listed().slice(0, 2)).toEqual(["chat-did-not-start:two", "pin-forgotten:able"]);
    expect(document.activeElement).toBe(theirs);
  });

  it("move up as one is dismissed", async () => {
    four();
    await inbox();
    await dormant("able");

    await dismiss(await dormant("able"));
    expect(listed()).toEqual(["chat-did-not-start:two", "pin-dormant:baker", "chat-fresh:1"]);

    await dismiss(await screen.findByText(/did not start/));
    expect(listed()).toEqual(["pin-dormant:baker", "chat-fresh:1"]);
  });

  it("bring the Inbox on screen when one answers the person's own press", async () => {
    // D-1695-3: Forget's answer, its Undo, is said where the person sees it.
    core([chat(1, "one")], { gone: ["able"] });
    render(<App />);
    await dormant("able");
    expect(screen.queryByRole("tabpanel", { name: "Inbox" })).toBeNull();

    fireEvent.click(
      within(noticeOf(await dormant("able"))).getByRole("button", { name: "Forget", hidden: true }),
    );

    expect(await screen.findByRole("tabpanel", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
  });
});

describe("the status bar", () => {
  it("counts the Inbox's Notices, but none the doctor's own button counts, and opens the Inbox", async () => {
    // A Notice the doctor also found is counted by the doctor's button alone (V91i).
    // A chat that cannot say it waits is the faint hand's to stand for (#1695).
    expect(IMPORTANCE.filter((family) => !countsInStatusBar(family))).toEqual(["chat-quiet"]);
    expect(countsInStatusBar("doctor-finding:hooks")).toBe(false);
    core([chat(1, "one", { fresh: "its conversation was not found" })], {
      gone: ["able", "baker"],
      wouldNotStart: [{ id: "two", name: "two", why: "no profile", approval: null }],
    });
    render(<App />);

    const button = await screen.findByRole("button", { name: "Notices: 4" });
    await userEvent.click(button);

    expect(await screen.findByRole("tabpanel", { name: "Inbox" })).toBeInTheDocument();
  });
});
