import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { InboxAlerts } from "./InboxAlerts";
import { NoticeList } from "./Notice";
import type { AlertRow, DoctorFixed, PlaneAlerts } from "./bindings";
import { forgetThisLaunch, settleLayout } from "./regions";
import {
  forgetTheirTheme,
  GLOBAL,
  sayAboutThisMachine,
  theirThemeOnce,
  useAboutThisMachine,
} from "./windowprefs";

/**
 * **Each alert, a Notice in the Inbox, carries its way out** (NO-6, #1238; #1695: the Alerts
 * drawer folded into the Inbox): a Settings link, a fix, another project, the Saving view —
 * never a command to type somewhere else. Driven from the window the way a person drives it: the
 * status line's Notices button, then the row's own button, and read back from what is on screen
 * and what the core was asked. The core is the mock: which alert has which way out is
 * `alerts.rs`'s to test.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";
const OUTER = "/home/dev/outer";

const FILE = (which: "shared" | "local") => ({
  which,
  file: which === "shared" ? "charter.toml" : "charter.local.toml",
  exists: which === "shared",
  text: "",
  refusals: [],
  parsed: true,
  fields: [],
});

const FRONT_DOOR: AlertRow = {
  severity: "warn",
  subject: "front door",
  detail: "ghost — no such persona",
  way: { kind: "settings", group: "project.general", setting: "project.general.persona.default" },
};
const PIN: AlertRow = {
  severity: "warn",
  subject: "charter",
  detail: "this charter is 2.0.0, and the plane pins 1.0.0",
  way: { kind: "settings", group: "project.general", setting: "project.general.charter.version" },
};
const REINIT: AlertRow = {
  severity: "warn",
  subject: "reinit",
  detail: "1 workspace is behind the current layout: ide",
  way: { kind: "fix", id: "workspace-reinit" },
};
const IDENTITY: AlertRow = {
  severity: "warn",
  subject: "git identity",
  detail: "not set: user.email",
  way: { kind: "fix", id: "git-identity" },
};
const NESTED: AlertRow = {
  severity: "bad",
  subject: "nested plane",
  detail: "memory and vault go to ~/outer/workspaces/plane, not ~/outer",
  way: { kind: "open-project", path: OUTER },
};
const ROOT: AlertRow = {
  severity: "warn",
  subject: "plane root",
  detail: "plane · detached HEAD",
  way: { kind: "saving" },
};
const SAVE: AlertRow = {
  severity: "bad",
  subject: "save",
  detail: "the plane's save is blocked: a conflict",
  way: { kind: "saving" },
};

let rows: AlertRow[] = [];
/** Another project's reading, for the line that names it; none by default. */
let others: PlaneAlerts[] = [];
/** What the reading answers instead, where it fails. */
let readingFails: string | undefined;
let fixAnswer: DoctorFixed | string = { fix: "", refused: null, said: [], complete: true };
let asked: { cmd: string; args: unknown }[] = [];

let stopped: string | null = null;
const reading = (): PlaneAlerts[] => [{ plane: PLANE, alerts: rows, stopped }, ...others];
const calls = (cmd: string) => asked.filter((one) => one.cmd === cmd);

function theme(trouble: string | null) {
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/op/.config/charter/layout.json", found: false, document: null },
    theme: {
      path: "/home/op/.config/charter/theme.json",
      found: trouble !== null,
      document: null,
      trouble,
    },
  };
}

