// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { commands, type ReportConsent } from "./bindings";

/**
 * **The person's answers about reports, on this machine** (#5688, spec #5683): whether purlis
 * sends its **Crash reports**, and whether agents may write **Report drafts**. Kept by the core
 * (`<config>/reports.json`, `purlis_core::report::Consent`), which refuses every draft and send
 * of a stream without its yes; this window only reads and writes the answers.
 *
 * Nothing is answered for the person (GDPR): `null` is unanswered, which the core reads as no.
 * `asked` is whether the first-run question was answered, by Save or by Not now, so it is asked
 * once.
 */
export type Consent = ReportConsent;

/** A machine that kept no answer: nothing chosen, and not yet asked. */
export const UNANSWERED: Consent = {
  crash_reports: null,
  agent_drafts: null,
  always_send: null,
  asked: false,
  crash_notice: null,
};

/** What the store holds: the answers once the core has said them, and why a keep failed. */
export type Held = {
  consent: Consent;
  /** Whether the core has said what this machine keeps. Until it has, nothing is asked: the
   *  answer decides that. */
  known: boolean;
  /** Why the last answer could not be kept, in the core's words. */
  trouble: string | undefined;
};

const FRESH: Held = { consent: UNANSWERED, known: false, trouble: undefined };

let held: Held = FRESH;
let reading: Promise<void> | undefined;
const listeners = new Set<() => void>();

function become(next: Held): void {
  held = next;
  for (const listener of listeners) listener();
}

const isAnswer = (said: unknown): boolean => said === null || typeof said === "boolean";

/** Whether `said` has the shape the core answers with. */
function isConsent(said: unknown): said is Consent {
  if (said === null || typeof said !== "object") return false;
  const { crash_reports, agent_drafts, always_send, asked, crash_notice } = said as Record<
    string,
    unknown
  >;
  return (
    isAnswer(crash_reports) &&
    isAnswer(agent_drafts) &&
    isAnswer(always_send) &&
    typeof asked === "boolean" &&
    (crash_notice === null || crash_notice === "due" || crash_notice === "settled")
  );
}

/** Reads the kept answers from the core, once per launch. */
function read(): Promise<void> {
  reading ??= commands.reportConsent().then(
    (said) =>
      become(
        said.status === "ok"
          ? // Only an answer of the core's own shape is known: from anything else, as from no
            // answer, no question is asked.
            isConsent(said.data)
            ? { ...held, consent: said.data, known: true }
            : held
          : { ...held, trouble: said.error },
      ),
    () => become({ ...held }),
  );
  return reading;
}

/** The answers, read from the core the first time they are asked for. */
export function useReportConsent(): Held {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      void read();
      return () => listeners.delete(listener);
    },
    () => held,
  );
}

/** Changes the answers and keeps them on this machine. Settles once the core has kept them. */
export async function answerReports(change: Partial<Consent>): Promise<void> {
  const consent = { ...held.consent, ...change };
  become({ ...held, consent, trouble: undefined });
  // Every control leaves this call (`void`), so a call that fails is said as trouble, not thrown.
  const kept = await commands
    .keepReportConsent(consent)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (kept.status === "error") become({ ...held, trouble: kept.error });
}

/** Forgets what this launch read and chose, as a new launch would. For tests and stories. */
export function forgetReportConsent(to: Held = FRESH): void {
  reading = to.known ? Promise.resolve() : undefined;
  become(to);
}
