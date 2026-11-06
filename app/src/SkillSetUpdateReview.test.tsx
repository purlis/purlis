// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Review updates for several skills** (#5354) at the window seam, the core faked at its command
 * boundary: a plain edit starts ticked, each kind of new capability starts unticked and is ticked
 * only once opened, nothing ticks them all, and accepting is one command with one Undo. What the
 * core does with them is its seam test (`skills/source/tests/update_set.rs`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillGained } from "./bindings";
import { PROJECT } from "./skillChanges";
import { updateOffer as offer, updateReview as review } from "./skillUpdates.testkit";
import { SetUpdateReviewPage, applySetUpdate, gainedSaid } from "./SkillSetUpdateReview";

afterEach(() => {
  cleanup();
  clearMocks();
});

/** A core offering an update of each of `gains`' skills, gaining what it names. */
function core(gains: Record<string, SkillGained[]>) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    switch (cmd) {
      case "skill_updates":
        return {
          offers: Object.keys(gains).map((name) => offer(name)),
          unasked: [],
          unconfirmed: [],
        };
      case "skill_update_review":
        return review(given.name as string, gains[given.name as string]);
      case "skill_set_update":
        return { notes: [], undo: 7 };
      case "skill_set_update_undo":
        return { notes: [] };
      default:
        return null;
    }
  });
  return asked;
}

function page(names: string[], onApply = vi.fn()) {
  render(
    <SetUpdateReviewPage
      plane="p"
      title="Review updates for 3 skills"
      members={names.map(member)}
      busy={false}
      onCancel={() => {}}
      onApply={onApply}
    />,
  );
  return onApply;
}

const member = (name: string) => ({ level: PROJECT, name, label: "project" });

const box = (name: string) => screen.getByRole("checkbox", { name: `Update ${name} (project)` });

describe("Review updates", () => {
  it("ticks a plain edit, leaves one that adds a hook unticked and lists it first", async () => {
    core({ tidy: [], hooky: ["hook"] });
    page(["tidy", "hooky"]);
    await waitFor(() => expect(box("hooky")).toBeInTheDocument());
    expect(box("tidy")).toBeChecked();
    expect(box("hooky")).not.toBeChecked();
    const rows = within(screen.getByTestId("update-set-list")).getAllByRole("checkbox");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toBe(box("hooky"));
    expect(screen.getByTestId("update-set-lead")).toHaveTextContent(
      "1 adds something new and starts unticked: open it to tick it.",
    );
  });

  it.each<SkillGained>([
    "script",
    "command",
    "hook",
    "tool",
    "network",
    "writes-outside",
    "hidden",
    "not-shown",
    "unread",
  ])(
    "starts an update that adds %s unticked, and lets it be ticked only once opened",
    async (kind) => {
      core({ newer: [kind] });
      const onApply = page(["newer"]);
      await waitFor(() => expect(box("newer")).toBeInTheDocument());
      expect(box("newer")).not.toBeChecked();
      expect(box("newer")).toBeDisabled();
      expect(screen.getByTestId("update-set-project-newer")).toHaveTextContent(gainedSaid([kind]));
      const accept = screen.getByRole("button", { name: "Accept updates" });
      expect(accept).toHaveAttribute("aria-disabled", "true");

      await userEvent.click(screen.getByRole("button", { name: "What changes in newer" }));
      await waitFor(() => expect(box("newer")).toBeEnabled());
      await userEvent.click(box("newer"));
      expect(box("newer")).toBeChecked();
      await userEvent.click(screen.getByRole("button", { name: "Accept 1 update" }));
      expect(onApply).toHaveBeenCalledWith([
        expect.objectContaining({ name: "newer", allowed: [kind] }),
      ]);
    },
  );

  it("offers nothing that ticks every update at once", async () => {
    core({ a: ["script"], b: ["network"], c: [] });
    page(["a", "b", "c"]);
    await waitFor(() => expect(box("c")).toBeChecked());
    expect(screen.queryByRole("button", { name: /all/i })).toBeNull();
    expect(screen.getByRole("button", { name: "Accept 1 update" })).toBeInTheDocument();
  });

  it("says why a chosen skill with no update waiting stays as it is", async () => {
    core({ tidy: [] });
    page(["tidy", "quiet"]);
    await waitFor(() =>
      expect(screen.getByTestId("set-stays-quiet")).toHaveTextContent("No update is waiting."),
    );
  });

  it("accepts as one command and undoes with the Undo it answered", async () => {
    const asked = core({});
    const said = vi.fn();
    const change = vi.fn(
      async (
        _label: string,
        run: () => Promise<unknown>,
        then?: (notes: readonly string[]) => Promise<unknown>,
      ) => {
        await run();
        said(await then?.([]));
      },
    );
    applySetUpdate({
      plane: "p",
      updates: [
        {
          level: PROJECT,
          name: "tidy",
          reviewed: { commit: "2".repeat(40), skills: [] },
          grant: null,
          allowed: [],
        },
      ],
      change,
      undo: change,
    });
    await waitFor(() => expect(said).toHaveBeenCalled());
    const done = said.mock.calls[0][0] as { text: string; undo: () => void };
    expect(done.text).toBe("1 skill updated.");
    done.undo();
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_set_update_undo")?.args).toEqual({
        plane: "p",
        undo: 7,
      }),
    );
    expect(asked.filter((one) => one.cmd === "skill_set_update")).toHaveLength(1);
  });
});