beforeEach(() => {
  forgetThisLaunch();
  forgetTheirTheme();
  // What `main.tsx` says about this machine at a launch, unsaid between tests.
  for (const subject of ["theme", "layout", "dismissed"]) sayAboutThisMachine(subject, undefined);
  rows = [];
  others = [];
  stopped = null;
  readingFails = undefined;
  asked = [];
  fixAnswer = { fix: "workspace-reinit", refused: null, said: ["✓ ide: healed"], complete: true };
  theme(null);
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [];
      if (cmd === "reopened_views") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "chat_states") return [];
      if (cmd === "plane_sidebar")
        return { root: PLANE, personas: [], persona: null, unfiled: [], workspaces: [] };
      if (cmd === "project_settings") return { shared: FILE("shared"), local: FILE("local") };
      if (cmd === "alerts_everywhere") {
        if (readingFails !== undefined) throw readingFails;
        return reading();
      }
      if (cmd === "plane_doctor_fix") {
        if (typeof fixAnswer === "string") throw new Error(fixAnswer);
        // Once it ran whole, the next reading has nothing left to say about it.
        if (fixAnswer.complete) rows = rows.filter((one) => one !== REINIT);
        return fixAnswer;
      }
      if (cmd === "plane_doctor_identity") return { name: "Bea Terminal", email: "" };
      if (cmd === "plane_doctor_fix_identity") {
        rows = rows.filter((one) => one !== IDENTITY);
        return {
          kind: "fixed",
          fixed: {
            fix: "git-identity",
            refused: null,
            said: ["✓ set user.email = bea@example.invalid (global git config)"],
            complete: true,
          },
        };
      }
      if (cmd === "open_plane") return { plane: null, ask: null };
      if (cmd === "use_built_in_theme") return "/home/op/.config/charter/theme.aside.json";
      if (cmd === "use_default_layout") return "/home/op/.config/charter/layout.aside.json";
      return null;
    },
    { shouldMockEvents: true },
  );
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

/** The window, with its first project open, and its Inbox opened from the status line's
 *  Notices button. */
async function drawer() {
  render(<App />);
  await screen.findByRole("tab", { name: /plane/ });
  await userEvent.click(await screen.findByRole("button", { name: /^Notices/ }));
  return screen.findByRole("tabpanel", { name: "Inbox" });
}

/** The Inbox's Notices. */
const project = (open: HTMLElement) => within(open).getByRole("region", { name: "Notices" });
const row = (open: HTMLElement, cause: string) => {
  const found = open.querySelector(`[data-cause="${cause}"]`);
  expect(found, cause).not.toBeNull();
  return found as HTMLElement;
};
/** Whether a row of `cause` is listed. */
const listed = (cause: string) =>
  document.querySelector(`.notice-list [data-cause="${cause}"]`) !== null;

