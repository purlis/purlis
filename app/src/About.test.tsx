// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AboutCharter } from "./About";
import { OPEN_LICENCES } from "./LicencesView";
import type { SlowMoment } from "./bindings";
import { SLOW_ENTRIES_COPIED } from "./diagnostics";

// The third-party notices `vite build` generates (#1784); the dev server and vitest serve none,
// so each test says what the build it stands for would have written.
const thirdParty = vi.hoisted(() => ({
  generated: true,
  packages: [] as {
    kind: "crate" | "npm";
    name: string;
    version: string;
    licence?: string;
    texts: number[];
    standard?: string[];
    authors?: string[];
  }[],
  texts: [] as string[],
}));
vi.mock("virtual:purlis-third-party", () => ({ default: thirdParty }));

afterEach(() => {
  cleanup();
  clearMocks();
  Object.assign(thirdParty, { generated: true, packages: [], texts: [] });
});

describe("About Charter", () => {
  it("opens with the keyboard on Close, as every question does (#1719)", async () => {
    mockIPC(() => new Promise(() => {}));
    render(<AboutCharter />);

    await userEvent.click(screen.getByTestId("title-about"));

    const dialog = await screen.findByRole("dialog");
    await waitFor(() =>
      expect(within(dialog).getByRole("button", { name: "Close" })).toHaveFocus(),
    );
  });

  it("says what it is reading while it reads, in sentence case (#630)", async () => {
    mockIPC(() => new Promise(() => undefined));
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    expect(await screen.findByText("Reading what this version brought…")).toBeTruthy();
  });

  it("says why what this version brought could not be read, and Read again asks again", async () => {
    let asks = 0;
    mockIPC((cmd) => {
      if (cmd !== "about_charter") throw new Error("not asked here");
      asks += 1;
      throw new Error("the changelog could not be read");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    expect(await screen.findByText(/could not read what this version brought/)).toBeTruthy();

    // NO-8's follow-up (#1296): the dialog reads once, so its refusal offers the retry.
    await userEvent.click(screen.getByRole("button", { name: "Read again" }));
    await waitFor(() => expect(asks).toBe(2));
    expect(await screen.findByText(/could not read what this version brought/)).toBeTruthy();
  });

  it("ships each vendored asset's attribution and licence text (FM-3)", async () => {
    mockIPC(() => {
      throw new Error("not asked here");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    // No project in front takes the ask for their tab, so they are drawn here (#1719).
    await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
    const notices = await screen.findByTestId("about-notices");
    expect(
      within(notices).getByText(/material-icon-theme 5\.39\.0, under the MIT licence/),
    ).toBeTruthy();
    const texts = [...notices.querySelectorAll("pre")].map((pre) => pre.textContent ?? "");
    expect(texts).toContainEqual(
      expect.stringMatching(/^The MIT License \(MIT\)\nCopyright \(c\) 2025 Material Extensions/),
    );
  });

  it("ships purlis's own NOTICE and Apache License 2.0 first, ahead of what others wrote", async () => {
    mockIPC(() => {
      throw new Error("not asked here");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    expect(await screen.findByText(/purlis is under the Apache License 2\.0/)).toBeTruthy();
    await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
    const notices = await screen.findByTestId("about-notices");
    const [notice, licence] = [...notices.querySelectorAll("pre")].map(
      (pre) => pre.textContent ?? "",
    );
    expect(notice).toMatch(/^Purlis\nCopyright 2026 Purlis\n/);
    expect(licence).toMatch(/Apache License\s+Version 2\.0, January 2004/);
  });

  it("lists every third-party crate and npm package the build ships, with its licence text, after purlis's own (#1784)", async () => {
    Object.assign(thirdParty, {
      packages: [
        {
          kind: "crate",
          name: "serde",
          version: "1.0.228",
          licence: "MIT OR Apache-2.0",
          texts: [0, 1],
        },
        {
          kind: "crate",
          name: "selectors",
          version: "0.38.0",
          licence: "MPL-2.0",
          texts: [2],
          standard: ["MPL-2.0"],
          authors: ["The Servo Project Developers"],
        },
        { kind: "npm", name: "react", version: "19.2.0", licence: "MIT", texts: [1] },
      ],
      texts: [
        "Apache License text",
        "MIT License\nCopyright (c) Meta",
        "Mozilla Public License Version 2.0",
      ],
    });
    mockIPC(() => {
      throw new Error("not asked here");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
    const notices = await screen.findByTestId("about-notices");
    const third = await within(notices).findByTestId("third-party-notices");
    expect(
      notices.querySelector('[data-testid="own-licence"]')?.compareDocumentPosition(third),
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(within(third).getByText(/2 Rust crates and 1 npm package/)).toBeTruthy();
    const parts = [...third.querySelectorAll("details")].map((part) => [
      part.querySelector("summary")?.textContent,
      part.querySelector("pre")?.textContent,
    ]);
    expect(parts).toEqual([
      ["serde 1.0.228 (MIT OR Apache-2.0)", "Apache License text"],
      ["serde 1.0.228 (MIT OR Apache-2.0), react 19.2.0 (MIT)", "MIT License\nCopyright (c) Meta"],
      [
        "selectors 0.38.0 (MPL-2.0; it ships no licence file, so this is the standard MPL-2.0 text; its authors: The Servo Project Developers)",
        "Mozilla Public License Version 2.0",
      ],
    ]);
  });

  it("says a window that was not built has no third-party notices to list (#1784)", async () => {
    Object.assign(thirdParty, { generated: false });
    mockIPC(() => {
      throw new Error("not asked here");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
    const notices = await screen.findByTestId("about-notices");
    expect(
      await within(notices).findByText(
        /This window was not built, so it has no third-party notices/,
      ),
    ).toBeTruthy();
  });

  it("opens the licences in a tab of the project in front, and closes (#1719)", async () => {
    mockIPC(() => new Promise(() => {}));
    let asked = 0;
    const take = (event: Event) => {
      asked += 1;
      event.preventDefault();
    };
    window.addEventListener(OPEN_LICENCES, take);
    try {
      render(<AboutCharter />);
      await userEvent.click(screen.getByTestId("title-about"));
      await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
      expect(asked).toBe(1);
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      expect(screen.queryByTestId("about-notices")).toBeNull();
    } finally {
      window.removeEventListener(OPEN_LICENCES, take);
    }
  });
});

/** A machine profile as `machine_profiles` answers one, its folders and sign-in words holding
 *  what must never reach a report: a path in the person's home and a token. */
function profile(kind: string, harness: string, installed: string | null, name = kind) {
  return {
    name,
    kind,
    harness,
    origin: "made",
    folder: `/Users/someone/.config/purlis/profiles/${name}`,
    imported_at: 1,
    imported_from: "/Users/someone/.claude",
    signs_in: "through its own sign-in, token TOKEN-SECRETVALUE",
    standing: "signed-in",
    plans: [],
    pinned: null,
    installed,
    offered: null,
    sandbox_only: false,
    commands_can_read_sign_in: "ANTHROPIC_API_KEY=TOKEN-SECRETVALUE",
    icon: { kind: "monogram", letters: "C" },
  };
}

/** The core as the About dialog asks it, for a dev build with two harnesses and some chats. */
/** The core, answering what Copy diagnostics asks: with no slow-answer log unless `slow` is one. */
function aCore({ slow }: { slow?: SlowMoment[] } = {}) {
  mockIPC((cmd) => {
    if (cmd === "slow_log_recent" && slow) return slow;
    if (cmd === "about_charter")
      return {
        version: "1.60.0-dev.3",
        build: { kind: "dev", of: "1.60.0" },
        profile: "dev-release",
        notes: null,
      };
    if (cmd === "machine_profiles")
      return [
        profile("claude", "Claude Code", "2.1.4"),
        profile("claude", "Claude Code", "2.1.4", "work"),
        profile("codex", "Codex", null),
      ];
    if (cmd === "diagnostics_here") return { os: "Darwin 27.2.0, aarch64", open: 12, hot: 4 };
    throw new Error(`not asked here: ${cmd}`);
  });
}

/** Copy diagnostics pressed in the About dialog of a core answering as {@link aCore} does: its lines. */
async function copiedReport(core: Parameters<typeof aCore>[0]): Promise<string[]> {
  const user = userEvent.setup();
  aCore(core);
  render(<AboutCharter />);
  await user.click(screen.getByTestId("title-about"));
  const dialog = await screen.findByRole("dialog");
  await user.click(within(dialog).getByRole("button", { name: "Copy diagnostics" }));
  await waitFor(() =>
    expect(within(dialog).getByRole("status")).toHaveTextContent("Diagnostics copied."),
  );
  return (await navigator.clipboard.readText()).split("\n");
}

describe("Copy diagnostics (#4774)", () => {
  it("copies a report with the version, the system, the harnesses and the chats, and says so", async () => {
    const user = userEvent.setup();
    aCore();
    render(<AboutCharter />);
    await user.click(screen.getByTestId("title-about"));
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByTestId("about-version");

    await user.click(within(dialog).getByRole("button", { name: "Copy diagnostics" }));

    await waitFor(() =>
      expect(within(dialog).getByRole("status")).toHaveTextContent("Diagnostics copied."),
    );
    const report = await navigator.clipboard.readText();
    expect(report.split("\n")).toEqual([
      "purlis diagnostics",
      "Version: 1.60.0-dev.3, a dev build of 1.60.0, dev-release profile",
      "System: Darwin 27.2.0, aarch64",
      "Harnesses: Claude Code 2.1.4, Codex (no version found)",
      "Chats: 12 open, 4 hot",
    ]);
  });

  it("ends with the slow-answer and stall log's recent entries, one JSON line each (#4725)", async () => {
    const slow: SlowMoment[] = [
      { kind: "answer", command: "opened_chats", ms: 1_840, at: 1_762_000_000_000, hot: 3 },
      {
        kind: "stall",
        ms: 412,
        at: 1_762_000_001_000,
        hot: 9,
        front: "steward 3",
        opening: true,
        dark: false,
        react: [{ name: "ChatView", ms: 180 }],
        events: [],
        answers: [{ name: "opened_chats", n: 2 }],
        scripts: [],
      },
    ];
    const report = await copiedReport({ slow });

    expect(report.slice(5)).toEqual([
      "Slow answers and stalls, most recent last:",
      '  {"kind":"answer","command":"opened_chats","ms":1840,"at":1762000000000,"hot":3}',
      '  {"kind":"stall","ms":412,"at":1762000001000,"hot":9,"front":"steward 3","opening":true,"dark":false,"react":[{"name":"ChatView","ms":180}],"events":[],"answers":[{"name":"opened_chats","n":2}],"scripts":[]}',
    ]);
  });

  it(`carries only the log's newest ${SLOW_ENTRIES_COPIED} entries, and says how many there are`, async () => {
    const slow: SlowMoment[] = Array.from({ length: 250 }, (_, i) => ({
      kind: "answer",
      command: `command_${i}`,
      ms: 1_000 + i,
      at: i,
      hot: 0,
    }));
    const report = await copiedReport({ slow });

    expect(report[5]).toBe(
      `Slow answers and stalls, the newest ${SLOW_ENTRIES_COPIED} of 250, most recent last:`,
    );
    const entries = report.slice(6);
    expect(entries).toHaveLength(SLOW_ENTRIES_COPIED);
    expect(entries[0]).toContain('"command":"command_50"');
    expect(entries.at(-1)).toContain('"command":"command_249"');
  });

  it("says the log has nothing in it when it is empty", async () => {
    const report = await copiedReport({ slow: [] });
    expect(report.slice(5)).toEqual(["Slow answers and stalls: none recorded"]);
  });

  it("holds no folder, sign-in or token a harness profile carries", async () => {
    const user = userEvent.setup();
    aCore();
    render(<AboutCharter />);
    await user.click(screen.getByTestId("title-about"));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Copy diagnostics" }));
    await waitFor(() =>
      expect(within(dialog).getByRole("status")).toHaveTextContent("Diagnostics copied."),
    );

    const report = await navigator.clipboard.readText();
    expect(report).not.toMatch(/SECRETVALUE|API_KEY|\/Users\/|someone|sign-in/);
  });

  it("says what went wrong when the diagnostics could not be gathered, and copies nothing", async () => {
    const user = userEvent.setup();
    await navigator.clipboard.writeText("before");
    mockIPC((cmd) => {
      if (cmd === "about_charter") return new Promise(() => {});
      if (cmd === "diagnostics_here") throw new Error("the registry is gone");
      return [];
    });
    render(<AboutCharter />);
    await user.click(screen.getByTestId("title-about"));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Copy diagnostics" }));

    await waitFor(() =>
      expect(within(dialog).getByRole("status")).toHaveTextContent(
        /purlis could not gather the diagnostics: .*the registry is gone/,
      ),
    );
    expect(await navigator.clipboard.readText()).toBe("before");
  });
});
