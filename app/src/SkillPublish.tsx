// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Publish a skill to a source, and export it** (SK-18, #2781; `prototype/skill-publish`).
 *
 * Publish is a review page: everything that would go, and the draft request's button fixed at the
 * foot, out of reach while anything stops it. Nothing goes until that button is pressed, and the
 * press hands back what the page showed, so the core refuses where any of it moved. Export is a
 * short dialog, then the system's save dialog.
 */
import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  commands,
  type ExportReview,
  type PublishDone,
  type PublishReviewView,
  type SkillLevel,
  type SkillSource,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { Held, useRead } from "./SkillSources";
import { NotRead, ReadingRows } from "./SkillPanels";
import { Choice, SettingRow } from "./settings/components";
import { Badge } from "./ui/badge/Badge";
import { Fact, Facts } from "./ui/card/Facts";
import { Dialog } from "./ui/dialog/Dialog";
import { Disclosure } from "./ui/disclosure/Disclosure";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Button } from "./ui/button/Button";
import { ReviewPage } from "./ui/review-page/ReviewPage";
import { Select } from "./ui/select/Select";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";

/** A file's size, as a review row says it. */
function sizeSaid(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}

/** Only a source on a forge takes a request: a folder on this machine is no place to publish. */
const onAForge = (source: SkillSource) => source.remote.url.startsWith("https://");

/** **The publish review page**: see the module. */
export function SkillPublishPage({
  plane,
  name,
  level,
  onCancel,
  onExport,
  onMove,
  onSources,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  onCancel: () => void;
  onExport: () => void;
  onMove: () => void;
  onSources: () => void;
}) {
  const sources = useRead("sources", () => commands.skillSources(plane));
  const offered = (sources.read?.data ?? []).filter(onAForge);
  const [picked, setPicked] = useState<string>();
  const source = offered.find((one) => one.label === picked) ?? offered[0];
  const { read, again } = useRead(source?.label, () =>
    // Never asked without a source (its key is none then); the guard is for the type alone.
    source === undefined
      ? Promise.resolve({ status: "error" as const, error: "No source is chosen." })
      : commands.skillPublishReview(plane, level, name, source),
  );
  const [sending, setSending] = useState(false);
  const [done, setDone] = useState<PublishDone>();
  const [refused, setRefused] = useState<string>();
  const review = read?.data;
  const request = review?.request ?? "pull request";

  const send = async () => {
    if (review === undefined || source === undefined) return;
    setSending(true);
    setRefused(undefined);
    try {
      const answer = await commands.skillPublish(plane, level, name, source, review.accepted);
      if (answer.status === "ok") setDone(answer.data);
      else setRefused(answer.error);
    } catch (err) {
      setRefused(String(err));
    } finally {
      setSending(false);
    }
  };

  if (done !== undefined && done.kind !== "cannot-push")
    return (
      <div className="skill-publish-done" data-testid="publish-opened">
        <Notice
          cause="skill-publish-opened"
          tone="news"
          label={
            done.kind === "opened"
              ? `Draft ${request} opened`
              : `These files already have a ${request}`
          }
          fixes={[
            {
              label: `Open on ${review?.forge ?? "the forge"}`,
              onPress: () => void openUrl(done.url),
            },
          ]}
          onDismiss={onCancel}
        >
          {done.kind === "opened"
            ? `Draft ${request} opened. Mark it ready on ${review?.forge ?? "the forge"} when you want it reviewed.`
            : `These files already have a ${request}. Nothing new was sent.`}
        </Notice>
      </div>
    );

  const stopped = review !== undefined && (review.local.length > 0 || review.stops.length > 0);
  const why = sending
    ? "The request is being opened."
    : source === undefined
      ? "No source is chosen."
      : read === undefined
        ? "The review is being read."
        : review === undefined
          ? "The review could not be read."
          : stopped
            ? "Fix what stops it first."
            : undefined;
  const main = `Open draft ${request}`;

  return (
    <ReviewPage
      label={`Publish ${name} to a source`}
      data-testid="publish-review"
      summary={stopped ? "Fix what stops it first." : "Opens a draft. You mark it ready."}
      summaryReserve={["Fix what stops it first.", "Opens a draft. You mark it ready."]}
      onCancel={onCancel}
      cancel={{ children: "Cancel", isDisabled: sending }}
      main={{
        children: sending ? "Opening…" : main,
        reserve: [main, "Opening…", "Open draft merge request", "Open draft pull request"],
        byReturn: false,
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: () => void send(),
      }}
    >
      <h2 className="ui-text-strong">
        Publish <code>{name}</code> to a source
      </h2>
      <p className="honest">
        This opens a draft {request} under your own account. Nothing is sent until you press the
        button below.
      </p>
      {done?.kind === "cannot-push" && (
        <Notice
          cause="skill-publish-cannot-push"
          tone="trouble"
          label="git could not push to this source"
          fixes={[
            { label: "Try again", onPress: () => setDone(undefined) },
            { label: "Export…", onPress: onExport },
          ]}
        >
          {`Nothing was sent. It takes write access to the source: ask for it, or export the skill and send it to them. git said: ${done.why}`}
        </Notice>
      )}
      {refused !== undefined && (
        <Notice
          cause="skill-publish-refused"
          tone="trouble"
          label="The skill was not published"
          fixes={[
            {
              label: "Review again",
              onPress: () => {
                setRefused(undefined);
                again();
              },
            },
          ]}
        >
          {refused}
        </Notice>
      )}
      {review !== undefined && review.local.length > 0 && (
        <Notice
          cause="skill-publish-local"
          tone="trouble"
          label={`${review.local.join(", ")} ${review.local.length === 1 ? "is" : "are"} kept on this machine, so nothing can be published`}
          fixes={
            review.local.includes(name)
              ? [{ label: "Move…", onPress: onMove }]
              : [{ label: "Review again", onPress: again }]
          }
        >
          {`${review.local.join(", ")} ${review.local.length === 1 ? "is" : "are"} kept on this machine, so nothing can be published. Move ${review.local.length === 1 ? "it" : "them"} to a shared level first.`}
        </Notice>
      )}
      <Held read={sources.read} what="this project's skill sources" again={sources.again}>
        {() =>
          source === undefined ? (
            <EmptyState
              headline="No source to publish to"
              body="Add a skill source on GitHub or GitLab, then publish to it."
              action={<Button onPress={onSources}>Go to sources</Button>}
            />
          ) : (
            <SettingRow
              label="Source"
              control={(ids) => (
                <Select
                  {...ids}
                  value={source.label}
                  isDisabled={sending}
                  options={offered.map((one) => ({ value: one.label, label: one.label }))}
                  onValueChange={(to) => {
                    setPicked(to);
                    setDone(undefined);
                    setRefused(undefined);
                  }}
                />
              )}
            />
          )
        }
      </Held>
      {source !== undefined &&
        (read === undefined ? (
          <ReadingRows what="what would go" testid="publish-reading" />
        ) : read.refused !== undefined ? (
          <NotRead
            what="what would go"
            why={read.refused}
            onAgain={again}
            testid="publish-unread"
            size="sm"
          />
        ) : (
          review !== undefined && <Reviewed review={review} onAgain={again} />
        ))}
    </ReviewPage>
  );
}

