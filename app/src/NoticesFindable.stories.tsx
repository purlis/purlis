// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Notices are findable** (Q182, Q183; R-331 to R-334): the Notices view's search box, one
 * filter per kind with its count, the Notices grouped by kind under a head that counts them
 * ("Clones made outside purlis · 2"), Dismissed (N), which brings a dismissed Notice back, and a
 * result whose words name Trust this clone, whose Go to puts the keyboard on that control.
 * Each held to axe and walked by the keyboard (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { ActionResultNotice } from "./ActionResultNotice";
import type { CloneToVouch } from "./bindings";
import { CloneVouchNotice, cloneVouchCause } from "./CloneVouchNotice";
import { Notice, NoticeList } from "./Notice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const clone = (folder: string, place: string): CloneToVouch =>
  ({ folder, place, places: [place] }) as unknown as CloneToVouch;

const CLONES = [
  clone("repos/web", "github.com/acme/web"),
  clone("repos/api", "github.com/acme/api"),
];

/** What the core says when a workspace clone's save cannot push (`forge/gitcredential.rs`). */
export const NOT_TRUSTED =
  "purlis did not clone this repository and you have not trusted it, so the save did not push with your purlis sign-in. If you made this clone yourself, press Trust this clone on its notice in the window, which names its remote, then save again";

/** A project's Notices: two clones made outside purlis, a stale repo and a doctor finding, each
 *  dismissed by its cause as the window's dismissals keep them. */
function TheNotices({ result = false }: { result?: boolean }) {
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  const [said, setSaid] = useState("Nothing pressed");
  const dismiss = (cause: string) => setDismissed((was) => new Set(was).add(cause));
  const showAgain = (cause: string) =>
    setDismissed((was) => new Set([...was].filter((one) => one !== cause)));
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: 560 }}>
      {result && (
        <ActionResultNotice
          cause="result:save"
          said={{ from: "story", refused: true, title: "web was not saved", words: NOT_TRUSTED }}
          onClose={() => undefined}
          onSettings={() => undefined}
        />
      )}
      <NoticeList findable={{ dismissed, showAgain }}>
        {CLONES.filter((one) => !dismissed.has(cloneVouchCause(one))).map((one) => (
          <CloneVouchNotice
            key={one.folder}
            clone={one}
            busy={false}
            onTrust={() => setSaid(`Trusted ${one.folder}`)}
            onDismiss={() => dismiss(cloneVouchCause(one))}
          />
        ))}
        {!dismissed.has("stale-clone:docs") && (
          <Notice
            cause="stale-clone:docs"
            label="docs in main"
            fixes={[{ label: "Switch to main", onPress: () => setSaid("Switched docs") }]}
            onDismiss={() => dismiss("stale-clone:docs")}
          >
            <p>docs is on spike/search, not main.</p>
          </Notice>
        )}
        <Notice
          cause="doctor-finding:hooks"
          tone="trouble"
          label="Hooks"
          fixes={[{ label: "Fix", onPress: () => setSaid("Fixed the hooks") }]}
        >
          <p>The project's hooks are not installed.</p>
        </Notice>
      </NoticeList>
      <p style={{ margin: 0 }} data-testid="said">
        {said}
      </p>
    </div>
  );
}

const meta = { component: TheNotices } satisfies Meta<typeof TheNotices>;
export default meta;
type Story = StoryObj<typeof meta>;

/** **R-331**: typing in the search box leaves the Notices that say what was typed. */
export const Searched: Story = {
  play: async ({ canvas, userEvent }) => {
    const search = canvas.getByRole("searchbox", { name: "Search the notices" });
    await userEvent.click(search);
    await userEvent.type(search, "acme/api");
    await waitFor(() => expect(canvas.queryByText(/repos\/web/)).toBeNull());
    await expect(canvas.getByText(/repos\/api/)).toBeVisible();
    await expect(canvas.queryByText(/hooks are not installed/)).toBeNull();
    await userEvent.clear(search);
    await userEvent.type(search, "nothing says this");
    await expect(await canvas.findByText("No notice says “nothing says this”.")).toBeVisible();
    await userEvent.clear(search);
    await waitFor(() => expect(canvas.getByText(/repos\/web/)).toBeVisible());
    expectFocusInSight();
  },
};

/** **R-332**: one filter per kind, each with its count; the Notices grouped under heads that
 *  count them; a filter leaves its kind alone, and pressed again shows every kind. */
export const ByKind: Story = {
  play: async ({ canvas, userEvent }) => {
    const filters = within(canvas.getByRole("group", { name: "Show notices of a kind" }));
    const names = filters.getAllByRole("button").map((one) => one.textContent);
    await expect(names).toEqual([
      "All · 4",
      // Trouble first: the most important kind leads.
      "Doctor findings · 1",
      "Clones made outside purlis · 2",
      "Stale repos · 1",
      "Dismissed (0)",
    ]);
    await expect(
      canvas.getByRole("heading", { name: "Clones made outside purlis · 2" }),
    ).toBeVisible();
    await expect(canvas.getByRole("heading", { name: "Doctor findings · 1" })).toBeVisible();
    const clones = filters.getByRole("button", { name: "Clones made outside purlis · 2" });
    await userEvent.click(clones);
    await expect(clones).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => expect(canvas.queryByText(/hooks are not installed/)).toBeNull());
    await expect(canvas.getAllByRole("button", { name: "Trust this clone" })).toHaveLength(2);
    await userEvent.keyboard(" ");
    await expect(clones).toHaveAttribute("aria-pressed", "false");
    await expect(await canvas.findByText(/hooks are not installed/)).toBeVisible();
    expectFocusInSight();
  },
};

/** **R-334**: a Notice dismissed is counted under Dismissed (N), listed there by its kind and
 *  what it is about, and Show again brings it back. */
export const BroughtBack: Story = {
  play: async ({ canvas, userEvent }) => {
    const web = canvas.getByText(/repos\/web/).closest<HTMLElement>("[data-cause]");
    await expect(web).not.toBeNull();
    await userEvent.click(within(web as HTMLElement).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(canvas.queryByText(/repos\/web/)).toBeNull());
    const filters = within(canvas.getByRole("group", { name: "Show notices of a kind" }));
    const gone = filters.getByRole("button", { name: "Dismissed (1)" });
    await userEvent.click(gone);
    const list = within(canvas.getByRole("list", { name: "Dismissed notices" }));
    await expect(list.getAllByRole("listitem")).toHaveLength(1);
    await expect(list.getByText("Clone made outside purlis")).toBeVisible();
    await expect(list.getByText("repos/web")).toBeVisible();
    await userEvent.click(list.getByRole("button", { name: "Show again" }));
    await expect(filters.getByRole("button", { name: "Dismissed (0)" })).toBeVisible();
    await userEvent.click(filters.getByRole("button", { name: "All · 4" }));
    await expect(await canvas.findByText(/repos\/web/)).toBeVisible();
    expectFocusInSight();
  },
};

/** **R-333**: a result whose words name Trust this clone carries Go to Trust this clone, which
 *  shows the clones' kind and puts the keyboard on that control. */
export const ToTheControl: Story = {
  args: { result: true },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Go to Trust this clone" }));
    const filters = within(canvas.getByRole("group", { name: "Show notices of a kind" }));
    await waitFor(() =>
      expect(
        filters.getByRole("button", { name: "Clones made outside purlis · 2" }),
      ).toHaveAttribute("aria-pressed", "true"),
    );
    await untilFocused(
      () => canvas.getAllByRole("button", { name: "Trust this clone" })[0],
      "on the first clone's Trust this clone",
    );
    expectFocusInSight();
  },
};
