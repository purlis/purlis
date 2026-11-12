// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Links in this pane…** (#6220, the accepted prototype's `LinksInThisPane`, S5): a real xterm,
 * the read-only terminal's, holding a harness's turn with three addresses in it, one the core
 * will not open. ⌘⇧L (Ctrl+Shift+L off a Mac) lists them, newest first. The keyboard walks the list, a refused entry says
 * why and keeps the keyboard, Enter opens one, and Esc closes it.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { onAMac } from "./mac";
import { LINK_CORE } from "./paneLinkCore";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

const FTP = "purlis opens only http and https addresses, so it will not open this ftp: one";

const LINKED = [
  "\u001b[1m● Read the setup first.\u001b[0m",
  "  https://purlis.dev/setup",
  "  The old mirror is ftp://mirror.purlis.dev/purlis (gone).",
  "\u001b[2m  ⎿ then https://github.com/purlis/purlis/pull/6148\u001b[22m",
].join("\r\n");

const PLAIN = ["\u001b[1m● The resolver's tests pass.\u001b[0m", "  ⎿ 12 tests, 0 failed"].join(
  "\r\n",
);

function Pane({ screen }: { screen: string }) {
  return (
    <div style={{ inlineSize: 640, paddingBlockEnd: "var(--space-8)" }}>
      <ReadOnlyTerminal
        screen={screen}
        label="A saved chat's last screen"
        cols={72}
        rows={5}
        fontSize={13}
        links={LINK_CORE}
      />
    </div>
  );
}

const meta = {
  title: "PaneLinksList",
  component: Pane,
  args: { screen: LINKED },
  // The dialog is drawn over the page, outside the story's frame: the page is pictured.
  parameters: { screenshot: "page" },
  beforeEach: () => {
    mockIPC((cmd, args) => {
      if (cmd === "pane_link_resolve") {
        const { candidates } = args as { candidates: { text: string }[] };
        return candidates.map((one) =>
          one.text.startsWith("ftp://")
            ? { kind: "url", target: one.text, does: { does: "refused", reason: FTP } }
            : { kind: "url", target: one.text, does: { does: "opens" } },
        );
      }
      if (cmd === "pane_link_open")
        return { answer: "opened", kind: "url", target: "https://purlis.dev/setup" };
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Pane>;
export default meta;

/** Opens the list from the keyboard alone: the screen's region, then ⌘⇧L. */
async function listed(canvasElement: HTMLElement) {
  const page = within(canvasElement.ownerDocument.body);
  const region = await page.findByRole("region", { name: "A saved chat's last screen" });
  await waitFor(() =>
    expect(canvasElement.querySelector(".xterm-screen")?.textContent ?? "").toMatch(
      /setup first|tests pass/,
    ),
  );
  region.focus();
  await userEvent.keyboard(
    onAMac() ? "{Meta>}{Shift>}L{/Shift}{/Meta}" : "{Control>}{Shift>}L{/Shift}{/Control}",
  );
  return { page, dialog: await page.findByRole("dialog", { name: "Links in this pane" }) };
}

/** **The links on screen, newest first**: each says its target and kind; one won't open. */
export const LinksInThisPane: StoryObj<typeof meta> = {
  play: async ({ canvasElement }) => {
    const { dialog } = await listed(canvasElement);
    const filter = within(dialog).getByRole("searchbox", { name: "Filter links" });
    await untilFocused(() => filter, "The filter takes the keys");
    const rows = await within(dialog).findAllByRole("option");
    await expect(rows.map((row) => row.getAttribute("aria-disabled"))).toEqual([
      null,
      "true",
      null,
    ]);
    await expect(rows[0]).toHaveAccessibleName("https://github.com/purlis/purlis/pull/6148");
    await expect(rows[1]).toHaveAccessibleDescription(/Web address · won't open/);
    await pictureNow();

    // Into the list, and down to the refused entry: it keeps the keyboard and says why.
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows[0], "The down arrow goes into the list");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows[1], "The refused entry keeps the keyboard");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(dialog).toBeInTheDocument();

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(within(document.body).queryByRole("dialog")).toBeNull());
  },
};

/** **Enter on an entry opens it** and closes the list. */
export const PickALink: StoryObj<typeof meta> = {
  play: async ({ canvasElement }) => {
    const { dialog } = await listed(canvasElement);
    const rows = await within(dialog).findAllByRole("option");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    await untilFocused(() => rows[2], "The oldest entry");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(within(document.body).queryByRole("dialog")).toBeNull());
  },
};

/** **No links on screen**: the list says so. */
export const NoLinks: StoryObj<typeof meta> = {
  args: { screen: PLAIN },
  play: async ({ canvasElement }) => {
    const { dialog } = await listed(canvasElement);
    const none = await within(dialog).findByText("No links on screen.");
    // Once the dialog has come in: it fades in (WebKit reads it as hidden until then).
    await waitFor(() => expect(none).toBeVisible());
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(within(document.body).queryByRole("dialog")).toBeNull());
  },
};
