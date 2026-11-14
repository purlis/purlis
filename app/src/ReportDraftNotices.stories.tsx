// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's report draft in the Inbox** (#5694, spec #5683): the card the Notices view draws for
 * a report a chat drafted with `purlis report`, as the reviewed prototype drew it
 * (`InboxReportDraft`). Its head names the chat it came from with Go to chat; Read opens the whole
 * draft, marked as an agent's and unreviewed, in fields the person can change; File files it
 * through the duplicate check; Dismiss lets it go. The core is faked at its command boundary: a
 * story reads which commands it was sent. And the update a filed draft leaves.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import type { DraftFiled, InboxUpdate, ReportDraft } from "./bindings";
import { Inbox } from "./Inbox";
import { DISMISS_SAYS, ReportDraftNotices, WRITTEN_BY_AN_AGENT } from "./ReportDraftNotices";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const TITLE = "purlis pin refuses a workspace it just listed";

const DRAFT: ReportDraft = {
  id: "d1",
  at: "2026-11-08T10:00:00Z",
  feature: false,
  title: TITLE,
  body:
    "purlis pin refuses a workspace it just listed\n\nIt said the workspace was not there. " +
    "`purlis workspace list` had just listed it.\n\n---\npurlis 0.41.2 · macos aarch64\n_Filed with " +
    "`purlis report bug`._",
  line: "purlis pin · exit 1 · purlis 0.41.2 · Darwin 27.2.0",
  scrubbed: ["home paths"],
  chat: "steward 4",
  session: 4,
  edited: false,
  digest: "0a1b2c3d4e5f",
  sends_at: null,
};

/** What the faked core answers File with, and every command it was sent. */
const core: { filed: DraftFiled[]; sent: { cmd: string; args: unknown }[] } = {
  filed: [],
  sent: [],
};

/** The faked core: one draft held, answering File with each of `filed` in turn. */
function fake(filed: DraftFiled[], membersMayAlwaysSend = true) {
  core.filed = [...filed];
  core.sent = [];
  mockIPC((cmd, args) => {
    core.sent.push({ cmd, args });
    if (cmd === "report_drafts") return [DRAFT];
    if (cmd === "members_may_always_send") return membersMayAlwaysSend;
    if (cmd === "edit_report_draft") {
      const { title, body } = args as { title: string; body: string };
      return { ...DRAFT, title, body, edited: true, digest: "ffffffffffff" };
    }
    if (cmd === "file_report_draft") return core.filed.shift();
    return null;
  });
  return () => clearMocks();
}

const sentTo = (cmd: string) =>
  core.sent.filter((one) => one.cmd === cmd).map((one) => one.args as Record<string, unknown>);

const INTAKE: DraftFiled = {
  kind: "sent",
  to: "intake",
  address: "purlis's report service, event 9f",
  title: TITLE,
};

const DUPLICATES: DraftFiled = {
  kind: "duplicates",
  issues: [
    {
      number: 41,
      title: "pin refuses workspaces it lists",
      state: "open",
      url: "https://github.com/purlis/purlis/issues/41",
    },
  ],
};

const ADDED: DraftFiled = {
  kind: "sent",
  to: "github",
  address: "https://github.com/purlis/purlis/issues/41#issuecomment-9001",
  title: TITLE,
};

/** The Inbox as wide as the window's side of it, its Notices holding the project's drafts. */
function TheInbox({ onGo, onSent }: Parameters<typeof ReportDraftNotices>[0]) {
  return (
    <div style={{ inlineSize: 460 }}>
      <Inbox
        plane="/plane"
        asks={[]}
        onGo={() => undefined}
        onLeave={() => undefined}
        onIgnore={() => undefined}
        notices={<ReportDraftNotices plane="/plane" onGo={onGo} onSent={onSent} />}
      />
    </div>
  );
}

const meta = {
  component: TheInbox,
  args: { plane: "/plane", onGo: fn(), onSent: fn() },
} satisfies Meta<typeof TheInbox>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The card, once the faked core listed its draft. */
async function theCard(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await waitFor(() => expect(canvas.getByText(`Report draft: ${TITLE}`)).toBeVisible());
  return canvas;
}

/** **The card as it waits**: its title, the chat it came from in its head, and its line. Go to
 *  chat goes to that chat. */
export const ADraftWaits: Story = {
  beforeEach: () => fake([]),
  play: async ({ canvasElement, args }) => {
    const canvas = await theCard(canvasElement);
    await expect(canvas.getByText("steward 4")).toBeVisible();
    await expect(canvas.getByText(DRAFT.line)).toBeVisible();
    const go = canvas.getByRole("button", { name: "Go to chat steward 4" });
    go.focus();
    expectFocusInSight();
    // The × stands in the row's time once the row holds the keyboard (or the pointer).
    await waitFor(() => expect(canvas.getByRole("button", { name: "Dismiss" })).toBeVisible());
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onGo).toHaveBeenCalledWith(4));
  },
};

