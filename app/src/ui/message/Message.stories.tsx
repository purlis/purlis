// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The message family** (#5649, spec #5638): the Inbox's views as the variants prototype's
 * variant A draws them, borderless rows in a 320 px side view on the side panel's own ground.
 * Each state's mark (waiting, done, failed, a report, plain), a read line, a row with its × over
 * its time, its links, a reply answered in the row behind its Answer link, closed and open
 * (#5651), a result read in place, and a consent's waiting edge. Walked by the
 * keyboard, held to axe and compared with its baseline in Chromium and WebKit
 * (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type ReactNode } from "react";
import {
  CircleAlert,
  CircleCheck,
  Info,
  Mail,
  MessageCircleQuestion,
  Send,
  ShieldQuestion,
  TriangleAlert,
  X,
} from "lucide-react";
import { expect, userEvent, within } from "storybook/test";
import { expectFocusInSight } from "../../stories.testkit";
import { Button, ButtonRow, IconButton } from "../button/Button";
import { TextArea } from "../field/Field";
import { Panel } from "../panel/Panel";
import {
  MessageBody,
  MessageForm,
  MessageLinks,
  MessageMeta,
  MessageRow,
  MessageTop,
  type MessageTone,
} from "./Message";

/** A side view of the right bar, as the window draws Asks, Updates and Notices. */
function View({ children, said }: { children: ReactNode; said?: string }) {
  return (
    <Panel variant="side">
      <ul style={{ inlineSize: "320px", margin: 0, padding: 0 }}>{children}</ul>
      {said !== undefined && <p role="status">{said}</p>}
    </Panel>
  );
}

/** One row with a title, its time, its meta, its words and its links, in `tone`. */
function Row({
  tone,
  icon,
  title,
  time,
  meta,
  words,
  links,
  form,
  formId,
  read,
  consent,
  onDismiss,
}: {
  tone: MessageTone;
  icon: ReactNode;
  title: string;
  time?: string;
  meta: string;
  words?: string;
  links?: ReactNode;
  form?: ReactNode;
  /** The form's id, which the link that opens it names. */
  formId?: string;
  read?: boolean;
  consent?: boolean;
  onDismiss?: () => void;
}) {
  return (
    <MessageRow
      tone={tone}
      icon={icon}
      read={read}
      consent={consent}
      aria-label={title}
      dismiss={
        onDismiss === undefined ? undefined : (
          <IconButton
            icon={<X />}
            size="sm"
            variant="quiet"
            aria-label={`Dismiss ${title}`}
            onPress={onDismiss}
          />
        )
      }
    >
      <MessageTop title={title} time={time === undefined ? undefined : <time>{time}</time>} />
      <MessageMeta>{meta}</MessageMeta>
      {words !== undefined && <MessageBody>{words}</MessageBody>}
      {links !== undefined && <MessageLinks>{links}</MessageLinks>}
      {form !== undefined && <MessageForm id={formId}>{form}</MessageForm>}
    </MessageRow>
  );
}

const meta = {
  component: MessageRow,
  args: { tone: "plain", icon: <Info />, children: null },
} satisfies Meta<typeof MessageRow>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Every state a row's mark says**, one under another with the hairline between them. */
export const EveryState: Story = {
  render: () => (
    <View>
      <Row
        tone="waiting"
        icon={<ShieldQuestion />}
        title="auth-fix"
        time="2m"
        meta="Asks your permission · web"
        words="Run cargo test --workspace"
      />
      <Row
        tone="done"
        icon={<CircleCheck />}
        title="scale rebuild #5511"
        time="02:05"
        meta="Task finished · new"
        words="All 1,204 tests passed on macOS and Linux."
      />
      <Row
        tone="failed"
        icon={<TriangleAlert />}
        title="#5497 scale run"
        time="01:50"
        meta="Task failed · new"
        words="cargo build failed: 3 errors in crates/scale."
      />
      <Row
        tone="report"
        icon={<Mail />}
        title="Report from reviewer on #5593"
        time="01:42"
        meta="Came back"
        words="Approve with one change."
      />
      <Row
        tone="plain"
        icon={<Info />}
        title="2 browsers were open at quit"
        meta="Note"
        words="They were closed. Their sign-ins are kept."
      />
      <Row
        tone="done"
        icon={<CircleCheck />}
        title="#5374 review"
        time="01:55"
        meta="Task finished"
        words="Review done: 2 must-fix, 3 nits."
        read
      />
    </View>
  ),
  play: async ({ canvasElement }) => {
    const rows = canvasElement.querySelectorAll('[data-scope="message"][data-part="root"]');
    await expect(rows).toHaveLength(6);
    for (const row of rows)
      await expect(row.querySelector('[data-part="mark"] svg')).not.toBeNull();
  },
};

/** An ask whose links and × say what was pressed. */
function AskWithLinks() {
  const [said, setSaid] = useState("Nothing pressed");
  return (
    <View said={said}>
      <Row
        tone="waiting"
        icon={<MessageCircleQuestion />}
        title="Issues cleanup"
        time="7h"
        meta="Waiting on your reply · smart-ide"
        words="Which of the three issues should I close first?"
        links={
          <>
            <Button size="sm" variant="quiet-link" data-lead="" onPress={() => setSaid("Answer")}>
              Answer
            </Button>
            <Button size="sm" variant="quiet-link" onPress={() => setSaid("Go to chat")}>
              Go to chat
            </Button>
          </>
        }
        onDismiss={() => setSaid("Dismissed")}
      />
    </View>
  );
}

/**
 * **A row's links and its ×** (the keyboard's way through): Answer leads in the accent's words,
 * Go to chat follows, and the × takes the time's place as the row holds the keyboard.
 */
export const LinksAndDismiss: Story = {
  render: () => <AskWithLinks />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Answer" })).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Answer");
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Go to chat" })).toHaveFocus();
    await expectFocusInSight();
    await userEvent.tab();
    const dismiss = canvas.getByRole("button", { name: "Dismiss Issues cleanup" });
    await expect(dismiss).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Dismissed");
  },
};

