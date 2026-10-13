// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { ForgeAccountRow } from "./bindings";
import { ForgeDeviceSignIn } from "./ForgeDeviceSignIn";

/** The forges with a browser sign-in, by the word the core names each. */
type Through = "github";

/**
 * **The browser sign-ins, side by side** (#1810 Q3; ADR 0077 §1): one button per forge purlis
 * has a registration on, each opening that forge's own sign-in. One component, drawn by the
 * welcome's "Sign in" page and by Settings › You › Accounts, beside "Sign in with a token…".
 *
 * - **Sign in with GitHub** is the device flow on github.com ({@link ForgeDeviceSignIn}, #729).
 * - A forge on a host of your own (a GitHub Enterprise server, a self-managed GitLab) has no
 *   browser sign-in: it takes a token, which the page says beside these buttons.
 */
export function ForgeBrowserSignIn({
  onSignedIn,
}: {
  /** The account a browser sign-in made: the list it belongs to is read again. */
  onSignedIn: (account: ForgeAccountRow) => void;
}) {
  const [through, setThrough] = useState<Through>();

  return (
    <>
      <div className="ui-button-row">
        <button
          type="button"
          className="ui-button ui-button-primary"
          // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
          tabIndex={0}
          onClick={() => setThrough("github")}
        >
          Sign in with GitHub
        </button>
        {/* Sign in with GitLab (#730) goes here: its button, and its dialog below. */}
      </div>
      {through === "github" && (
        <ForgeDeviceSignIn
          onCancel={() => setThrough(undefined)}
          onSignedIn={(account) => {
            setThrough(undefined);
            onSignedIn(account);
          }}
        />
      )}
    </>
  );
}
