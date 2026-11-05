// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { RunShown } from "./bindings";

/**
 * **What a Run grant would let happen** (ADR 0111, #5239): the program's full path, the keys and
 * the hosts, each from the app, then the chat's command, apart and said to be the chat's. The
 * command is plain text, wrapped, already spelled by the core so that no character in it changes
 * what it seems to say. Words to read, nothing to reach. A plain body for now: the ask's own part
 * family replaces it (#5080).
 */
export function RunGrantFacts({ run }: { run: RunShown }) {
  return (
    <div data-testid="run-grant">
      <p className="ui-text-meta">
        Runs <code data-testid="run-grant-program">{run.program}</code>, in this chat's own sandbox,
        with {run.persona}'s vault <code>{run.vault}</code>:
      </p>
      <ul className="ui-text-meta">
        {run.values.map((one) => (
          <li key={one.var}>
            <code>{one.key}</code> {one.in_a_file ? "in a private file, its path in" : "in"}{" "}
            <code>{one.var}</code>
          </li>
        ))}
      </ul>
      <p className="ui-text-meta" data-testid="run-grant-hosts">
        {run.every_host ? (
          "It can reach every host: this chat's sandbox holds it to none."
        ) : run.hosts.length === 0 ? (
          "It can reach no host."
        ) : (
          <>
            It can reach these hosts:{" "}
            {run.hosts.map((host, at) => (
              <span key={host}>
                {at > 0 && ", "}
                <code>{host}</code>
              </span>
            ))}
          </>
        )}
      </p>
      {run.caution != null && <p className="ui-text-meta">{run.caution}</p>}
      <p className="ui-text-meta">The chat's command, in its own words:</p>
      <pre className="whitespace-pre-wrap break-all" data-testid="run-grant-command">
        {run.command}
      </pre>
    </div>
  );
}