/** **Read**: the whole draft, marked as an agent's and unreviewed, in fields the person can
 *  change, reached by the keyboard from Read past the row's ways and its ×. */
export const ReadOpensTheDraft: Story = {
  beforeEach: () => fake([]),
  play: async ({ canvasElement }) => {
    const canvas = await theCard(canvasElement);
    const read = canvas.getByRole("button", { name: "Read" });
    read.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByText(WRITTEN_BY_AN_AGENT)).toBeVisible());
    await expect(read).toHaveAttribute("aria-expanded", "true");
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "File" }), "Tab reaches File");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Always send" }),
      "Tab reaches Always send",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Go to chat steward 4" }),
      "Tab reaches Go to chat, the row's last way",
    );
    // The row's × is its last stop; what Read opened stands under the whole row, so after it.
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Dismiss" }),
      "Tab reaches the row's Dismiss",
    );
    await userEvent.tab();
    const title = canvas.getByRole("textbox", { name: "Title" });
    await untilFocused(() => title, "Tab reaches the draft's title");
    expectFocusInSight();
    await expect(title).toHaveValue(TITLE);
    await expect(canvas.getByRole("textbox", { name: "Body" })).toHaveValue(DRAFT.body);
  },
};

/** **File, with no GitHub sign-in**: the core sends it to the intake, the card goes, and the
 *  window is told it was sent. A changed title is kept first, and the new digest filed. */
export const FiledToTheReportService: Story = {
  beforeEach: () => fake([INTAKE]),
  parameters: { screenshot: false },
  play: async ({ canvasElement, args }) => {
    const canvas = await theCard(canvasElement);
    canvas.getByRole("button", { name: "Read" }).focus();
    await userEvent.keyboard("{Enter}");
    const title = await waitFor(() => canvas.getByRole("textbox", { name: "Title" }));
    await userEvent.clear(title);
    await userEvent.type(title, "pin refuses a listed workspace");
    canvas.getByRole("button", { name: "File" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onSent).toHaveBeenCalled());
    await expect(sentTo("edit_report_draft")[0]).toMatchObject({
      id: "d1",
      title: "pin refuses a listed workspace",
    });
    await expect(sentTo("file_report_draft")[0]).toEqual({
      id: "d1",
      digest: "ffffffffffff",
      how: { how: "check" },
    });
    await waitFor(() => expect(canvas.queryByText(`Report draft: ${TITLE}`)).toBeNull());
  },
};

/**
 * **Always send, held by the project** (#5696): the project turns sending without asking off, so
 * the card's Always send is drawn off, its reason its tooltip, and the keyboard still reaches it;
 * a press asks nothing and sends nothing.
 */
export const AlwaysSendHeldByTheProject: Story = {
  beforeEach: () => fake([], false),
  play: async ({ canvasElement }) => {
    const canvas = await theCard(canvasElement);
    const always = canvas.getByRole("button", { name: "Always send" });
    await waitFor(() => expect(always).toHaveAttribute("aria-disabled", "true"));
    always.focus();
    await untilFocused(() => always, "the keyboard reaches Always send, off");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(within(document.body).queryByRole("dialog")).toBeNull();
    await expect(sentTo("always_send_report_draft")).toEqual([]);
  },
};

/**
 * **Always send, asked once** (#5695): pressed while the person has not said yes, it asks first,
 * saying where drafts will go and where to see and stop them. Return answers neither, and
 * nothing is sent while it asks. Pictured with the question open.
 */
export const AlwaysSendAsksOnce: Story = {
  beforeEach: () => fake([]),
  play: async ({ canvasElement }) => {
    const canvas = await theCard(canvasElement);
    canvas.getByRole("button", { name: "Always send" }).focus();
    await userEvent.keyboard("{Enter}");
    const body = within(document.body);
    const dialog = await body.findByRole("dialog", { name: "Always send report drafts?" });
    // In the DOM is not yet on screen: the scrim fades in from nothing as it opens
    // (`ui/dialog/dialog.css`), so the keyboard waits until the dialog is seen (#6628).
    await waitFor(() => expect(dialog).toBeVisible());
    await expect(dialog).toHaveTextContent(/private report service/);
    await expect(dialog).toHaveTextContent(/never go to GitHub/);
    await expect(dialog).toHaveTextContent(/Settings › You › Reports/);
    await userEvent.keyboard("{Enter}");
    await expect(sentTo("always_send_report_draft")).toEqual([]);
    await userEvent.tab();
    expectFocusInSight();
    await expect(body.getByRole("dialog")).toBeVisible();
  },
};

