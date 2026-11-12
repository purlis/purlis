// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsTab } from "./SettingsTab";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";
import { forgetGroups } from "./links";
import type { EntryWritten, SettingsFile, SettingsWhich } from "../bindings";
import { pickIn } from "../ui/select/select.testkit";

/**
 * **Settings › Sandbox's hosts** (#1341): the project's own hosts, kept in `charter.toml` and
 * followed by every teammate, and yours, in `charter.local.toml` on this machine only — each a
 * list with Add and Remove. The core checks every host (`purlis_core::settings::hosts`); held
 * here is what the person sees and what the window sends.
 */

const PLANE = "/home/dev/plane";

const NAMES: Record<SettingsWhich, string> = { shared: "hosts", local: "myHosts" };

/** When, in seconds since 1970, and by what, a host of yours was added, as the core records it. */
type Added = Record<string, { at: number; by: "you" | "block" }>;

function fileOf(
  which: SettingsWhich,
  hosts: readonly string[],
  unconfirmed: readonly string[] = [],
  added: Added = {},
): SettingsFile {
  const text =
    which === "shared"
      ? `schema = 1\n\n[sandbox]\nmode = "on"\nhosts = ${JSON.stringify(hosts)}\n`
      : hosts.length > 0
        ? `[sandbox]\nhosts = ${JSON.stringify(hosts)}\n`
        : "";
  return {
    which,
    file: which === "shared" ? "charter.toml" : "charter.local.toml",
    exists: which === "shared" || hosts.length > 0,
    text,
    refusals: [],
    parsed: true,
    fields:
      which === "shared"
        ? [
            { path: [{ key: "schema" }], value: { kind: "text", value: "1" } },
            { path: [{ key: "sandbox" }, { key: "mode" }], value: { kind: "text", value: "on" } },
          ]
        : [],
    entries: hosts.map((host, at) => ({
      collection: NAMES[which],
      id: `host:${at}:${host}`,
      label: unconfirmed.includes(host) ? `${host} (not yet confirmed)` : host,
      keys: [{ key: "sandbox" }, { key: "hosts" }],
      values: [
        { field: "host", value: host },
        ...(which === "local"
          ? [{ field: "confirmed", value: unconfirmed.includes(host) ? "no" : "yes" }]
          : []),
        ...(which === "local" && added[host]
          ? [
              { field: "added_at", value: String(added[host].at) },
              { field: "added_by", value: added[host].by },
            ]
          : []),
      ],
    })),
  };
}

const said = (value: string | null, source = "default") => ({ value, source });

const SAVING = {
  plane: {
    mode: said("push", "shared"),
    from_share: false,
    branch: said(null),
    save_branch: said(null),
    sign: said("off"),
    autosave: said("on"),
    autosave_after: said("1m"),
  },
  repos: [],
  plane_left_out: null,
  repos_left_out: null,
};

type Asked = { cmd: string; which: SettingsWhich; base: string | null; arg: string };

