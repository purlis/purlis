// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { KeySaved, YourKeyRow } from "../bindings";
import { KeyForm } from "../KeyForm";
import { windowCopy } from "../windowCopy";
import { OnItsPage } from "./settingsStory.testkit";
import { yourKeysGroup } from "./yourKeys";

/**
 * **You › Your keys** (T1a-11, R-171, R-578), driven the way a person drives it, against core
 * faked at its command boundary: what the page lists is core's, *Remove…* always asks and has
 * no undo, the keyboard lands on the next row after it, *Stop sending* asks then calls core, and
 * the key form draws in place in every host and never as a dialog.
 */

function row(id: string, more: Partial<YourKeyRow> = {}): YourKeyRow {
  return {
    id,
    product: "Anthropic",
    origins: ["https://api.anthropic.com"],
    approval: "off",
    checked: "passed",
    state: "marked",
    profiles: [],
    chats: [],
    pools: [],
    ...more,
  };
}

/** What the page sent core, in order. */
let sent: { cmd: string; args: Record<string, unknown> }[] = [];
let rows: YourKeyRow[] = [];
let answers: Record<string, (args: Record<string, unknown>) => unknown> = {};

beforeEach(() => {
  sent = [];
  rows = [];
  answers = {};
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      sent.push({ cmd, args: given });
      const answer = answers[cmd];
      if (answer !== undefined) return answer(given);
      if (cmd === "your_keys") return rows;
      return null;
    },
    { shouldMockEvents: true },
  );
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const page = () => render(<OnItsPage group={yourKeysGroup()} />);
const called = (cmd: string) => sent.filter((one) => one.cmd === cmd);

describe("the page lists what core answers", () => {
  it("each key's product, origins, approval, profiles, chats and pools", async () => {
    rows = [
      row("01A", {
        profiles: ["claude-anthropic", "claude-work"],
        chats: ["Fix the parser"],
        pools: ["gpu", "lab"],
      }),
    ];
    page();
    const table = await screen.findByRole("table", { name: "Your keys" });
    expect(within(table).getByText("Anthropic")).toBeInTheDocument();
    expect(within(table).getByText("Sent to https://api.anthropic.com")).toBeInTheDocument();
    expect(within(table).getByText("Asks before each use: off")).toBeInTheDocument();
    expect(
      within(table).getByText("Used by claude-anthropic, claude-work. Chats: Fix the parser."),
    ).toBeInTheDocument();
    expect(within(table).getByRole("button", { name: "Stop sending to gpu" })).toBeInTheDocument();
    expect(within(table).getByRole("button", { name: "Stop sending to lab" })).toBeInTheDocument();
  });

  it("says No keys yet with Add a key… when core holds none", async () => {
    page();
    expect(await screen.findByText(windowCopy.W4)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add a key…" })).toBeInTheDocument();
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("says why the list can't be read, with Try again", async () => {
    let fail = true;
    answers.your_keys = () => {
      if (fail) throw new Error("the index is not your keys' index");
      return [row("01A")];
    };
    page();
    expect(
      await screen.findByText(
        windowCopy.W115({ error: "Error: the index is not your keys' index" }),
      ),
    ).toBeInTheDocument();
    fail = false;
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("table", { name: "Your keys" })).toBeInTheDocument();
  });
});

describe("Remove…", () => {
  it("always asks, with W5's count from core, and offers no undo", async () => {
    rows = [row("01A")];
    for (const count of [2, 1, 0]) {
      answers.your_key_chats = () => count;
      const { unmount } = page();
      await userEvent.click(
        await screen.findByRole("button", { name: "Remove the key for Anthropic" }),
      );
      const asked = await screen.findByRole("alertdialog");
      expect(within(asked).getByText(new RegExp(escape(windowCopy.W5({ count }))))).toBeVisible();
      expect(within(asked).getByText(/There is no undo\./)).toBeVisible();
      expect(called("your_key_remove")).toHaveLength(0);
      await userEvent.click(within(asked).getByRole("button", { name: "Cancel" }));
      unmount();
    }
    expect(called("your_key_remove")).toHaveLength(0);
    expect(screen.queryByRole("button", { name: /undo/i })).toBeNull();
  });

  it("removes through core, and the keyboard goes to the next row, or to Add a key…", async () => {
    rows = [row("01A"), row("01B", { product: "Other" })];
    answers.your_key_chats = () => 0;
    answers.your_key_remove = (args) => {
      rows = rows.filter((one) => one.id !== args.id);
      return null;
    };
    page();
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove the key for Anthropic" }),
    );
    await userEvent.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Remove" }),
    );
    expect(called("your_key_remove").map((one) => one.args.id)).toEqual(["01A"]);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Replace the key… for Other" })).toHaveFocus(),
    );

    await userEvent.click(screen.getByRole("button", { name: "Remove the key for Other" }));
    await userEvent.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Remove" }),
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "Add a key…" })).toHaveFocus());
  });

  it("that core refuses leaves the row, with W114 under it", async () => {
    rows = [row("01A")];
    answers.your_key_chats = () => 1;
    answers.your_key_remove = () => {
      throw new Error("your keychain didn't delete it");
    };
    page();
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove the key for Anthropic" }),
    );
    await userEvent.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Remove" }),
    );
    expect(
      await screen.findByText(windowCopy.W114({ error: "Error: your keychain didn't delete it" })),
    ).toBeInTheDocument();
    expect(screen.getByText("Anthropic")).toBeInTheDocument();
  });
});