describe("each alert in the Inbox", () => {
  it("carries its way out, and no command to type", async () => {
    rows = [PIN, FRONT_DOOR, REINIT, NESTED, ROOT, SAVE];
    const open = await drawer();

    const ways = (cause: string) =>
      within(row(open, cause))
        .getAllByRole("button")
        .map((one) => one.textContent);
    expect(ways("alert:charter")).toEqual(["Fix it in Settings"]);
    expect(ways("alert:front door")).toEqual(["Fix it in Settings"]);
    expect(ways("alert:reinit")).toEqual(["Reinit"]);
    expect(ways("alert:nested plane")).toEqual(["Open the outer project"]);
    expect(ways("alert:plane root")).toEqual(["Go to Saving"]);
    expect(ways("alert:save")).toEqual(["Go to Saving"]);
    expect(project(open).querySelector("code")).toBeNull();
    expect(project(open)).not.toHaveTextContent("purlis persona default");
  });

  it("opens the project's General settings, where the default persona's picker is, for the front door", async () => {
    rows = [FRONT_DOOR];
    const open = await drawer();

    await userEvent.click(
      within(row(open, "alert:front door")).getByRole("button", { name: "Fix it in Settings" }),
    );

    expect(await screen.findByRole("radio", { name: "Project" })).toBeChecked();
    const groups = screen.getByRole("navigation", { name: "Groups" });
    await waitFor(() =>
      expect(within(groups).getByRole("button", { name: "General" })).toHaveAttribute(
        "aria-current",
        "true",
      ),
    );
    // The settings the core's rows name (#1289) are controls this group draws: the default
    // persona's picker and the version lock, by the ids the Settings builders give them.
    for (const setting of [FRONT_DOOR, PIN].map((one) =>
      one.way.kind === "settings" ? one.way.setting : null,
    ))
      expect(document.querySelector(`[data-setting="${setting}"]`), setting ?? "").not.toBeNull();
  });

  it("reinits the workspaces behind the layout through the doctor's fix, then reads again", async () => {
    rows = [REINIT];
    const open = await drawer();
    const before = calls("alerts_everywhere").length;

    await userEvent.click(
      within(row(open, "alert:reinit")).getByRole("button", { name: "Reinit" }),
    );

    await waitFor(() =>
      expect(calls("plane_doctor_fix").map((one) => one.args)).toEqual([
        { plane: PLANE, fix: "workspace-reinit" },
      ]),
    );
    await waitFor(() => expect(calls("alerts_everywhere").length).toBeGreaterThan(before));
    await waitFor(() => expect(listed("alert:reinit")).toBe(false));
  });

  it("says why a fix was refused, on its row", async () => {
    rows = [REINIT];
    fixAnswer = {
      fix: "workspace-reinit",
      refused: "this purlis may not write the project",
      said: [],
      complete: false,
    };
    const open = await drawer();

    await userEvent.click(
      within(row(open, "alert:reinit")).getByRole("button", { name: "Reinit" }),
    );

    await waitFor(() =>
      expect(row(open, "alert:reinit")).toHaveTextContent(
        "purlis could not reinit: this purlis may not write the project",
      ),
    );
  });

  it("opens a fix's form for a fix that takes input, rather than applying it bare (#1301)", async () => {
    rows = [IDENTITY];
    const open = await drawer();
    const before = calls("alerts_everywhere").length;

    const fix = within(row(open, "alert:git identity")).getByRole("button", { name: "Fix" });
    await userEvent.click(fix);
    const form = await within(open).findByRole("form", { name: "Git identity" });
    // Pressing it applied nothing: the fix needs a name and an email first.
    expect(calls("plane_doctor_fix")).toEqual([]);
    expect(calls("plane_doctor_fix_identity")).toEqual([]);
    expect(fix).toHaveAttribute("aria-expanded", "true");
    expect(fix).toHaveAttribute("aria-controls", form.id);
    await waitFor(() => expect(within(form).getByLabelText("Name")).toHaveValue("Bea Terminal"));

    const email = within(form).getByLabelText("Email");
    await waitFor(() => expect(email).toHaveFocus());
    await userEvent.type(email, "bea@example.invalid", { skipClick: true });
    await userEvent.click(within(form).getByRole("button", { name: "Set identity" }));

    await waitFor(() =>
      expect(calls("plane_doctor_fix_identity").map((one) => one.args)).toEqual([
        { plane: PLANE, name: "", email: "bea@example.invalid" },
      ]),
    );
    expect(calls("plane_doctor_fix")).toEqual([]);
    await waitFor(() => expect(calls("alerts_everywhere").length).toBeGreaterThan(before));
    await waitFor(() => expect(listed("alert:git identity")).toBe(false));
  });

  it("opens the outer project through the window's one way in", async () => {
    rows = [NESTED];
    const open = await drawer();

    await userEvent.click(
      within(row(open, "alert:nested plane")).getByRole("button", {
        name: "Open the outer project",
      }),
    );

    await waitFor(() =>
      expect(calls("open_plane").map((one) => one.args)).toContainEqual({ path: OUTER }),
    );
  });

  it("opens the project's Saving view for a plane root being worked in", async () => {
    rows = [ROOT];
    const open = await drawer();

    await userEvent.click(
      within(row(open, "alert:plane root")).getByRole("button", { name: "Go to Saving" }),
    );

    expect(await screen.findByRole("tab", { name: /Saving/ })).toBeInTheDocument();
  });

  it("says when the alerts could not be read, with Read again, and counts nothing it cannot", async () => {
    readingFails = "the plane could not be listed";
    const open = await drawer();

    const trouble = await waitFor(() => row(open, "alerts-unread"));
    expect(trouble).toHaveTextContent(
      "purlis could not read the alerts: the plane could not be listed",
    );
    readingFails = undefined;
    rows = [REINIT];
    await userEvent.click(within(trouble).getByRole("button", { name: "Read again" }));

    await waitFor(() => expect(listed("alert:reinit")).toBe(true));
    expect(listed("alerts-unread")).toBe(false);
  });

  it("says where purlis stopped looking, keeps what it found, and counts no number", async () => {
    rows = [REINIT];
    stopped = "a workspace could not be read";
    const open = await drawer();

    await waitFor(() => expect(row(open, "alerts-stopped")).toHaveTextContent(/there may be more/));
    expect(listed("alert:reinit")).toBe(true);
    expect(screen.getByRole("button", { name: "Notices: not counted" })).toBeInTheDocument();
  });
});