function core({
  refuse,
  check,
  local = [],
  unconfirmed = [],
  added = {},
}: {
  refuse?: (host: string) => string | undefined;
  check?: (host: string) => string | null | Promise<string | null>;
  local?: string[];
  unconfirmed?: string[];
  added?: Added;
} = {}) {
  const hosts: Record<SettingsWhich, string[]> = { shared: ["10.100.39.145:6443"], local };
  let waiting = [...unconfirmed];
  const local_ = () => fileOf("local", hosts.local, waiting, added);
  const asked: Asked[] = [];
  const checked: string[] = [];
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      const which = given.which as SettingsWhich;
      switch (cmd) {
        case "project_settings":
          return { shared: fileOf("shared", hosts.shared), local: local_() };
        case "project_extensions":
          return { extensions: [], local_left_out: null };
        case "project_saving_in_force":
          return SAVING;
        case "project_theme":
        case "project_theme_drawn":
          return null;
        case "extensions_on":
          return [];
        case "sandbox_state":
          return {
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
        case "check_sandbox_host": {
          const host = given.host as string;
          checked.push(host);
          return check?.(host) ?? null;
        }
        case "add_sandbox_host": {
          const host = given.host as string;
          asked.push({ cmd, which, base: given.base as string | null, arg: host });
          const why = refuse?.(host);
          if (why)
            return {
              kind: "refused",
              fields: [{ field: "host", why }],
              referrers: [],
              reasons: [],
            } satisfies EntryWritten;
          hosts[which] = [...hosts[which], host];
          return {
            kind: "saved",
            file: fileOf(which, hosts[which]),
            added: `host:${hosts[which].length - 1}:${host}`,
            removed: null,
          } satisfies EntryWritten;
        }
        case "confirm_sandbox_host": {
          const id = given.id as string;
          asked.push({ cmd, which: "local", base: given.base as string | null, arg: id });
          const host = hosts.local.find((one, at) => `host:${at}:${one}` === id) ?? "";
          waiting = waiting.filter((one) => one !== host);
          return { kind: "saved", file: local_(), added: id, removed: null } satisfies EntryWritten;
        }
        case "remove_sandbox_host": {
          const id = given.id as string;
          asked.push({ cmd, which, base: given.base as string | null, arg: id });
          const took = hosts[which].find((host, at) => `host:${at}:${host}` === id) ?? "";
          hosts[which] = hosts[which].filter((host) => host !== took);
          return {
            kind: "saved",
            file: fileOf(which, hosts[which]),
            added: null,
            removed: [{ field: "host", value: took }],
          } satisfies EntryWritten;
        }
      }
      return undefined;
    },
    { shouldMockEvents: true },
  );
  return { asked, checked, hosts };
}

