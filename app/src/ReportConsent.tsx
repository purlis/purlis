// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type ReportSamples } from "./bindings";
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
  disabled = false,
  onChange,
}: {
  ids: RowIds;
  value: boolean | null;
  /** Held by another level (#5696): shown, and out of reach. */
  disabled?: boolean;
  onChange: (always: boolean) => void;
}) {
  return (
    <Choice
      kind="radio"
      ids={ids}
      disabled={disabled}
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

/**
 * "See exactly what's sent…": opens a sample of each kind of report, as its bytes. The core makes
 * them with the envelope builders a real report goes through (`report_samples`), so the sample
 * can never say less than what is sent.
 */
export function WhatReportsSend({ ids }: { ids: RowIds }) {
  const [open, setOpen] = useState(false);
  const [samples, setSamples] = useState<ReportSamples | { error: string }>();
  const show = () => {
    setOpen(true);
    void commands
      .reportSamples()
      .then((made) => setSamples(made.status === "ok" ? made.data : { error: made.error }))
      .catch((err: unknown) => setSamples({ error: String(err) }));
  };
  return (
    <div id={ids.id} role="group" aria-labelledby={ids.labelledBy}>
      <Button onPress={show}>See exactly what's sent…</Button>
      {open && (
        <Dialog
          title="What a report sends"
          description={NEVER_SENT}
          size="wide"
          onClose={() => setOpen(false)}
          cancel={{ children: "Close" }}
        >
          {samples !== undefined && "error" in samples && (
            <p>The samples could not be made: {samples.error}</p>
          )}
          {samples !== undefined && !("error" in samples) && (
            <>
              <CodeBlock label="A crash report" text={samples.crash} />
              <CodeBlock label="A report draft an agent wrote, sent unread" text={samples.draft} />
            </>
          )}
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
