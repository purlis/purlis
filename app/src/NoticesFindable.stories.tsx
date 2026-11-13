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

/** The project the stories' Notices and results are of. */
const PROJECT = "/home/dev/plane";

/** What the core says when a workspace clone's save cannot push (`forge/gitcredential.rs`). */
export const NOT_TRUSTED =
  "purlis did not clone this repository and you have not trusted it, so the save did not push with your purlis sign-in. If you made this clone yourself, press Trust this clone on its notice in the window, which names its remote, then save again";

/** A project's Notices: two clones made outside purlis, a stale repo and a doctor finding, each
 *  dismissed by its cause as the window's dismissals keep them. */
function TheNotices({
  result = false,
  title = "repos/api was not saved",
  project = PROJECT,
  dismissedFirst = [],
  clones = CLONES,
}: {
  result?: boolean;
  /** The result's title, which names the clone it is about. */
  title?: string;
  project?: string;
  dismissedFirst?: readonly string[];
  clones?: readonly CloneToVouch[];
}) {
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set(dismissedFirst));
  /** The clones trusted: each leaves the list, as a clone trusted in the window does. */
  const [trusted, setTrusted] = useState<ReadonlySet<string>>(new Set());
  const [said, setSaid] = useState("Nothing pressed");
  const dismiss = (cause: string) => setDismissed((was) => new Set(was).add(cause));
  const showAgain = (cause: string) =>
    setDismissed((was) => new Set([...was].filter((one) => one !== cause)));
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: 560 }}>
      {result && (
        <ActionResultNotice
          cause="result:save"
          said={{ from: "story", refused: true, title, words: NOT_TRUSTED }}
          onClose={() => undefined}
          onSettings={() => undefined}
          project={PROJECT}
        />
      )}
      <NoticeList findable={{ project, dismissed, showAgain }}>
        {clones
          .filter((one) => !dismissed.has(cloneVouchCause(one)) && !trusted.has(one.folder))
          .map((one) => (
            <CloneVouchNotice
              key={one.folder}
              clone={one}
              busy={false}
              onTrust={() => {
                setTrusted((was) => new Set(was).add(one.folder));
                setSaid(`Trusted ${one.folder}`);
              }}
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
    await expect(search).toHaveAttribute("placeholder", "Search the notices");
    await userEvent.click(search);
    await userEvent.type(search, "acme/api");
    await waitFor(() => expect(canvas.queryByText(/repos\/web/)).toBeNull());
    await expect(canvas.getByText(/repos\/api/)).toBeVisible();
    await expect(canvas.queryByText(/hooks are not installed/)).toBeNull();
    await userEvent.clear(search);
    await userEvent.type(search, "nothing says this");
    await expect(await canvas.findByText("No notice matches “nothing says this”.")).toBeVisible();
    // What it reads is what a Notice says, never a button's label.
    await userEvent.clear(search);
    await userEvent.type(search, "switch");
    await expect(await canvas.findByText("No notice matches “switch”.")).toBeVisible();
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
    await expect(list.getByText("repos/web").closest("code")).toBeNull();
    await expect(canvas.getByText(/purlis keeps dismissed until it changes/)).toBeVisible();
    await userEvent.click(list.getByRole("button", { name: "Show again" }));
    await expect(filters.getByRole("button", { name: "Dismissed (0)" })).toBeVisible();
    // The keyboard never falls to the page: the row gone, it is on Dismissed.
    await untilFocused(
      () => filters.getByRole("button", { name: "Dismissed (0)" }),
      "on Dismissed once its last row is shown again",
    );
    await expect(canvas.getByText("Nothing is kept dismissed here")).toBeVisible();
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
    // On the clone the result names (api), not the first one listed.
    await untilFocused(
      () =>
        canvas
          .getAllByRole("button", { name: "Trust this clone" })
          .find((one) => one.closest('[data-cause*="repos/api"]') !== null) ?? null,
      "on api's Trust this clone",
    );
    expectFocusInSight();
  },
};

/** **A kind that empties falls back to All** (R-332, R-333): after Go to, Trust this clone on
 *  the last clone leaves no Notice of its kind, so every kind is shown, its filter pressed, and
 *  what stands is listed; never a blank view. */
export const ToTheControlThenTrusted: Story = {
  args: { result: true, clones: [CLONES[1]] },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Go to Trust this clone" }));
    const filters = within(canvas.getByRole("group", { name: "Show notices of a kind" }));
    await waitFor(() =>
      expect(
        filters.getByRole("button", { name: "Clones made outside purlis · 1" }),
      ).toHaveAttribute("aria-pressed", "true"),
    );
    await untilFocused(
      () => canvas.queryByRole("button", { name: "Trust this clone" }),
      "on api's Trust this clone",
    );
    // Pressed from the keyboard, where Go to left it.
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(filters.getByRole("button", { name: "All · 2" })).toHaveAttribute(
        "aria-pressed",
        "true",
      ),
    );
    await expect(canvas.getByText(/docs is on spike\/search/)).toBeVisible();
    await expect(canvas.getByText(/hooks are not installed/)).toBeVisible();
  },
};

