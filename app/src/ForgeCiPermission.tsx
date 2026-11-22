// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type CiPermissionAnswer, type ForgeDeviceCode } from "./bindings";
import { SettingActions } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { Dialog } from "./ui/dialog/Dialog";

type Step =
  | { at: "asking" }
  | { at: "waiting"; code: ForgeDeviceCode }
  | { at: "done"; answer: CiPermissionAnswer }
  | { at: "ended"; why: string };

/**
 * **Allow CI files** (ADR 0128 D1.1, D8.3): the second GitHub sign-in, on the person's press, that
 * asks for `workflow` and nothing a chat ever carries. GitHub refuses a push that changes a CI
 * file unless the token has it, so purlis keeps this token apart and uses it only for a push the
 * person approves. Same shape as {@link ForgeDeviceSignIn}: a code, GitHub's page, then the
 * core's own sentence for how it ended (`forge_ci_permission_wait`). Cancel stops the waiting
 * (`forge_ci_permission_cancel`) and keeps nothing.
 */
export function ForgeCiPermission({
  accountId,
  login,
  host,
  onDone,
  onCancel,
}: {
  accountId: string;
  login: string;
  host: string;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [step, setStep] = useState<Step>({ at: "asking" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let left = false;
    let flow: number | undefined;
    const stop = (which: number) =>
      void commands.forgeCiPermissionCancel(which).catch(() => undefined);
    const refused = (err: unknown) => ({ status: "error" as const, error: String(err) });
    void commands
      .forgeCiPermissionStart(accountId)
      .catch(refused)
      .then((started) => {
        if (started.status === "error") {
          if (!left) setStep({ at: "ended", why: started.error });
          return;
        }
        if (left) {
          stop(started.data.flow);
          return;
        }
        flow = started.data.flow;
        setStep({ at: "waiting", code: started.data });
        return commands
          .forgeCiPermissionWait(started.data.flow)
          .catch(refused)
          .then((ended) => {
            flow = undefined;
            if (left) return;
            if (ended.status === "ok") setStep({ at: "done", answer: ended.data });
            else setStep({ at: "ended", why: ended.error });
          });
      })
      .catch((err: unknown) => {
        if (!left) setStep({ at: "ended", why: String(err) });
      });
    return () => {
      left = true;
      if (flow !== undefined) stop(flow);
    };
  }, [accountId, attempt]);

  const done = step.at === "done";
  return (
    <Dialog
      onClose={done ? onDone : onCancel}
      title="Allow CI files"
      subtitle={`for ${login} on ${host}; kept apart from your everyday sign-in`}
      main={{
        children: done ? "Done" : step.at === "ended" ? "Try again" : "Open GitHub in your browser",
        reserve: ["Open GitHub in your browser", "Try again", "Done"],
        isDisabled: step.at === "asking",
        onPress: () => {
          if (step.at === "done") onDone();
          else if (step.at === "ended") {
            setStep({ at: "asking" });
            setAttempt((n) => n + 1);
          } else if (step.at === "waiting") void openUrl(step.code.verification_uri);
        },
      }}
    >
      {step.at === "asking" && (
        <p className="pending" role="status" aria-busy="true">
          Asking {host} for a code…
        </p>
      )}
      {step.at === "waiting" && (
        <>
          <p>
            Enter this code on GitHub&apos;s page and allow purlis to push CI files. purlis uses
            this permission only when you approve a push that changes one, and never gives it to a
            chat.
          </p>
          <Card as="p" variant="ask" className="forge-device-code">
            <strong>
              <code aria-label="The code to enter on GitHub">{step.code.user_code}</code>
            </strong>
          </Card>
          <SettingActions>
            <Button onPress={() => void navigator.clipboard?.writeText(step.code.user_code)}>
              Copy code
            </Button>
          </SettingActions>
          <p className="pending" role="status" aria-busy="true">
            Waiting for you to allow it on GitHub…
          </p>
        </>
      )}
      {step.at === "done" && <p role="status">{step.answer.says}</p>}
      {step.at === "ended" && (
        <p className="trouble said-in-full" role="alert">
          {step.why}
        </p>
      )}
    </Dialog>
  );
}