describe("Stop sending", () => {
  it("asks with W123's count from core, then deletes that pool's row through core", async () => {
    rows = [row("01A", { pools: ["gpu", "lab"] })];
    answers.your_key_pool_chats = () => 2;
    answers.your_key_stop_sending = (args) => {
      rows = [row("01A", { pools: ["lab"] })];
      expect(args).toEqual({ id: "01A", pool: "gpu" });
      return null;
    };
    page();
    await userEvent.click(await screen.findByRole("button", { name: "Stop sending to gpu" }));
    const asked = await screen.findByRole("alertdialog");
    expect(within(asked).getByText(windowCopy.W123({ count: 2, pool: "gpu" }))).toBeVisible();
    expect(called("your_key_stop_sending")).toHaveLength(0);
    await userEvent.click(within(asked).getByRole("button", { name: "Stop sending" }));
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Stop sending to gpu" })).toBeNull(),
    );
    expect(screen.getByRole("button", { name: "Stop sending to lab" })).toBeInTheDocument();
  });

  it("that fails keeps the pool's line, with W129 under it", async () => {
    rows = [row("01A", { pools: ["gpu"] })];
    answers.your_key_pool_chats = () => 0;
    answers.your_key_stop_sending = () => {
      throw new Error("the allowance store is locked");
    };
    page();
    await userEvent.click(await screen.findByRole("button", { name: "Stop sending to gpu" }));
    await userEvent.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Stop sending" }),
    );
    expect(
      await screen.findByText(
        windowCopy.W129({ pool: "gpu", error: "Error: the allowance store is locked" }),
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop sending to gpu" })).toBeInTheDocument();
  });
});

describe("the key form", () => {
  it("draws in place in each host, and no dialog opens", async () => {
    // Your keys: Add a key… under the head, Replace the key… under the row.
    rows = [row("01A")];
    page();
    await userEvent.click(await screen.findByRole("button", { name: "Add a key…" }));
    expect(screen.getByLabelText(windowCopy.W1)).toHaveAttribute("type", "password");
    expect(screen.queryByRole("dialog")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await userEvent.click(screen.getByRole("button", { name: "Replace the key… for Anthropic" }));
    const table = screen.getByRole("table", { name: "Your keys" });
    expect(within(table).getByLabelText(windowCopy.W1)).toHaveAttribute("type", "password");
    expect(screen.queryByRole("dialog")).toBeNull();
    cleanup();

    // Any other host (J0's form, T1a-13; New chat's right pane, T1a-27) draws it inside itself.
    for (const host of ["Add profile", "New chat"]) {
      render(
        <section aria-label={host}>
          <KeyForm target={{ kind: "this-profile", name: "claude-anthropic" }} onSaved={() => {}} />
        </section>,
      );
      const inside = screen.getByRole("region", { name: host });
      expect(within(inside).getByLabelText(windowCopy.W1)).toHaveAttribute("type", "password");
      expect(screen.queryByRole("dialog")).toBeNull();
      cleanup();
    }
  });

  it("keeps the value out of React and the page, and W9's Replace the key… uses it with no second paste", async () => {
    const answered: KeySaved = { kind: "held", held: "01H", may_replace: true };
    answers.your_key_save = () => answered;
    answers.your_key_chats = () => 1;
    answers.your_key_replace_held = () => ({ kind: "saved", id: "01H" });
    let saved: string | undefined;
    render(
      <KeyForm
        target={{ kind: "this-profile", name: "claude-anthropic" }}
        onSaved={(id) => (saved = id)}
      />,
    );
    const box = screen.getByLabelText(windowCopy.W1);
    await userEvent.type(box, "synthetic-pasted-value");
    expect(document.body.innerHTML).not.toContain("synthetic-pasted-value");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText(windowCopy.W9)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: windowCopy.W2 })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Replace the key…" }));
    expect(await screen.findByText(windowCopy.W5({ count: 1 }))).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Replace the key" }));

    await waitFor(() => expect(saved).toBe("01H"));
    expect(called("your_key_replace_held").map((one) => one.args)).toEqual([
      { held: "01H", value: "synthetic-pasted-value", profile: "claude-anthropic" },
    ]);
  });

  it("aimed at a missing entry with another key held, offers only Use the key you already have", async () => {
    answers.your_key_save = () => ({ kind: "held", held: "01H", may_replace: false });
    render(<KeyForm target={{ kind: "this-entry", id: "01GONE" }} onSaved={() => {}} />);
    await userEvent.type(screen.getByLabelText(windowCopy.W1), "synthetic-value");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("button", { name: windowCopy.W2 })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Replace the key…" })).toBeNull();
    expect(screen.queryByText(windowCopy.W9)).toBeNull();
  });
});

describe("the window", () => {
  it("has no Unmark control for a model key: no command, and none on a row in any state", async () => {
    const bindings = readFileSync(join(process.cwd(), "src", "bindings.ts"), "utf8");
    expect(bindings.match(/\byourKey\w*/g)?.filter((name) => /unmark/i.test(name)) ?? []).toEqual(
      [],
    );
    rows = (["marked", "unmarked", "missing", "locked"] as const).map((state, at) =>
      row(`01${at}`, { state, product: `Product ${at}`, checked: "not checked yet" }),
    );
    page();
    await screen.findByRole("table", { name: "Your keys" });
    expect(screen.queryAllByRole("button", { name: /unmark/i })).toEqual([]);
    expect(screen.getAllByRole("button", { name: /^Mark again/ })).toHaveLength(1);
  });
});

function escape(words: string): string {
  return words.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
