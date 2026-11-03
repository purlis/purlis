// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Ref } from "react";
import type { SecretShown } from "./bindings";
import { TextField } from "./ui/field/Field";

/**
 * **What a Secret ask fills, and the box its value is typed in** (#5068, ADR 0109): the key, the
 * vault and the persona by name, and a masked box that holds its own words, so the value is
 * never in React's state or the page's markup. Fill reads it from `box`; the chat hears filled
 * or declined, and never the value. A plain field for now: the ask's own part family replaces it
 * once its prototype is ruled on (#5080).
 */
export function SecretFill({ secret, box }: { secret: SecretShown; box: Ref<HTMLInputElement> }) {
  return (
    <div data-testid="secret-fill">
      <p className="ui-text-meta">
        Fills <code>{secret.key}</code> in {secret.persona}'s vault <code>{secret.vault}</code>
        {secret.tool != null && <> for {secret.tool}</>}, on this machine. The chat hears filled or
        declined, never the value.
      </p>
      <TextField
        inputRef={box}
        type="password"
        aria-label={`Value of ${secret.key}`}
        placeholder="Type or paste the value"
      />
    </div>
  );
}
