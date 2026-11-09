// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  commands,
  type ArtifactCompared,
  type ArtifactHistory as History,
  type ArtifactOpened,
  type ArtifactOpenedAsOf,
  type ArtifactPlace,
  type ArtifactTheme,
  type ArtifactVersion,
  type PlaneId,
} from "./bindings";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";
import { Select } from "./ui/select/Select";

/** The version the tab shows the artifact as of, as History picked it. */
export type AsOf = { id: string };

/** What comparing against the folder as it is now is called. */
const NOW = "now";

/**
 * **An artifact's History, in its bar** (spec #5948 story 47, #5966): its versions, each
 * Regenerate this machine kept the data of and each saved change to its page, newest first.
 *
 * - **Show this version** puts the tab on the artifact as of it ({@link AsOfStanding}): the page
 *   saved by then, with the data kept then. It reads nothing live and offers nothing, and its
 *   script runs only when it is the template approved here.
 * - **Compare** names the page's files and the data's values that changed between two versions,
 *   or between one and now.
 * - **How many are kept is said**: the data of the last few Regenerates, on this machine only.
 */
export function ArtifactHistory({
  plane,
  workspace,
  folder,
  name,
  onAsOf,
}: {
  plane: PlaneId;
  workspace: string;
  folder: string;
  name: string;
  /** The person asked to see the artifact as of a version. */
  onAsOf: (asOf: AsOf) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button size="sm" onPress={() => setOpen(true)} data-testid="artifact-history-button">
        History…
      </Button>
      {open ? (
        <HistoryDialog
          plane={plane}
          workspace={workspace}
          folder={folder}
          name={name}
          onClose={() => setOpen(false)}
          onAsOf={(asOf) => {
            setOpen(false);
            onAsOf(asOf);
          }}
        />
      ) : null}
    </>
  );
}

/** The History dialog: its versions read as it opens, and a comparison once asked for. */
export function HistoryDialog({
  plane,
  workspace,
  folder,
  name,
  onClose,
  onAsOf,
}: {
  plane: PlaneId;
  workspace: string;
  folder: string;
  name: string;
  onClose: () => void;
  onAsOf: (asOf: AsOf) => void;
}) {
  const [history, setHistory] = useState<History | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [version, setVersion] = useState("");
  const [against, setAgainst] = useState(NOW);
  const [compared, setCompared] = useState<ArtifactCompared | null>(null);
  const [comparing, setComparing] = useState(false);

  useEffect(() => {
    let gone = false;
    commands.artifactHistory(plane, workspace, folder).then(
      (answer) => {
        if (gone) return;
        if (answer.status === "ok") {
          setHistory(answer.data);
          setVersion(answer.data.versions[0]?.id ?? "");
        } else setFailed(readRefused(name, answer.error));
      },
      (why: unknown) => {
        if (!gone) setFailed(readRefused(name, String(why)));
      },
    );
    return () => {
      gone = true;
    };
  }, [plane, workspace, folder, name]);

  const versions = history?.versions ?? [];
  const options = versions.map((one) => ({ value: one.id, label: versionSaid(one) }));
  const compare = () => {
    setComparing(true);
    setFailed(null);
    commands.artifactCompare(plane, workspace, folder, version, against).then(
      (answer) => {
        setComparing(false);
        if (answer.status === "ok") setCompared(answer.data);
        else setFailed(compareRefused(answer.error));
      },
      (why: unknown) => {
        setComparing(false);
        setFailed(compareRefused(String(why)));
      },
    );
  };
  const any = versions.length > 0;
  return (
    <Dialog
      onClose={onClose}
      title={`History of ${name}`}
      description={
        history === null
          ? failed === null
            ? "Reading its versions…"
            : undefined
          : `purlis keeps the data of your last ${history.kept_most} Regenerates on this machine, never in the project. Earlier pages come from the project's saved history.`
      }
      cancel={{ children: "Close" }}
      main={
        any
          ? {
              children: "Show this version",
              onPress: () => onAsOf({ id: version }),
              isDisabled: version === "",
            }
          : undefined
      }
      alternative={
        any
          ? {
              children: "Compare",
              onPress: compare,
              isDisabled: comparing || version === "" || version === against,
            }
          : undefined
      }
      data-testid="artifact-history"
    >
      {history !== null && !any ? (
        <p data-testid="artifact-history-none">
          No versions yet. Each Regenerate keeps one, and so does each saved change to its page.
        </p>
      ) : null}
      {any ? (
        <div className="artifact-history-picks">
          <Select
            aria-label="Version"
            value={version}
            options={options}
            onValueChange={(to) => {
              setVersion(to);
              setCompared(null);
            }}
            data-testid="artifact-history-version"
          />
          <Select
            aria-label="Compare with"
            value={against}
            options={[{ value: NOW, label: "Now" }, ...options]}
            onValueChange={(to) => {
              setAgainst(to);
              setCompared(null);
            }}
            data-testid="artifact-history-against"
          />
        </div>
      ) : null}
      {failed === null ? null : (
        <p className="trouble" role="alert" data-testid="artifact-history-failed">
          {failed}
        </p>
      )}
      <div role="status" data-testid="artifact-history-compared">
        {compared === null ? null : <ComparedSaid compared={compared} />}
      </div>
    </Dialog>
  );
}