/** **A possible duplicate**: File lists it under the card, nothing filed yet; Add to #41 adds the
 *  draft to that issue instead of filing it. */
export const PossibleDuplicates: Story = {
  beforeEach: () => fake([DUPLICATES, ADDED]),
  play: async ({ canvasElement, args }) => {
    const canvas = await theCard(canvasElement);
    canvas.getByRole("button", { name: "File" }).focus();
    await userEvent.keyboard("{Enter}");
    const add = await waitFor(() => canvas.getByRole("button", { name: "Add to #41" }));
    await expect(canvas.getByRole("button", { name: "File as new" })).toBeVisible();
    await expect(sentTo("file_report_draft")).toEqual([
      { id: "d1", digest: DRAFT.digest, how: { how: "check" } },
    ]);
    await expect(args.onSent).not.toHaveBeenCalled();
    add.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onSent).toHaveBeenCalled());
    await expect(sentTo("file_report_draft")[1]).toEqual({
      id: "d1",
      digest: DRAFT.digest,
      how: { how: "add-to", issue: 41 },
    });
  },
};

/** **Dismiss** lets the draft go, unsent, and says so as its tooltip. */
export const DismissedUnsent: Story = {
  beforeEach: () => fake([]),
  parameters: { screenshot: false },
  play: async ({ canvasElement, args }) => {
    const canvas = await theCard(canvasElement);
    const dismiss = canvas.getByRole("button", { name: "Dismiss" });
    dismiss.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.queryByText(`Report draft: ${TITLE}`)).toBeNull());
    await expect(sentTo("dismiss_report_draft")).toEqual([{ id: "d1" }]);
    await expect(sentTo("file_report_draft")).toEqual([]);
    await expect(args.onSent).not.toHaveBeenCalled();
    await expect(DISMISS_SAYS).toBe("Lets the draft go. Nothing is sent.");
  },
};

const SENT: InboxUpdate = {
  key: "report-sent:d1",
  kind: "report-sent",
  at: Math.floor(Date.parse("2026-11-08T10:05:00Z") / 1000),
  session: 4,
  chain: ["steward 4"],
  says: `Sent a report on GitHub: ${TITLE}`,
  read: false,
};

/**
 * **Sent a report, held with Undo** (#5695): a draft Always send sends says so from the start of
 * its hold, with Undo while it lasts.
 */
export const SentWithUndo: Story = {
  beforeEach: () => fake([]),
  render: ({ onGo }) => (
    <div style={{ inlineSize: 460 }}>
      <Inbox
        plane="/plane"
        asks={[]}
        onGo={() => undefined}
        onLeave={() => undefined}
        onIgnore={() => undefined}
        updates={{
          rows: [
            {
              update: { ...SENT, says: `Sent a report: ${TITLE}` },
              go: () => onGo(4),
              dismiss: () => undefined,
              answers: [
                {
                  label: "Undo",
                  name: `Undo: don't send ${TITLE}`,
                  title: "Don't send it. The draft comes back for you to read.",
                  press: () => onGo(-1),
                },
              ],
            },
          ],
          onMarkAllRead: () => undefined,
          onDismissAll: () => undefined,
        }}
      />
    </div>
  ),
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await waitFor(() => expect(canvas.getByText(`Sent a report: ${TITLE}`)).toBeVisible());
    const undo = canvas.getByRole("button", { name: `Undo: don't send ${TITLE}` });
    undo.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onGo).toHaveBeenCalledWith(-1));
  },
};

/** **The update a filed draft leaves**: on the chat it came from, with Go to chat. */
export const SentReportUpdate: Story = {
  beforeEach: () => fake([]),
  render: ({ onGo }) => (
    <div style={{ inlineSize: 460 }}>
      <Inbox
        plane="/plane"
        asks={[]}
        onGo={() => undefined}
        onLeave={() => undefined}
        onIgnore={() => undefined}
        updates={{
          rows: [{ update: SENT, go: () => onGo(4), dismiss: () => undefined }],
          onMarkAllRead: () => undefined,
          onDismissAll: () => undefined,
        }}
      />
    </div>
  ),
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await waitFor(() => expect(canvas.getByText(SENT.says, { exact: false })).toBeVisible());
    await expect(canvas.getByText("Report sent")).toBeVisible();
    const go = canvas.getAllByRole("button", { name: /^Go to chat/ })[0] as HTMLElement;
    go.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onGo).toHaveBeenCalledWith(4));
  },
};
