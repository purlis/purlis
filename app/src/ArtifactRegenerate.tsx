// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  commands,
  type ArtifactRegenerate as About,
  type ArtifactRegenerated,
  type PlaneId,
} from "./bindings";
import { Button } from "./ui/button/Button";
import { AlertDialog } from "./ui/dialog/Dialog";

/**
 * **Regenerate, in an artifact's bar** (ADR 0120 §6, #5962): the pin's command run again in
 * purlis's session host, writing this person's data files.
 *
 * - **Drawn only for a pin that names a command.** Where it cannot run here (no script it can
 *   approve, a platform with no sandbox) the bar says why and the button is off.
 * - **The first press asks**, and so does every press after the code it runs changed: a prompt
 *   naming the folder of code a yes approves. The yes is of the fingerprint shown with it; purlis
 *   checks it again before it records it and again before it runs.
 * - **A run says how it went**: what it wrote, or what went wrong with the date the data shown was
 *   last good, and the tail of its output with secrets masked.
 */
export function ArtifactRegenerate({
  plane,
  workspace,
  folder,
  name,
}: {
  plane: PlaneId;
  workspace: string;
  folder: string;
  name: string;
}) {
  const [about, setAbout] = useState<About | null>(null);
  const [asked, setAsked] = useState(0);
  const [busy, setBusy] = useState(false);
  const [asking, setAsking] = useState(false);
  const [ran, setRan] = useState<ArtifactRegenerated | null>(null);
  const [refused, setRefused] = useState<string | null>(null);

  useEffect(() => {
    let gone = false;
    commands.artifactRegenerateAbout(plane, workspace, folder).then(
      (answer) => {
        if (gone) return;
        setAbout(answer.status === "ok" ? answer.data : null);
      },
      () => {},
    );
    return () => {
      gone = true;
    };
  }, [plane, workspace, folder, asked]);

  if (about === null || !about.has_command) return null;

  const run = () => {
    setBusy(true);
    setRan(null);
    setRefused(null);
    commands.artifactRegenerate(plane, workspace, folder).then(
      (answer) => {
        setBusy(false);
        if (answer.status === "ok") setRan(answer.data);
        else setRefused(answer.error);
        setAsked((n) => n + 1);
      },
      (why: unknown) => {
        setBusy(false);
        setRefused(String(why));
      },
    );
  };

  const approveAndRun = () => {
    setAsking(false);
    const shown = about.fingerprint;
    if (shown === null) return;
    setBusy(true);
    setRefused(null);
    commands.artifactRegenerateApprove(plane, workspace, folder, shown).then(
      (answer) => {
        if (answer.status === "ok") {
          run();
        } else {
          setBusy(false);
          setRefused(answer.error);
          setAsked((n) => n + 1);
        }
      },
      (why: unknown) => {
        setBusy(false);
        setRefused(String(why));
      },
    );
  };

  const press = () => (about.standing === "approved" ? run() : setAsking(true));

  return (
    <>
      <span className="artifact-bar-standing" role="status" data-testid="artifact-regenerate">
        {busy ? "Regenerating" : said(about, ran)}
      </span>
      <Button
        size="sm"
        isDisabled={busy || about.refused !== null}
        onPress={press}
        data-testid="artifact-regenerate-button"
      >
        Regenerate
      </Button>
      {refused !== null || (ran?.failed ?? null) !== null ? (
        <div className="artifact-regenerate-said">
          <p role="alert" data-testid="artifact-regenerate-failed">
            {refused ?? ran?.failed}
          </p>
          {ran?.output ? (
            <pre className="artifact-regenerate-output" data-testid="artifact-regenerate-output">
              {ran.output}
            </pre>
          ) : null}
        </div>
      ) : null}
      <AlertDialog
        isOpen={asking}
        onClose={() => setAsking(false)}
        title={`Run the code that regenerates ${name}?`}
        titleId="artifact-regenerate-ask"
        initialFocus="cancel"
        description={
          <>
            Regenerate runs the code in <code>{about.code}</code> on this machine, in a sandbox with
            no network that writes only this artifact&apos;s data files.{" "}
            {asksBecause(about.standing)} purlis asks again whenever that code changes.
          </>
        }
        main={{ children: "Approve and run", variant: "grants", onPress: approveAndRun }}
        data-testid="artifact-regenerate-ask"
      />
    </>
  );
}

/** Why the prompt asks, by where the command stands. */
function asksBecause(standing: string | null): string {
  switch (standing) {
    case "changed":
      return "The code changed since you approved it.";
    case "pin-changed":
      return "This pin's command changed since you approved it.";
    default:
      return "You have not approved it on this machine.";
  }
}

/** The bar's line for Regenerate: what the last run did, or when the data was last good. */
function said(about: About, ran: ArtifactRegenerated | null): string {
  if (ran !== null && ran.failed === null) return `Regenerated ${ran.written.join(", ")}`;
  if (about.refused !== null) return `Cannot regenerate: ${about.refused}`;
  if (about.last_good !== null) return `Data from ${about.last_good}`;
  return "";
}