/** What a comparison found, in words. */
function ComparedSaid({ compared }: { compared: ArtifactCompared }) {
  if (compared.template.length === 0 && compared.data.length === 0)
    return <p>Nothing changed between these versions.</p>;
  return (
    <>
      {compared.template.length === 0 ? null : (
        <>
          <p>Its page:</p>
          <ul className="artifact-history-changes">
            {compared.template.map((change) => (
              <li key={change.path}>
                <code>{change.path}</code> {change.how}
              </li>
            ))}
          </ul>
        </>
      )}
      {compared.data.length === 0 ? null : (
        <>
          <p>Its data:</p>
          <ul className="artifact-history-changes">
            {compared.data.map((change) => (
              <li key={change.path}>
                <code>{change.path}</code>: {change.before ?? "nothing"} →{" "}
                {change.after ?? "nothing"}
              </li>
            ))}
          </ul>
        </>
      )}
      {compared.cut ? (
        <p>More changed than is listed here: these are the first {compared.data.length}.</p>
      ) : null}
    </>
  );
}

/** A version, as its option says it. */
export function versionSaid(version: ArtifactVersion): string {
  if (version.kind === "regenerated") return `${version.at} · Regenerated`;
  const by = version.by === null ? "" : ` by ${version.by}`;
  const said = version.said === null || version.said === "" ? "" : `: ${version.said}`;
  return `${version.at} · Page changed${by}${said}`;
}

/**
 * **Opens the artifact as of `asOf`** for its frame, handing what it is to `onThen`: the frame
 * is told only where its page stands, as for any page.
 */
export function openAsOf(
  plane: PlaneId,
  workspace: string,
  folder: string,
  asOf: AsOf,
  onThen: (then: ArtifactOpenedAsOf) => void,
): (
  first: ArtifactPlace,
  theme: ArtifactTheme,
) => Promise<{ status: "ok"; data: ArtifactOpened } | { status: "error"; error: string }> {
  return (first, theme) =>
    commands.artifactOpenAsOf(plane, workspace, folder, asOf.id, first, theme).then((answer) => {
      if (answer.status === "error") return answer;
      onThen(answer.data);
      return { status: "ok", data: answer.data.opened };
    });
}

/** **What the bar says of a tab shown as of a version**, and its way back to now. */
export function AsOfStanding({
  then,
  onBack,
}: {
  then: ArtifactOpenedAsOf | null;
  onBack: () => void;
}) {
  return (
    <>
      <span className="artifact-bar-standing" role="status" data-testid="artifact-as-of">
        {then === null ? "Opening an earlier version…" : asOfSaid(then)}
      </span>
      <Button size="sm" onPress={onBack} data-testid="artifact-as-of-back">
        Back to now
      </Button>
    </>
  );
}

/** The bar's sentence for a version: when, and what of it is not as it was. */
export function asOfSaid(then: ArtifactOpenedAsOf): string {
  const notes = [
    then.template === null ? "with today's page, as none was saved by then" : null,
    then.data_at === null ? "no data was kept by then" : null,
  ].filter((note) => note !== null);
  return notes.length === 0 ? `As of ${then.at}` : `As of ${then.at}, ${notes.join("; ")}`;
}

/** A refused read of the versions, and what to do: the dialog reads them once as it opens. */
function readRefused(name: string, why: string): string {
  return `purlis could not read the history of ${name}: ${why}. Close this and open History again.`;
}

/** A refused comparison, and what to do: Compare, beside it, is the retry. */
function compareRefused(why: string): string {
  return `purlis could not compare these versions: ${why}. Pick other versions and press Compare again.`;
}
