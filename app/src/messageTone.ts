// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { UpdateKind } from "./bindings";
import { kindOfUpdate, type RowKind } from "./InboxRow";
import type { MessageTone } from "./ui/message/Message";
import type { NoticeTone } from "./ui/notice/Notice";

/**
 * **What a line is, as its message row's state** (#5649, ADR 0119 decision 6): the one table an
 * ask, a line of Mine or Project and a Notice all take their row's tone from. What waits on the
 * person is `waiting`; trouble, or what came to nothing, `failed`; a task that finished `done`;
 * what came back (a chat resumed, a handoff arrived) a `report`; any other Notice `plain`.
 */
const TONES = {
  waits: "waiting",
  trouble: "failed",
  done: "done",
  report: "report",
  note: "plain",
} as const satisfies Record<string, MessageTone>;

/** The updates that are something coming back, rather than something done. */
const REPORTS: ReadonlySet<UpdateKind> = new Set<UpdateKind>(["resumed", "arrived"]);

/** A row's kind, as its state: trouble `failed`, any other update `done`, the rest `waiting`. */
export const toneOfKind = (kind: RowKind): MessageTone =>
  kind === "trouble" ? TONES.trouble : kind === "update" ? TONES.done : TONES.waits;

/** An update's state, by its kind. */
export const toneOfUpdate = (kind: UpdateKind): MessageTone =>
  REPORTS.has(kind) ? TONES.report : toneOfKind(kindOfUpdate(kind));

/** A Notice's state: `waiting` where it waits on the person, else by its tone. */
export const toneOfNotice = (tone: NoticeTone, waits: boolean): MessageTone =>
  waits || tone === "attention" ? TONES.waits : tone === "trouble" ? TONES.trouble : TONES.note;