/** **A consent, answered in the row** (ADR 0097 §6): Allow grants, Keep blocked as plain beside
 *  it, and the row's waiting edge among the notes around it. */
export const ConsentInTheRow: Story = {
  render: () => (
    <View>
      <Row
        tone="failed"
        icon={<CircleAlert />}
        title="Runner key changed"
        time="now"
        meta="Problem"
        words="qa's runner no longer matches the key purlis paired with."
        links={
          <Button size="sm" variant="quiet-link" data-lead="">
            Pair again
          </Button>
        }
      />
      <Row
        tone="waiting"
        consent
        icon={<ShieldQuestion />}
        title="steward 4"
        time="now"
        meta="Waiting on you"
        words="The sandbox blocked a host: api.example.com"
        form={
          <ButtonRow size="sm">
            <Button variant="grants">Allow</Button>
            <Button variant="secondary">Keep blocked</Button>
          </ButtonRow>
        }
      />
    </View>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Pair again" })).toHaveFocus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Allow" })).toHaveFocus();
    await expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Keep blocked" })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** **An ask answered in the row, behind its Answer link** (#5651): Answer opens its reply box
 *  with the keyboard in it, and reads Cancel while it is open. */
function AskAnsweredHere({ opened = false }: { opened?: boolean }) {
  const [open, setOpen] = useState(opened);
  const [said, setSaid] = useState("Nothing sent");
  const [text, setText] = useState("");
  return (
    <View said={said}>
      <Row
        tone="waiting"
        icon={<MessageCircleQuestion />}
        title="Issues cleanup"
        time="7h"
        meta="Waiting on your reply · smart-ide"
        words="Which of the three issues should I close first?"
        links={
          <>
            <Button
              size="sm"
              variant="quiet-link"
              data-lead=""
              aria-label={open ? "Cancel answering Issues cleanup" : "Answer Issues cleanup"}
              aria-expanded={open}
              aria-controls={open ? "answer-form" : undefined}
              onPress={() => setOpen(!open)}
            >
              {open ? "Cancel" : "Answer"}
            </Button>
            <Button size="sm" variant="quiet-link">
              Go to chat
            </Button>
          </>
        }
        formId="answer-form"
        form={
          open ? (
            <TextArea
              strip
              autoFocus={!opened}
              aria-label="Reply to Issues cleanup"
              value={text}
              onChange={setText}
              placeholder="Reply to Issues cleanup…"
              hint="Enter sends · Shift+Enter new line"
              onKeyDown={(event) => {
                if (event.key !== "Enter" || event.shiftKey) return;
                event.preventDefault();
                setSaid("Sent");
              }}
              end={
                <IconButton
                  icon={<Send />}
                  size="sm"
                  variant="primary"
                  aria-label="Send reply to Issues cleanup"
                  onPress={() => setSaid("Sent")}
                />
              }
            />
          ) : undefined
        }
      />
    </View>
  );
}

