// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useState } from "react";
import { commands } from "../bindings";
import { FIX_STARTING_SIZE, tellFixStarted } from "../saving";
import { Button } from "../ui/button/Button";

/** The project Pool setup's tab is in: where a help chat starts (#4497). */
export const PoolSetupPlane = createContext<string | undefined>(undefined);

/**
 * **Get help** on a step that failed (#4497, spec #4469 stories 59 and 60): a chat started on
 * purlis's message about the step and `error`, the words shown where a press failed, with no
 * secret values. The chat explains, and changes nothing itself: it starts only under the
 * project's sandbox, and every fix stays here, in Pool setup.
 */
export function GetHelp({ step, error }: { step: string; error?: string }) {
  const plane = useContext(PoolSetupPlane);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  if (plane === undefined) return null;
  const press = () => {
    if (busy) return;
    setBusy(true);
    setRefused(undefined);
    void commands
      .poolSetupGetHelp(
        plane,
        step,
        error ?? null,
        FIX_STARTING_SIZE.columns,
        FIX_STARTING_SIZE.rows,
      )
      .then((answer) => {
        if (answer.status === "ok") tellFixStarted(plane, answer.data);
        else setRefused(`No chat was started: ${answer.error}`);
      })
      .catch((err: unknown) => setRefused(`No chat was started: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  return (
    <>
      <Button variant="secondary" onPress={press} isDisabled={busy}>
        Get help
      </Button>
      {refused && (
        <span className="pool-setup-p" role="alert">
          {refused}
        </span>
      )}
    </>
  );
}

/** What a step that needs attention says beside Get help. */
export const GET_HELP_SAYS =
  "Get help opens a chat that explains this step. It changes nothing itself: every fix is made here.";