beforeEach(() => {
  forgetThisLaunch();
  forgetGroups();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/dev/layout.json", found: false, document: null, trouble: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

async function at(group: string) {
  render(<SettingsTab plane={PLANE} level="project" />);
  await userEvent.click(
    await within(await screen.findByRole("navigation", { name: "Groups" })).findByRole("button", {
      // A page with things waiting says how many after its name (R46).
      name: (name) =>
        name === group ||
        (name.startsWith(`${group} `) && /^\d+$/.test(name.slice(group.length + 1))),
    }),
  );
}

describe("Settings › Sandbox's hosts", () => {
  it("lists the project's hosts, each with Remove", async () => {
    core();
    await at("Sandbox");
    expect(
      await screen.findByRole("button", { name: "Remove 10.100.39.145:6443" }),
    ).toBeInTheDocument();
  });

  it("adds a project host to the committed file, sent against the text it was drawn from", async () => {
    const { asked, hosts } = core();
    await at("Sandbox");

    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await userEvent.type(within(form).getByLabelText("Host"), "*.internal.example");
    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));

    await waitFor(() => expect(hosts.shared).toContain("*.internal.example"));
    expect(asked[0]).toEqual({
      cmd: "add_sandbox_host",
      which: "shared",
      base: fileOf("shared", ["10.100.39.145:6443"]).text,
      arg: "*.internal.example",
    });
    expect(await screen.findByText("Added *.internal.example.")).toBeInTheDocument();
  });

  it("says why a host is refused under its field, and keeps what was typed", async () => {
    core({
      refuse: () => "That is a URL. Type its host alone, such as x.example, without the scheme.",
    });
    await at("Sandbox");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await userEvent.type(within(form).getByLabelText("Host"), "https://x.example/");
    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));

    // Heard once, as the form's summary (#3291), and said under the field it is about.
    expect(await within(form).findByRole("alert")).toHaveTextContent("Host: That is a URL.");
    expect(within(form).getByLabelText("Host")).toHaveAccessibleDescription(/That is a URL/);
    expect(within(form).getByLabelText("Host")).toHaveValue("https://x.example/");
  });

  it("checks the host as it is typed, with the core's sentence, before Add is pressed", async () => {
    const url =
      "That is a URL. Type its host alone, such as x.example, without the scheme or a path.";
    const { asked, checked } = core({ check: (host) => (host.includes("/") ? url : null) });
    await at("Sandbox");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    const field = within(form).getByLabelText("Host");

    await userEvent.type(field, "https://x.example/");
    expect(await within(form).findByText(url)).toBeInTheDocument();
    expect(asked.map((one) => one.cmd)).not.toContain("add_sandbox_host");
    expect(checked.at(-1)).toBe("https://x.example/");

    await userEvent.clear(field);
    await userEvent.type(field, "x.example");
    await waitFor(() => expect(within(form).queryByText(url)).not.toBeInTheDocument());
  });

  it("drops a check's answer that comes back after the field was typed over", async () => {
    const stale = "Leave out the path: a host ends before the first /, so type x.example.";
    let answer: (why: string) => void = () => undefined;
    const late = new Promise<string | null>((resolve) => {
      answer = resolve;
    });
    const { checked } = core({ check: (host) => (host === "x.example/" ? late : null) });
    await at("Sandbox");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    const field = within(form).getByLabelText("Host");

    await userEvent.type(field, "x.example/");
    await userEvent.type(field, "{Backspace}");
    await waitFor(() => expect(checked.at(-1)).toBe("x.example"));
    // The answer for the text the field held before arrives last, and is not said.
    answer(stale);
    await late;
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(within(form).queryByText(stale)).not.toBeInTheDocument();
    expect(field).toHaveValue("x.example");
  });

  it("removes a project host by its identity", async () => {
    const { asked, hosts } = core();
    await at("Sandbox");
    await userEvent.click(await screen.findByRole("button", { name: "Remove 10.100.39.145:6443" }));
    await waitFor(() => expect(hosts.shared).toEqual([]));
    expect(asked[0]).toMatchObject({
      cmd: "remove_sandbox_host",
      which: "shared",
      arg: "host:0:10.100.39.145:6443",
    });
  });

  it("adds your own host to this machine's file alone, on its own page", async () => {
    const { asked, hosts } = core();
    await at("Your hosts");
    expect(screen.getAllByText(/on this machine only/)[0]).toBeInTheDocument();

    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await userEvent.type(within(form).getByLabelText("Host"), "[[fd00::7]:8443");
    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));

    await waitFor(() => expect(hosts.local).toEqual(["[fd00::7]:8443"]));
    expect(hosts.shared).toEqual(["10.100.39.145:6443"]);
    expect(asked[0]).toMatchObject({ cmd: "add_sandbox_host", which: "local", base: null });
  });

  it("waits for your Confirm on a host Settings did not add here, and confirms it", async () => {
    const { asked } = core({ local: ["10.0.0.6"], unconfirmed: ["10.0.0.6"] });
    await at("Your hosts");

    expect(await screen.findByText("Waiting for you")).toBeVisible();
    await userEvent.click(await screen.findByRole("button", { name: "Confirm 10.0.0.6" }));

    await waitFor(() =>
      expect(screen.queryByRole("button", { name: /^Confirm/ })).not.toBeInTheDocument(),
    );
    expect(asked[0]).toMatchObject({ cmd: "confirm_sandbox_host", arg: "host:0:10.0.0.6" });
    expect(screen.getByRole("rowheader", { name: "10.0.0.6:443" })).toBeInTheDocument();
    expect(screen.getByText("Confirmed", { selector: "[data-scope=badge]" })).toBeVisible();
  });
});

/**
 * **Your hosts is a table** (#2919, prototype A, R65, R67–R73): a row per host, waiting first,
 * the host in mono with its port quiet, its status, and its Confirm and Remove; over it a filter,
 * Confirm all and Add host, which swap to how many are chosen, Confirm and Remove; each act says
 * what it did.
 */
