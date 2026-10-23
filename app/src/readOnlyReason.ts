// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext } from "react";

/**
 * **The id of the sentence a read-only project's controls are off with**, inside a group that is
 * off (`ReadOnlyLine.ReadOnlyFieldset`, #2908): a control there adds it to its description, so a
 * screen reader hears why it is off wherever it meets it. Undefined where nothing is off.
 *
 * A module of its own, which reads nothing of the core, so the set's controls can read it
 * wherever they are drawn (the theme specimen among them).
 */
export const ReadOnlyReasonId = createContext<string | undefined>(undefined);

/** {@link ReadOnlyReasonId}, read. */
export function useReadOnlyReasonId(): string | undefined {
  return useContext(ReadOnlyReasonId);
}
