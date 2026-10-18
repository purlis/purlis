// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0
import { LockOpen } from "lucide-react";

/**
 * **The mark an answer that grants leads with** (ADR 0097 §6): an open lock, before the words of
 * a `.ui-button-grants`. It is the cue that is not colour, so an Allow is told from Keep blocked
 * in every theme and by a person who does not see the accent. Hidden from a screen reader: the
 * button's name stays its words.
 */
export function GrantsMark() {
  return <LockOpen className="ui-button-grants-mark" aria-hidden="true" focusable="false" />;
}