describe("Settings › Your hosts, the table", () => {
  // Nine: a filter is drawn from eight (R164).
  const MINE = [
    "docs.github.com",
    "api.example.com:8443",
    "zed.dev",
    "fork.dev",
    "crates.io",
    "pypi.org",
    "code.visualstudio.com",
    "www.jetbrains.com",
    "[fd00::7]:8443",
  ];
  const WAITING = ["zed.dev", "fork.dev"];

  async function hostsTable() {
    const made = core({ local: [...MINE], unconfirmed: [...WAITING] });
    await at("Your hosts");
    const table = await screen.findByRole("grid", { name: "Your hosts" });
    return { ...made, table, user: userEvent.setup() };
  }

  /** The table's rows, each as its cells' words. */
  const rowsOf = (table: HTMLElement) =>
    within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) =>
        within(row)
          .queryAllByRole("rowheader")
          .concat(within(row).queryAllByRole("gridcell"))
          .map((cell) => cell.textContent?.trim() ?? "")
          .filter((words) => words !== ""),
      );

  it("lists each host with its status, waiting first, and Confirm only on a waiting one (R65, R67, R68)", async () => {
    const { table } = await hostsTable();

    expect(rowsOf(table).map((cells) => cells.slice(0, 2))).toEqual([
      ["zed.dev:443", "Waiting for you"],
      ["fork.dev:443", "Waiting for you"],
      ["docs.github.com:443", "Confirmed"],
      ["api.example.com:8443", "Confirmed"],
      ["crates.io:443", "Confirmed"],
      ["pypi.org:443", "Confirmed"],
      ["code.visualstudio.com:443", "Confirmed"],
      ["www.jetbrains.com:443", "Confirmed"],
      ["[fd00::7]:8443", "Confirmed"],
    ]);
    // The host in mono, its port in the quiet words.
    const host = within(table).getByText("api.example.com");
    expect(host.closest("code")).not.toBeNull();
    expect(within(table).getAllByText(":8443")[0]).toHaveClass("ui-hosts-port");
    expect(within(table).getAllByRole("button", { name: /^Confirm / })).toHaveLength(2);
    expect(within(table).getAllByRole("button", { name: /^Remove / })).toHaveLength(9);
  });

  it("says when and by what each host was added, and that one from before the record was added earlier (R65, #6172)", async () => {
    const now = Date.UTC(2026, 10, 12, 12, 0, 0) / 1000;
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(now * 1000);
    try {
      core({
        local: ["docs.github.com", "zed.dev", "crates.io", "pypi.org"],
        added: {
          "docs.github.com": { at: now - 3 * 86_400, by: "you" },
          "zed.dev": { at: now - 20 * 60, by: "block" },
          "pypi.org": { at: now - 90 * 86_400, by: "you" },
        },
      });
      await at("Your hosts");
      const table = await screen.findByRole("grid", { name: "Your hosts" });
      expect(within(table).getByRole("columnheader", { name: "Added" })).toBeInTheDocument();
      expect(rowsOf(table).map((cells) => [cells[0], cells[2]])).toEqual([
        ["docs.github.com:443", "3 days ago · you"],
        ["zed.dev:443", "20 minutes ago · a chat's block"],
        ["crates.io:443", "Earlier"],
        ["pypi.org:443", "14 Aug 2026 · you"],
      ]);
    } finally {
      vi.useRealTimers();
    }
  });

  it("filters by any part of a host, and says so when none is left (R69, R73)", async () => {
    const { table, user } = await hostsTable();

    const filter = screen.getByRole("searchbox", { name: "Filter hosts" });
    expect(filter).toHaveAttribute("placeholder", "Filter 9 hosts");
    await user.type(filter, "dev");
    expect(rowsOf(table).map((cells) => cells[0])).toEqual(["zed.dev:443", "fork.dev:443"]);
    expect(filter).toHaveFocus();
    await user.type(filter, "x");

    expect(within(table).getByText("No host is called “devx”.")).toBeVisible();
  });

  it("offers Confirm all and Add host, and swaps them for what to do to the chosen (R70, R71)", async () => {
    const { table, user } = await hostsTable();
    const bar = screen.getByRole("group", { name: "Your hosts" });

    expect(within(bar).getByRole("button", { name: "Confirm all 2 waiting" })).toBeVisible();
    expect(within(bar).getByRole("button", { name: "Add host" })).toBeVisible();
    await user.click(within(table).getByRole("checkbox", { name: "Select All" }));

    expect(within(bar).getByText("9 selected")).toBeVisible();
    expect(within(bar).queryByRole("button", { name: /^Confirm all/ })).toBeNull();
    expect(within(bar).getByRole("button", { name: "Confirm" })).toBeVisible();
    expect(within(bar).getByRole("button", { name: "Remove" })).toBeVisible();
  });

  it("confirms one host, and says so (R72)", async () => {
    const { table, asked, user } = await hostsTable();

    await user.click(within(table).getByRole("button", { name: "Confirm zed.dev" }));

    expect(await screen.findByText("Confirmed zed.dev.")).toBeVisible();
    expect(asked).toEqual([
      expect.objectContaining({ cmd: "confirm_sandbox_host", arg: "host:2:zed.dev" }),
    ]);
  });

  it("confirms every waiting host at once, and says so (R72)", async () => {
    const { asked, user } = await hostsTable();

    await user.click(screen.getByRole("button", { name: "Confirm all 2 waiting" }));

    expect(await screen.findByText("Confirmed every waiting host.")).toBeVisible();
    expect(asked.map((one) => one.arg)).toEqual(["host:2:zed.dev", "host:3:fork.dev"]);
  });

  it("confirms the chosen hosts, and says how many rather than every waiting host", async () => {
    const { table, asked, user } = await hostsTable();

    await user.click(within(table).getByRole("checkbox", { name: "Select fork.dev:443" }));
    await user.click(
      within(screen.getByRole("group", { name: "Your hosts" })).getByRole("button", {
        name: "Confirm",
      }),
    );

    expect(await screen.findByText("Confirmed fork.dev.")).toBeVisible();
    expect(asked.map((one) => one.arg)).toEqual(["host:3:fork.dev"]);
  });

  it("removes one host, says so, and offers its Undo (R72)", async () => {
    const { table, hosts, user } = await hostsTable();

    await user.click(within(table).getByRole("button", { name: "Remove docs.github.com" }));

    expect(await screen.findByText("Removed docs.github.com.")).toBeVisible();
    expect(hosts.local).not.toContain("docs.github.com");
    expect(screen.getByRole("button", { name: "Undo" })).toBeVisible();
  });

  it("removes every chosen host, each by its identity as the file stands then", async () => {
    const { table, asked, hosts, user } = await hostsTable();

    await user.click(within(table).getByRole("checkbox", { name: "Select docs.github.com:443" }));
    await user.click(within(table).getByRole("checkbox", { name: "Select zed.dev:443" }));
    await user.click(
      within(screen.getByRole("group", { name: "Your hosts" })).getByRole("button", {
        name: "Remove",
      }),
    );

    await waitFor(() =>
      expect(hosts.local).toEqual(
        MINE.filter((one) => one !== "zed.dev" && one !== "docs.github.com"),
      ),
    );
    expect(asked.map((one) => one.arg)).toEqual(["host:2:zed.dev", "host:0:docs.github.com"]);
    expect(await screen.findByText("Removed 2 hosts.")).toBeVisible();
  });

  it("opens the form to add a host from the bar, and comes back to Add host from it", async () => {
    const { user } = await hostsTable();

    await user.click(screen.getByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    expect(within(form).getByLabelText("Host")).toHaveFocus();
    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.getByRole("button", { name: "Add host" })).toHaveFocus());
  });
});

