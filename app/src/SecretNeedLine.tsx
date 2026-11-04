// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { TextField } from "./ui/field/Field";

/** The most a Secret need's purpose keeps, as the core cuts it (`MOST_PURPOSE_CHARS`). */
const MOST_PURPOSE_CHARS = 200;

/**
 * **The line remembering a Secret need writes, with its purpose to edit** (#5240, spec #5235): the
 * file and each line exactly as they go in, so what reaches git is what the person read. One
 * copy, for a Secret ask's Remember and a vault row's *Offer as a Secret need*. A plain body for
 * now: the ask's own part family replaces it (#5080).
 */
export function SecretNeedLine({
  secretKey,
  file,
  need,
  vault,
  purpose,
  onPurpose,
}: {
  secretKey: string;
  file: string;
  /** A `secret-need.<key>` line goes in. */
  need: boolean;
  /** The vault a `vault:` line names, where the persona names none. */
  vault?: string | null;
  purpose: string;
  onPurpose: (to: string) => void;
}) {
  return (
    <div className="secret-need-line" data-testid="secret-need-line">
      <p className="ui-text-meta">
        Adds to <code>{file}</code>. It rides your next save, and teammates read every word.
      </p>
      {need && (
        <div className="secret-need-line-row">
          <code>secret-need.{secretKey}:</code>
          <TextField
            aria-label={`What ${secretKey} is for`}
            value={purpose}
            maxLength={MOST_PURPOSE_CHARS}
            placeholder="Say what it is for"
            onChange={onPurpose}
          />
        </div>
      )}
      {vault != null && (
        <>
          <code>vault: {vault}</code>
          <p className="ui-text-meta">
            This sets the persona's vault. A <code>vault: none</code> in that file is replaced.
          </p>
        </>
      )}
    </div>
  );
}