/** This machine's rows, in an Inbox's Notices of their own, as the window's store says them. */
function machineAlone(): HTMLElement {
  function Machine() {
    const machine = useAboutThisMachine();
    return (
      <NoticeList>
        <InboxAlerts
          plane={PLANE}
          reading={{ at: "read", planes: [{ plane: PLANE, alerts: [], stopped: null }] }}
          machine={machine}
          elsewhere={[]}
          does={{
            openSettings: vi.fn(),
            openProject: vi.fn(),
            openSaving: vi.fn(),
            reread: vi.fn(),
            openInboxOf: vi.fn(),
          }}
        />
      </NoticeList>
    );
  }
  return render(<Machine />).container;
}

describe("a row about this machine", () => {
  it("uses the built-in theme once asked, moving the file aside, and the row goes", async () => {
    theme("/home/op/.config/charter/theme.json is not valid JSON");
    // What `main.tsx` does before the first frame: the theme file is read, and said.
    theirThemeOnce();
    const open = await drawer();
    const machine = await within(open).findByRole("region", { name: "Notices" });
    const themeRow = () => machine.querySelector('[data-cause="alert:theme"]') as HTMLElement;

    await userEvent.click(within(themeRow()).getByRole("button", { name: "Use built-in…" }));
    expect(calls("use_built_in_theme")).toHaveLength(0);
    await userEvent.click(within(themeRow()).getByRole("button", { name: "Keep it" }));
    expect(calls("use_built_in_theme")).toHaveLength(0);

    await userEvent.click(within(themeRow()).getByRole("button", { name: "Use built-in…" }));
    await userEvent.click(within(themeRow()).getByRole("button", { name: "Use built-in" }));

    await waitFor(() => expect(calls("use_built_in_theme")).toHaveLength(1));
    await waitFor(() => expect(machine.querySelector('[data-cause^="alert:"]')).toBeNull());
  });

  it("with nothing to do but read it, can be dismissed", async () => {
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "/home/op/.config/charter/layout.json",
        found: true,
        document: null,
        trouble: "/home/op/.config/charter/layout.json is not valid JSON",
      },
      theme: { path: "", found: false, document: null, trouble: null },
    };
    // What `main.tsx` does after the first frame: what the layout file cost is said.
    await settleLayout();
    // Alone: the window writes a layout file of its own as soon as the Inbox is opened, and a
    // file it wrote is one it can use, so the row would go before anyone read it.
    const machine = machineAlone();
    const layout = machine.querySelector('[data-cause="alert:layout"]') as HTMLElement;

    await userEvent.click(within(layout).getByRole("button", { name: "Dismiss" }));

    await waitFor(() => expect(machine.querySelector('[data-cause^="alert:"]')).toBeNull());
    expect(calls("use_default_layout")).toHaveLength(0);
  });

  it("uses the default layout once asked, saying what goes aside with the file, and the row goes", async () => {
    // #1289: a layout file purlis could not use had only Dismiss.
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "/home/op/.config/charter/layout.json",
        found: true,
        document: null,
        trouble: "/home/op/.config/charter/layout.json is not valid JSON",
      },
      theme: { path: "", found: false, document: null, trouble: null },
    };
    await settleLayout();
    // Alone: the window writes a layout file of its own as soon as the Inbox is opened, and a
    // file it wrote is one it can use, so the row would go before anyone read it.
    const machine = machineAlone();
    const layoutRow = () => machine.querySelector('[data-cause="alert:layout"]') as HTMLElement;

    await userEvent.click(
      within(layoutRow()).getByRole("button", { name: "Use the default layout…" }),
    );
    expect(calls("use_default_layout")).toHaveLength(0);
    // The question says what the file keeps besides the arrangement (D-1289-1).
    expect(layoutRow().textContent).toMatch(/layout\.aside\.json/);
    expect(layoutRow().textContent).toMatch(/Notices you dismissed, which can show again/);
    // Only what the file holds: pins are not in it (they are the machine store's).
    expect(layoutRow().textContent).toMatch(/text sizes, your editor, how chats are listed/);
    expect(layoutRow().textContent).not.toMatch(/pins/);
    // And what the window does until then, which a change the person makes writes again.
    expect(layoutRow().textContent).toMatch(/next change you make writes a new file/);
    await userEvent.click(within(layoutRow()).getByRole("button", { name: "Keep it" }));
    expect(calls("use_default_layout")).toHaveLength(0);

    await userEvent.click(
      within(layoutRow()).getByRole("button", { name: "Use the default layout…" }),
    );
    await userEvent.click(
      within(layoutRow()).getByRole("button", { name: "Use the default layout" }),
    );

    await waitFor(() => expect(calls("use_default_layout")).toHaveLength(1));
    await waitFor(() => expect(machine.querySelector('[data-cause^="alert:"]')).toBeNull());
    expect(calls("use_built_in_theme")).toHaveLength(0);
  });

  it("offers the default layout where a dismissal could not be kept", async () => {
    sayAboutThisMachine("dismissed", {
      severity: "warn",
      detail: "purlis could not keep what you dismissed: the layout is not JSON",
      remedy: "use the default layout, which moves it aside",
      defaultLayout: true,
    });
    const open = await drawer();
    const machine = await within(open).findByRole("region", { name: "Notices" });
    const kept = machine.querySelector('[data-cause="alert:dismissed"]') as HTMLElement;

    expect(
      within(kept)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(expect.arrayContaining(["Use the default layout…"]));
  });
});

describe("another project's alerts", () => {
  it("are one line a project, naming how many, whose way out opens that project's Inbox", async () => {
    const openInboxOf = vi.fn();
    render(
      <NoticeList>
        <InboxAlerts
          plane={PLANE}
          reading={{
            at: "read",
            planes: [
              { plane: PLANE, alerts: [], stopped: null },
              { plane: OUTER, alerts: [SAVE, ROOT], stopped: null },
            ],
          }}
          machine={[]}
          elsewhere={[{ plane: OUTER, name: "outer" }]}
          does={{
            openSettings: vi.fn(),
            openProject: vi.fn(),
            openSaving: vi.fn(),
            reread: vi.fn(),
            openInboxOf,
          }}
        />
      </NoticeList>,
    );

    const line = document.querySelector(`[data-cause="alerts-elsewhere:${OUTER}"]`) as HTMLElement;
    expect(line).toHaveTextContent("2 alerts in outer");
    // A failure among them is trouble.
    expect(line).toHaveClass("notice-trouble");
    await userEvent.click(within(line).getByRole("button", { name: "See them in outer" }));
    expect(openInboxOf).toHaveBeenCalledWith(OUTER);
  });
});
