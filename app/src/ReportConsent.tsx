// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Button } from "./ui/button/Button";
import { CodeBlock } from "./ui/code/CodeBlock";
import { Dialog } from "./ui/dialog/Dialog";
import { Choice, SettingRow } from "./settings/components";
import type { RowIds } from "./settings/components";
import { answerReports, useReportConsent, type Consent } from "./reportAnswers";

/**
 * **Asking once whether purlis may send reports** (#5688, spec #5683, the prototype's variant
 * D): the Dialog family, one setting row with a radio choice per stream, nothing chosen.
 *
 * - **Neither answer is chosen for the person** (GDPR), and accepting weighs what declining
 *   does: Not now and Save are both secondary, and Return answers neither (`byReturn: false`).
 * - **Save waits for both answers**, and its tooltip says why while it does.
 * - **Not now, or Esc, leaves both off** and is not asked again here; the crash Notice
 *   (#5700) asks once more after the next crash.
 * - **What is never sent is said in the dialog**, and "See exactly what's sent…" opens sample
 *   bytes of each kind before the person answers.
 *
 * Settings › You › Reports holds the same two choices ({@link CrashReportsChoice},
 * {@link AgentDraftsChoice}), so the answer can be changed whenever the person wants.
 */

export const CRASH_SAYS =
  "When purlis crashes, it sends where it crashed, the error, and the purlis and OS versions to purlis's crash service in the EU.";
export const DRAFTS_SAYS =
  "When an agent finds purlis misbehaving, it writes a report draft for you to read. Nothing is sent until you file it.";
export const NEVER_SENT =
  "Never sent: your code, your chats, file paths, project or workspace names, or secrets.";

/** What a crash report holds, as it would be sent: a sample, before the first one is. */
export const CRASH_SAMPLE = `{
  "where": "purlis_core::plane::read_layout",
  "error": "called \`Option::unwrap()\` on a \`None\` value",
  "purlis": "0.41.2",
  "channel": "stable",
  "os": "macOS 27.2 (arm64)"
}`;

/** What a report draft holds, as an agent writes it: a sample. */
export const DRAFT_SAMPLE = `Title: purlis worktree update refuses a clean branch
Command: purlis worktree update
Exit: 1
purlis 0.41.2 (stable) · macOS 27.2 (arm64) · Claude Code 3.4.0 · a built-in persona

What happened: the update said the branch had uncommitted work, but git status was clean.
What was expected: the branch brought up to its base.`;

/** An answer as a radio choice holds it: nothing chosen while it is unanswered. */
const chosen = (said: boolean | null, yes: string, no: string): string | undefined =>
  said === null ? undefined : said ? yes : no;

/** Crash reports: Send or Don't send, nothing chosen until the person chooses. */
export function CrashReportsChoice({
  ids,
  value,
  onChange,
}: {
  ids: RowIds;
  value: boolean | null;
  onChange: (send: boolean) => void;
}) {
  return (
    <Choice
      kind="radio"
      ids={ids}
      options={[
        { value: "send", label: "Send" },
        { value: "dont", label: "Don't send" },
      ]}
      value={chosen(value, "send", "dont")}
      onValueChange={(to) => onChange(to === "send")}
    />
  );
}

/** Agent drafts: Allow or Don't allow, nothing chosen until the person chooses. */
export function AgentDraftsChoice({
  ids,
  value,
  onChange,
}: {
  ids: RowIds;
  value: boolean | null;
  onChange: (allow: boolean) => void;
}) {
  return (
    <Choice
      kind="radio"
      ids={ids}
      options={[
        { value: "allow", label: "Allow" },
        { value: "dont", label: "Don't allow" },
      ]}
      value={chosen(value, "allow", "dont")}
      onValueChange={(to) => onChange(to === "allow")}
    />
  );
}

export const ALWAYS_SEND_SAYS =
  "Drafts go straight to purlis's private report service without you reading them, at most five a day. Each waits a few seconds with Undo first. Never to GitHub.";

/** Always send: Send without asking or Ask first (#5695), nothing chosen until the person chooses. */
export function AlwaysSendChoice({
  ids,
  value,
  onChange,
}: {
  ids: RowIds;
  value: boolean | null;
  onChange: (always: boolean) => void;
}) {
  return (
    <Choice
      kind="radio"
      ids={ids}
      options={[
        { value: "always", label: "Send without asking" },
        { value: "ask", label: "Ask me first" },
      ]}
      value={chosen(value, "always", "ask")}
      onValueChange={(to) => onChange(to === "always")}
    />
  );
}