/**
 * **A persona's hosts** (#1407, `[sandbox.personas.<p>] hosts`): added and removed on their own
 * page as the project's are, each host checked as it is typed by the core's parser, and written
 * through the one settings save, which refuses a persona the project does not define. Adding
 * one grants nothing: a chat as that persona reaches the list only once the person on each
 * machine allows it as it now stands (D-1362-7), so the page says so and never allows.
 */
describe("Settings › Sandbox › Persona hosts", () => {
  type Lists = Record<string, string[]>;

  function personaFile(lists: Lists): SettingsFile {
    const tables = Object.entries(lists)
      .map(
        ([persona, hosts]) => `\n[sandbox.personas.${persona}]\nhosts = ${JSON.stringify(hosts)}\n`,
      )
      .join("");
    return {
      which: "shared",
      file: "charter.toml",
      exists: true,
      text: `schema = 1\n\n[sandbox]\nmode = "on"\n${tables}`,
      refusals: [],
      parsed: true,
      fields: [
        { path: [{ key: "schema" }], value: { kind: "text", value: "1" } },
        { path: [{ key: "sandbox" }, { key: "mode" }], value: { kind: "text", value: "on" } },
        ...Object.entries(lists).map(([persona, hosts]) => ({
          path: [{ key: "sandbox" }, { key: "personas" }, { key: persona }, { key: "hosts" }],
          value: { kind: "list" as const, value: hosts },
        })),
      ],
      entries: [],
    };
  }

  type Saved = { which: SettingsWhich; base: string | null; edits: unknown };

  function personaCore({
    lists = { devops: ["10.100.39.145:6443"] },
    personas = ["devops", "writer"],
    refuse,
    check,
    policy = null,
    reached,
  }: {
    lists?: Lists;
    personas?: string[];
    refuse?: string[];
    check?: (host: string) => string | null;
    policy?: Record<string, unknown> | null;
    reached?: Lists;
  } = {}) {
    let now: Lists = structuredClone(lists);
    const saved: Saved[] = [];
    const checked: string[] = [];
    const commands: string[] = [];
    mockIPC(
      (cmd, args) => {
        commands.push(cmd);
        const given = (args ?? {}) as Record<string, unknown>;
        switch (cmd) {
          case "project_settings":
            return { shared: personaFile(now), local: fileOf("local", []) };
          case "project_extensions":
            return { extensions: [], local_left_out: null };
          case "project_saving_in_force":
            return SAVING;
          case "start_options":
            return {
              profiles: [],
              refused: [],
              personas,
              persona: null,
              persona_profiles: {},
              ignore_fix: null,
            };
          case "sandbox_state":
            return {
              on: true,
              offer: false,
              said: null,
              never: [],
              hosts_changed: null,
              presets_changed: null,
              presets: [],
              persona_hosts: Object.entries(now).map(([persona, hosts]) => ({
                persona,
                hosts,
                reached: reached?.[persona] ?? hosts,
                tools: [],
                digest: `d:${hosts.join(",")}`,
                allowed: false,
                default: false,
                waiting: false,
              })),
              besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
              policy,
            };
          case "check_sandbox_host": {
            const host = given.host as string;
            checked.push(host);
            return check?.(host) ?? null;
          }
          case "save_project_settings": {
            const change = given.change as {
              kind: string;
              edits: { path: { key: string }[]; value: { value: string[] } | null }[];
            };
            saved.push({
              which: given.which as SettingsWhich,
              base: given.base as string | null,
              edits: change.edits,
            });
            if (refuse !== undefined) return { kind: "refused", reasons: refuse };
            let next: Lists = structuredClone(now);
            for (const edit of change.edits) {
              const persona = edit.path[2].key;
              const value = edit.value;
              next =
                value === null
                  ? Object.fromEntries(Object.entries(next).filter(([one]) => one !== persona))
                  : { ...next, [persona]: value.value };
            }
            now = next;
            return { kind: "saved", file: personaFile(now) };
          }
        }
        return undefined;
      },
      { shouldMockEvents: true },
    );
    return { saved, checked, commands, now: () => now };
  }

  const PATH = (persona: string) => [
    { key: "sandbox" },
    { key: "personas" },
    { key: persona },
    { key: "hosts" },
  ];

  it("lists each persona's hosts on a page of their own, each with Remove", async () => {
    personaCore({ lists: { devops: ["10.100.39.145:6443", "*.internal.example"] } });
    await at("Persona hosts");
    expect(
      await screen.findByRole("button", { name: "Remove devops: 10.100.39.145:6443" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Remove devops: *.internal.example" }),
    ).toBeInTheDocument();
  });

  it("says adding a host grants nothing until it is allowed on each machine", async () => {
    personaCore();
    await at("Persona hosts");
    expect(
      await screen.findByText(/reaches them only once the person on each machine allows the list/),
    ).toBeInTheDocument();
  });

  it("adds a host to the persona's list through the settings save, against the text drawn", async () => {
    const { saved, commands, now } = personaCore();
    await at("Persona hosts");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await pickIn(within(form).getByLabelText("Persona"), "devops");
    await userEvent.type(within(form).getByLabelText("Host"), "*.internal.example");
    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));

    await waitFor(() => expect(now().devops).toEqual(["10.100.39.145:6443", "*.internal.example"]));
    expect(saved[0]).toEqual({
      which: "shared",
      base: personaFile({ devops: ["10.100.39.145:6443"] }).text,
      edits: [
        {
          path: PATH("devops"),
          value: { kind: "list", value: ["10.100.39.145:6443", "*.internal.example"] },
        },
      ],
    });
    // Adding is not allowing: the window's Allow is never asked for here.
    expect(commands).not.toContain("allow_persona_hosts");
    expect(await screen.findByText("Added devops: *.internal.example.")).toBeInTheDocument();
  });

  it("starts a list for a persona that has none yet", async () => {
    const { saved, now } = personaCore();
    await at("Persona hosts");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await pickIn(within(form).getByLabelText("Persona"), "writer");
    await userEvent.type(within(form).getByLabelText("Host"), "api.example.com");
    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));

    await waitFor(() => expect(now().writer).toEqual(["api.example.com"]));
    expect(saved[0].edits).toEqual([
      { path: PATH("writer"), value: { kind: "list", value: ["api.example.com"] } },
    ]);
  });

  it("checks the host as it is typed, and sends nothing the parser refuses", async () => {
    const url = "That is a URL. Type its host alone, such as x.example, without the scheme.";
    const { saved, checked } = personaCore({
      check: (host) => (host.includes("/") ? url : null),
    });
    await at("Persona hosts");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await userEvent.type(within(form).getByLabelText("Host"), "https://x.example/");
    expect(await within(form).findByText(url)).toBeInTheDocument();
    expect(checked.at(-1)).toBe("https://x.example/");

    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));
    expect(await within(form).findAllByText(url)).not.toHaveLength(0);
    expect(saved).toEqual([]);
  });

  it("refuses a host the persona already lists, under the field", async () => {
    const { saved } = personaCore();
    await at("Persona hosts");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await pickIn(within(form).getByLabelText("Persona"), "devops");
    await userEvent.type(within(form).getByLabelText("Host"), "10.100.39.145:6443");
    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));
    expect(
      await within(form).findByText("10.100.39.145:6443 is already one of devops's hosts."),
    ).toBeInTheDocument();
    expect(saved).toEqual([]);
  });

  it("says the core's refusal, such as a persona the project does not define", async () => {
    const why =
      "sandbox.personas.writer in charter.toml names no persona of this project, so it grants nothing — make the persona first, or take its table out";
    personaCore({ refuse: [why] });
    await at("Persona hosts");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await pickIn(within(form).getByLabelText("Persona"), "writer");
    await userEvent.type(within(form).getByLabelText("Host"), "api.example.com");
    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));
    // Heard once, as the form's summary (#3291), and drawn where it belongs.
    expect(await within(form).findByRole("alert")).toHaveTextContent(why);
    expect(form.querySelector(".ui-setting-error:not([aria-live])")).toHaveTextContent(why);
  });

  it("removes one host, and the persona's table with its last", async () => {
    const { saved, now } = personaCore({
      lists: { devops: ["10.100.39.145:6443", "*.internal.example"] },
    });
    await at("Persona hosts");
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove devops: 10.100.39.145:6443" }),
    );
    await waitFor(() => expect(now().devops).toEqual(["*.internal.example"]));
    expect(saved[0].edits).toEqual([
      { path: PATH("devops"), value: { kind: "list", value: ["*.internal.example"] } },
    ]);

    await userEvent.click(
      await screen.findByRole("button", { name: "Remove devops: *.internal.example" }),
    );
    await waitFor(() => expect(now().devops).toBeUndefined());
    expect(saved[1].edits).toEqual([
      { path: [{ key: "sandbox" }, { key: "personas" }, { key: "devops" }], value: null },
    ]);
  });

  it("undoes an add with a remove of that host, and a remove with the text it was made in", async () => {
    const { saved, now } = personaCore();
    await at("Persona hosts");
    await userEvent.click(await screen.findByRole("button", { name: "Add host" }));
    const form = screen.getByRole("form", { name: "New host" });
    await pickIn(within(form).getByLabelText("Persona"), "devops");
    await userEvent.type(within(form).getByLabelText("Host"), "api.example.com");
    await userEvent.click(within(form).getByRole("button", { name: "Add host" }));
    await waitFor(() => expect(now().devops).toContain("api.example.com"));

    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(() => expect(now().devops).toEqual(["10.100.39.145:6443"]));
    expect(saved[1].edits).toEqual([
      { path: PATH("devops"), value: { kind: "list", value: ["10.100.39.145:6443"] } },
    ]);
  });

  it("says which listed hosts policy keeps every chat from", async () => {
    personaCore({
      lists: { devops: ["10.100.39.145:6443", "paste.example"] },
      reached: { devops: ["10.100.39.145:6443"] },
    });
    await at("Persona hosts");
    expect(
      await screen.findByText(
        /devops lists paste\.example, which policy on this machine keeps every chat from/,
      ),
    ).toBeInTheDocument();
  });

  it("offers no Add or Remove where policy forbids a persona's own hosts", async () => {
    personaCore({
      policy: {
        locked_by: "Locked by policy, set by IT in policy.json.",
        presets: null,
        hosts: null,
        personal_hosts: false,
        persona_hosts: true,
        opt_out: false,
        required: null,
        write_grants: false,
        undeclared_secret_asks: false,
        run_grants: false,
      },
    });
    await at("Persona hosts");
    expect(
      await screen.findByText(/Policy forbids a persona's own hosts, so none reaches a chat here/),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add host" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Remove devops/ })).not.toBeInTheDocument();
  });

  it("says where policy turns off asks for an undeclared secret, and who set it", async () => {
    // #5078: the lock is shown as policy's, with nothing to change.
    personaCore({
      policy: {
        locked_by: "Locked by policy, set by IT in policy.json.",
        presets: null,
        hosts: null,
        personal_hosts: false,
        persona_hosts: false,
        opt_out: false,
        required: null,
        write_grants: false,
        undeclared_secret_asks: true,
        run_grants: false,
      },
    });
    await at("Secret asks");
    expect(
      await screen.findByText(
        /Policy turns off asks for a secret a persona doesn't declare: a chat that asks for one is told it was declined by policy, and you aren't asked\. Locked by policy, set by IT in policy\.json\./,
      ),
    ).toBeInTheDocument();
  });
});
