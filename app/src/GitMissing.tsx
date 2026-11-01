// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { LoaderCircle } from "lucide-react";
import type { GitInstall } from "./bindings";
import type { GitGate } from "./gitPresence";
import { Button } from "./ui/button/Button";

/**
 * **Git is not installed** (#1813): the guided screen a {@link GitGate} opens when an action
 * needs git and this machine has none. purlis's own screen, instead of the system's surprise
 * dialog that running Apple's stub `/usr/bin/git` would open.
 *
 * - **macOS:** Install opens Apple's installer for the command line developer tools, which
 *   bring git. purlis then watches for it, and the action that waited carries on by itself.
 * - **Linux:** the distribution's own package manager line, for the person to run (it needs
 *   `sudo`, so purlis never runs it).
 * - **Elsewhere:** where git is downloaded from.
 *
 * Check again looks at once, everywhere; Not now drops the waiting action. Draws nothing while
 * the gate is closed, so a surface can always mount it.
 */
export function GitMissing({ gate }: { gate: GitGate }) {
  if (!gate.open) return null;
  const { install } = gate;
  return (
    <section className="git-missing" aria-labelledby="git-missing-title" data-testid="git-missing">
      <h3 id="git-missing-title">
        {install === null ? "purlis could not look for git" : "Git is not installed"}
      </h3>
      {install !== null && <Explained install={install} installing={gate.installing} />}

      {/* Verbatim: the core's sentence. */}
      {gate.trouble && (
        <p className="trouble said-in-full" role="alert">
          {gate.trouble}
        </p>
      )}

      <div className="doing">
        {install?.kind === "developer-tools" && !gate.installing && (
          // `tabIndex={0}` on every button, per `docs/ui-primitives.md` (#189).
          <Button variant="primary" onPress={() => void gate.startInstall()}>
            Install
          </Button>
        )}
        <Button isDisabled={gate.checking} onPress={() => void gate.checkAgain()}>
          Check again
        </Button>
        <Button onPress={gate.cancel}>Not now</Button>
      </div>
      {gate.checking && (
        <p className="pending" aria-busy="true" role="status">
          <LoaderCircle className="node-icon spinning" />
          Looking for git…
        </p>
      )}
    </section>
  );
}

/** What this machine does to get git, one paragraph for the why and one for the way. */
function Explained({ install, installing }: { install: GitInstall; installing: boolean }) {
  switch (install.kind) {
    case "developer-tools":
      return (
        <>
          <p className="came-back">
            purlis works with git, which comes with Apple&apos;s command line developer tools.
          </p>
          <p className="came-back" role="status">
            {installing
              ? "Apple's installer is open. Finish it there, and purlis carries on by itself."
              : "Install opens Apple's installer. When it has finished, purlis carries on where you were."}
          </p>
        </>
      );
    case "package":
      return (
        <>
          <p className="came-back">purlis works with git. Install it with your package manager:</p>
          <code className="harness-installer">{install.command}</code>
          <p className="came-back">Then press Check again, and purlis carries on where you were.</p>
        </>
      );
    case "download":
      return (
        <>
          <p className="came-back">
            purlis works with git. Get it from <code>{install.page}</code>.
          </p>
          <p className="came-back">Then press Check again, and purlis carries on where you were.</p>
        </>
      );
  }
}