/** **Answer, closed** (#5651): the ask's words and its links, no form until Answer is pressed. */
export const AnswerClosed: Story = {
  render: () => <AskAnsweredHere />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    const answer = canvas.getByRole("button", { name: "Answer Issues cleanup" });
    await expect(answer).toHaveFocus();
    await expect(answer).toHaveAttribute("aria-expanded", "false");
    await expect(canvas.queryByRole("textbox")).toBeNull();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("textbox", { name: "Reply to Issues cleanup" })).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("main{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Sent");
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    const cancel = canvas.getByRole("button", { name: "Cancel answering Issues cleanup" });
    await expect(cancel).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.queryByRole("textbox")).toBeNull();
  },
};

/** **Answer, open** (#5651): the reply box in the row under the links, the link reading Cancel. */
export const AnswerOpen: Story = {
  render: () => <AskAnsweredHere opened />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    const cancel = canvas.getByRole("button", { name: "Cancel answering Issues cleanup" });
    await expect(cancel).toHaveFocus();
    await expect(cancel).toHaveAttribute("aria-expanded", "true");
    await userEvent.tab();
    await userEvent.tab();
    await expect(canvas.getByRole("textbox", { name: "Reply to Issues cleanup" })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** A line of Mine whose report opens in place, its title stepping back once it is read. */
function ResultReadHere() {
  const [open, setOpen] = useState(false);
  const [read, setRead] = useState(false);
  return (
    <View>
      <MessageRow tone="failed" icon={<TriangleAlert />} read={read} aria-label="#5497 scale run">
        <MessageTop title="#5497 scale run" time={<time>01:50</time>} />
        <MessageMeta>{read ? "Task failed" : "Task failed · new"}</MessageMeta>
        {open && (
          <MessageBody id="result-report">
            cargo build failed: 3 errors in crates/scale. The first is in src/plan.rs: a borrowed
            value does not live long enough.
          </MessageBody>
        )}
        <MessageLinks>
          <Button
            size="sm"
            variant="quiet-link"
            data-lead=""
            aria-label={`${open ? "Hide" : "Read"} #5497 scale run`}
            aria-expanded={open}
            aria-controls={open ? "result-report" : undefined}
            onPress={() => {
              setRead(true);
              setOpen(!open);
            }}
          >
            {open ? "Hide" : "Read"}
          </Button>
          <Button size="sm" variant="quiet-link">
            Go to chat
          </Button>
        </MessageLinks>
      </MessageRow>
    </View>
  );
}

/** **A result read in place** (#5651, ADR 0117 decision 8): Read opens its report or why it
 *  failed, marks it read, and reads Hide while it is open. */
export const ResultReadInPlace: Story = {
  render: () => <ResultReadHere />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.tab();
    const read = canvas.getByRole("button", { name: "Read #5497 scale run" });
    await expect(read).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await expect(read).toHaveAttribute("aria-expanded", "true");
    await expect(canvas.getByText(/a borrowed value/)).toBeVisible();
    await expectFocusInSight();
  },
};