/** **Go to the clone a result names** (R-333): by its folder's path, or its exact name in quotes
 *  or code, never by a bare word, so folders named "clone" and "save", listed first, are not
 *  taken for the “web” the result names (its words say "Trust this clone" and "save again"). */
export const ToTheNamedFolder: Story = {
  args: {
    result: true,
    title: "“web” was not saved",
    clones: [
      clone("repos/clone", "github.com/acme/clone"),
      clone("repos/save", "github.com/acme/save"),
      CLONES[0],
    ],
  },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Go to Trust this clone" }));
    await untilFocused(
      () =>
        canvas
          .getAllByRole("button", { name: "Trust this clone" })
          .find((one) => one.closest('[data-cause*="repos/web|"]') !== null) ?? null,
      "on web's Trust this clone",
    );
    expectFocusInSight();
  },
};

/** **Go to never ends blank** (R-333): the clone it names was dismissed, so Go to shows
 *  Dismissed with the keyboard on that clone's Show again. */
export const ToADismissedControl: Story = {
  args: {
    result: true,
    dismissedFirst: [`clone-vouch:repos/api|github.com/acme/api`],
    clones: [CLONES[1]],
  },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Go to Trust this clone" }));
    const filters = within(canvas.getByRole("group", { name: "Show notices of a kind" }));
    await waitFor(() =>
      expect(filters.getByRole("button", { name: "Dismissed (1)" })).toHaveAttribute(
        "aria-pressed",
        "true",
      ),
    );
    await untilFocused(
      () => within(canvas.getByRole("list", { name: "Dismissed notices" })).getByRole("button"),
      "on the dismissed clone's Show again",
    );
    expectFocusInSight();
  },
};

/** **Go to never ends blank** (R-333): no Notice of its kind stands or is dismissed, so every
 *  kind is shown, and what stands is listed. */
export const ToAControlNoLongerThere: Story = {
  args: { result: true, clones: [] },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Go to Trust this clone" }));
    const filters = within(canvas.getByRole("group", { name: "Show notices of a kind" }));
    await waitFor(() =>
      expect(filters.getByRole("button", { name: "All · 2" })).toHaveAttribute(
        "aria-pressed",
        "true",
      ),
    );
    await expect(canvas.getByText(/hooks are not installed/)).toBeVisible();
    await userEvent.tab();
    expectFocusInSight();
  },
};

/** **A Go to is its project's** (R-333): another project's Notices view leaves it alone. */
export const AnotherProjectsGoTo: Story = {
  args: { result: true, project: "/home/dev/other" },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Go to Trust this clone" }));
    const filters = within(canvas.getByRole("group", { name: "Show notices of a kind" }));
    await expect(filters.getByRole("button", { name: "All · 4" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await expect(canvas.getByRole("button", { name: "Go to Trust this clone" })).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Show again hands the keyboard on** (WCAG 2.4.3): with two dismissed, to the row that took
 *  the place of the one shown again. The search finds in the Dismissed list too. */
export const ShownAgainOneOfTwo: Story = {
  args: {
    dismissedFirst: [
      `clone-vouch:repos/web|github.com/acme/web`,
      `clone-vouch:repos/api|github.com/acme/api`,
    ],
  },
  play: async ({ canvas, userEvent }) => {
    const filters = within(canvas.getByRole("group", { name: "Show notices of a kind" }));
    await userEvent.click(filters.getByRole("button", { name: "Dismissed (2)" }));
    const list = () => within(canvas.getByRole("list", { name: "Dismissed notices" }));
    const search = canvas.getByRole("searchbox", { name: "Search the notices" });
    await userEvent.type(search, "api");
    await waitFor(() => expect(list().getAllByRole("listitem")).toHaveLength(1));
    await userEvent.clear(search);
    await waitFor(() => expect(list().getAllByRole("listitem")).toHaveLength(2));
    await userEvent.click(list().getAllByRole("button", { name: "Show again" })[0]);
    await untilFocused(
      () => list().getByRole("button", { name: "Show again" }),
      "on the dismissed row that took its place",
    );
    expectFocusInSight();
  },
};
