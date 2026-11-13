// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Your hosts, as a table** (#2919, prototype A of `prototype/settings-page`, R65, R67–R74):
 * nine hosts, two waiting for your Confirm, first; the bar with its filter, Confirm all and Add
 * host; rows chosen, the bar saying how many with Confirm and Remove; and a filter that leaves
 * none. Each story is drawn, held to axe, walked by the keyboard and pictured in every pictured
 * theme cell (`docs/component-stories.md`).
 */
import { useRef } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import type { CollectionEntry } from "./groups";
import { HostsTable } from "./HostsTable";

const WAITING = new Set(["zed.dev", "fork.dev"]);

/** The moment the stories tell each Added age from, held still so the pictures are. */
const NOW = new Date(Date.UTC(2026, 10, 12, 12, 0, 0));
const SECONDS = NOW.getTime() / 1000;

/** When and by what each was added (#6172, R65); a host with none was added before the record. */
const ADDED: Record<string, CollectionEntry["added"]> = {
  "productionresultssa9.blob.core.windows.net": { at: SECONDS - 90 * 86_400, by: "you" },
  "docs.gitlab.com": { at: SECONDS - 3 * 86_400, by: "you" },
  "zed.dev": { at: SECONDS - 20 * 60, by: "block" },
  "docs.github.com": { at: SECONDS - 2 * 3600, by: "block" },
  "netlog-git.invalid:8443": { at: SECONDS - 86_400, by: "you" },
  "fork.dev": { at: SECONDS - 30, by: "block" },
};

const HOSTS: readonly CollectionEntry[] = [
  "productionresultssa9.blob.core.windows.net",
  "docs.gitlab.com",
  "tuf-repo-cdn.sigstore.dev",
  "zed.dev",
  "docs.github.com",
  "netlog-git.invalid:8443",
  "fork.dev",
  "code.visualstudio.com",
  "www.jetbrains.com",
].map((host, at) => ({
  id: `host:${at}:${host}`,
  label: host,
  host,
  settings: [],
  confirm: WAITING.has(host),
  status: WAITING.has(host) ? "waiting" : "confirmed",
  added: ADDED[host],
}));

/** The table on a page's width, as Settings draws it. */
function Hosts(props: {
  onConfirm: (entry: CollectionEntry) => void;
  onRemove: (entry: CollectionEntry) => void;
  onConfirmAll: (hosts: readonly string[], every: boolean) => void;
  onRemoveAll: (hosts: readonly string[]) => void;
  onAdd: () => void;
}) {
  const add = useRef<HTMLButtonElement>(null);
  return (
    <div className="ui-settings-page">
      <HostsTable
        label="Your hosts"
        entries={HOSTS}
        busy={false}
        addRef={add}
        now={NOW}
        {...props}
      />
    </div>
  );
}

const meta = {
  component: Hosts,
  args: {
    onConfirm: fn(),
    onRemove: fn(),
    onConfirmAll: fn(),
    onRemoveAll: fn(),
    onAdd: fn(),
  },
} satisfies Meta<typeof Hosts>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **Waiting first** (R65, R67, R68, R70): the filter, then Confirm all and Add host; the table
 * one Tab stop on its first row, whose Confirm the arrows reach; each row says when and by what
 * it was added, or that it was added earlier (#6172).
 */
export const WaitingFirst: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const filter = canvas.getByRole("searchbox", { name: "Filter hosts" });
    await expect(filter).toHaveAttribute("placeholder", "Filter 9 hosts");
    filter.focus();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Confirm all 2 waiting" }),
      "Confirm all",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onConfirmAll).toHaveBeenCalledWith(["zed.dev", "fork.dev"], true);
    const first = canvas.getAllByRole("row")[1] as HTMLElement;
    await expect(within(first).getByRole("rowheader")).toHaveTextContent("zed.dev:443");
    await expect(within(first).getByText("Waiting for you")).toBeVisible();
    await expect(within(first).getByText("20 minutes ago · a chat's block")).toBeVisible();
  },
};

/** **Rows chosen** (R70, R71): the head's check chooses every row; the bar says how many. */
export const RowsChosen: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("checkbox", { name: "Select zed.dev:443" }));
    await userEvent.click(canvas.getByRole("checkbox", { name: "Select docs.github.com:443" }));
    const bar = canvas.getByRole("group", { name: "Your hosts" });
    await waitFor(() => expect(within(bar).getByText("2 selected")).toBeVisible());
    const remove = within(bar).getByRole("button", { name: "Remove" });
    remove.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onRemoveAll).toHaveBeenCalledWith(["zed.dev", "docs.github.com"]);
  },
};

/** **A filter that leaves none** (R69, R73): the table says so where its rows would be. */
export const NoneLeft: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const filter = canvas.getByRole("searchbox", { name: "Filter hosts" });
    await userEvent.click(filter);
    await userEvent.keyboard("nothing.example");
    await waitFor(() =>
      expect(canvas.getByText("No host is called “nothing.example”.")).toBeVisible(),
    );
    await untilFocused(() => filter, "the filter keeps the keyboard");
    expectFocusInSight();
  },
};