/**
 * **Always send, asked once** (#5695, spec #5683 story 38): pressed on a draft while the person
 * has not said yes to it, this says where drafts will go and where to see and stop them. Return
 * answers neither: the yes is pressed.
 */
export function AlwaysSendDialog({ onYes, onCancel }: { onYes: () => void; onCancel: () => void }) {
  return (
    <Dialog
      title="Always send report drafts?"
      description="From now on, drafts agents write go straight to purlis's private report service, unread. Each waits a few seconds with Undo first, and at most five go a day. They never go to GitHub."
      onClose={onCancel}
      initialFocus="dialog"
      cancel={{ children: "Cancel", onPress: onCancel }}
      main={{ children: "Always send", variant: "secondary", byReturn: false, onPress: onYes }}
    >
      <p>
        Settings › You › Reports lists every report sent, byte for byte. Turn Always send off there
        at any time.
      </p>
    </Dialog>
  );
}

/** "See exactly what's sent…": opens a sample of each kind of report, as its bytes. */
export function WhatReportsSend({ ids }: { ids: RowIds }) {
  const [open, setOpen] = useState(false);
  return (
    <div id={ids.id} role="group" aria-labelledby={ids.labelledBy}>
      <Button onPress={() => setOpen(true)}>See exactly what's sent…</Button>
      {open && (
        <Dialog
          title="What a report sends"
          description={NEVER_SENT}
          size="wide"
          onClose={() => setOpen(false)}
          cancel={{ children: "Close" }}
        >
          <CodeBlock label="A crash report" text={CRASH_SAMPLE} />
          <CodeBlock label="A report draft, as an agent writes it" text={DRAFT_SAMPLE} />
        </Dialog>
      )}
    </div>
  );
}

/** The first-run question, drawn: what it answers is the caller's to keep. */
export function ReportConsentDialog({
  onSave,
  onNotNow,
}: {
  /** Both answers, once both are chosen and Save is pressed. */
  onSave: (answers: Pick<Consent, "crash_reports" | "agent_drafts">) => void;
  /** Not now, or Esc: neither answered. */
  onNotNow: () => void;
}) {
  const [crash, setCrash] = useState<boolean | null>(null);
  const [drafts, setDrafts] = useState<boolean | null>(null);
  const both = crash !== null && drafts !== null;
  return (
    <Dialog
      title="Help fix purlis"
      description="Choose for each kind of report. You can change it later in Settings › You › Reports."
      onClose={onNotNow}
      initialFocus="dialog"
      cancel={{ children: "Not now", onPress: onNotNow }}
      main={{
        children: "Save",
        variant: "secondary",
        byReturn: false,
        isDisabled: !both,
        tooltip: both ? undefined : "Choose for both kinds of report first.",
        onPress: () => {
          if (both) onSave({ crash_reports: crash, agent_drafts: drafts });
        },
      }}
    >
      <SettingRow
        label="Crash reports"
        help={CRASH_SAYS}
        grouped
        control={(ids) => <CrashReportsChoice ids={ids} value={crash} onChange={setCrash} />}
      />
      <SettingRow
        label="Agent drafts"
        help={DRAFTS_SAYS}
        grouped
        control={(ids) => <AgentDraftsChoice ids={ids} value={drafts} onChange={setDrafts} />}
      />
      <SettingRow
        label="What is sent"
        help={NEVER_SENT}
        control={(ids) => <WhatReportsSend ids={ids} />}
      />
    </Dialog>
  );
}

/**
 * The first-run question, asked while this machine has not answered it: on a fresh machine,
 * and once on the first launch after the update that brought it, since no older purlis kept an
 * answer. `quiet` holds it back while another question has the window.
 */
export function ReportConsentAsk({ quiet = false }: { quiet?: boolean }) {
  const { consent, known } = useReportConsent();
  if (!known || consent.asked || quiet) return null;
  return (
    <ReportConsentDialog
      onSave={(answers) => void answerReports({ ...answers, asked: true })}
      onNotNow={() => void answerReports({ asked: true })}
    />
  );
}