/** Everything the review shows: what stops it, where it goes, what goes, what stays, the grades. */
function Reviewed({ review, onAgain }: { review: PublishReviewView; onAgain: () => void }) {
  return (
    <>
      {review.stops.length > 0 && (
        <section aria-label="What stops it" data-testid="publish-stops">
          <Notice
            cause="skill-publish-stopped"
            tone="trouble"
            label={`${counted(review.stops.length, "thing stops", "things stop")} this publish`}
            fixes={[{ label: "Review again", onPress: onAgain }]}
          >
            {`${counted(review.stops.length, "thing stops", "things stop")} this publish. Fix them in the skill, then review again. There is no way to send it past them.`}
          </Notice>
          <ul className="skill-publish-rows">
            {review.stops.map((stop) => (
              <li key={`${stop.place}${stop.said}`}>
                <code>{stop.place}</code>
                <span className="honest">{stop.said}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
      <Facts aria-label="Where it goes">
        <Fact label="Goes into">
          <code>{review.base}</code>, reviewed at <code>{review.commit_short}</code>
        </Fact>
        <Fact label="New branch">
          <code>{review.branch}</code>
        </Fact>
        <Fact label="Lands in">
          {review.lands.map((land) => (
            <span key={land.folder} className="skill-publish-path">
              <code>{land.folder}/</code>
              {land.replaces && <Badge variant="waiting">Replaces what is there</Badge>}
            </span>
          ))}
        </Fact>
        <Fact label="Title">{review.title}</Fact>
        <Fact label="Commit names">{review.author}</Fact>
      </Facts>
      <section aria-label="Files that go">
        <h3 className="ui-text-small ui-text-strong">Files that go ({review.files.length})</h3>
        <ul className="skill-publish-rows">
          {review.files.map((file) => (
            <li key={file.path}>
              <span className="skill-publish-path">
                <code>{file.path}</code>
                {file.runs && <Badge variant="waiting">Runs a script</Badge>}
                {file.comes_along && <Badge>Comes along</Badge>}
              </span>
              <span className="honest">{sizeSaid(file.bytes)}</span>
            </li>
          ))}
        </ul>
      </section>
      {review.behind.length > 0 && (
        <section aria-label="Stays behind">
          <h3 className="ui-text-small ui-text-strong">Stays behind ({review.behind.length})</h3>
          <ul className="skill-publish-rows">
            {review.behind.map((file) => (
              <li key={file.path}>
                <code>{file.path}</code>
                <span className="honest">{file.why}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {review.broken.length > 0 && (
        <p className="honest" data-testid="publish-broken">
          It refers to {review.broken.join(", ")}, which {review.broken.length === 1 ? "is" : "are"}{" "}
          not there.
        </p>
      )}
      <section aria-label="Grades in the description">
        <h3 className="ui-text-small ui-text-strong">Grades in the description</h3>
        {review.grades.length === 0 ? (
          <p className="honest">Not graded.</p>
        ) : (
          <Table aria-label="Grades in the description" data-testid="publish-grades">
            <TableHeader>
              <TableColumn isRowHeader>Skill</TableColumn>
              <TableColumn>When</TableColumn>
              <TableColumn>Version</TableColumn>
              <TableColumn>Harness</TableColumn>
              <TableColumn>Grader</TableColumn>
              <TableColumn align="end">With the skill</TableColumn>
              <TableColumn align="end">Without</TableColumn>
              <TableColumn align="end">Runs</TableColumn>
              <TableColumn>Ended</TableColumn>
            </TableHeader>
            <TableBody>
              {review.grades.map((one, at) => (
                <TableRow key={at} id={at} textValue={one.when}>
                  <TableCell>{one.skill}</TableCell>
                  <TableCell>{one.when}</TableCell>
                  <TableCell>{one.version}</TableCell>
                  <TableCell>{one.harness}</TableCell>
                  <TableCell>{one.grader}</TableCell>
                  <TableCell>{one.with_it}</TableCell>
                  <TableCell>{one.without_it}</TableCell>
                  <TableCell>{one.runs}</TableCell>
                  <TableCell>{one.ended}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
        <p className="honest">No chat text, paths, costs or device names go with them.</p>
      </section>
      <Disclosure size="sm" label="The request's description, whole">
        <pre className="skill-review-file" data-testid="publish-body">
          {review.body}
        </pre>
      </Disclosure>
    </>
  );
}

/** What the export dialog says it writes. */
function writes(name: string, review: ExportReview | undefined): string {
  const along = (review?.skills ?? []).filter((one) => one !== name);
  const also =
    along.length === 0
      ? ""
      : `, and ${along.join(", ")}, ${along.length === 1 ? "the skill" : "the skills"} it refers to`;
  return `Writes a copy of ${name}${also} that works outside purlis.`;
}

/** **The export dialog**: a zip or a folder, then where. */
export function ExportDialog({
  plane,
  name,
  level,
  onClose,
  onExported,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  onClose: () => void;
  onExported: (to: string) => void;
}) {
  const { read } = useRead(`${name}`, () => commands.skillExportReview(plane, level, name));
  const [format, setFormat] = useState("zip");
  const [saving, setSaving] = useState(false);
  const [refused, setRefused] = useState<string>();
  const stops = read?.data?.stops ?? [];
  const save = async () => {
    setSaving(true);
    setRefused(undefined);
    try {
      const answer = await commands.skillExport(plane, level, name, format === "zip");
      if (answer.status === "error") setRefused(answer.error);
      else if (answer.data !== null) onExported(answer.data);
    } catch (err) {
      setRefused(String(err));
    } finally {
      setSaving(false);
    }
  };
  return (
    <Dialog
      onClose={onClose}
      title={`Export ${name}`}
      titleId="skill-export"
      description={writes(name, read?.data)}
      main={{
        children: "Choose where to save…",
        isDisabled: saving || read === undefined || stops.length > 0,
        tooltip: stops.length > 0 ? "Fix what stops it first." : undefined,
        onPress: () => void save(),
      }}
    >
      <SettingRow
        label="Export as"
        grouped
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            value={format}
            disabled={saving}
            onValueChange={setFormat}
            options={[
              { value: "zip", label: "A zip", says: "One file to send to someone" },
              { value: "folder", label: "A folder", says: "Skill folders to drop into a project" },
            ]}
          />
        )}
      />
      {read?.refused !== undefined && <p className="honest">{read.refused}</p>}
      {(read?.data?.behind ?? []).map((file) => (
        <p key={file.path} className="honest">
          Stays behind: <code>{file.path}</code>, {file.why}
        </p>
      ))}
      {(read?.data?.broken ?? []).map((one) => (
        <p key={one} className="honest">
          It refers to {one}, which is not there.
        </p>
      ))}
      {stops.map((stop) => (
        <p key={`${stop.place}${stop.said}`} className="honest">
          <code>{stop.place}</code> {stop.said}
        </p>
      ))}
      {refused !== undefined && <p className="honest">{refused}</p>}
    </Dialog>
  );
}
