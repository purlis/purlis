// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, type Ref } from "react";
import { FolderGit2, TriangleAlert } from "lucide-react";
import {
  commands,
  type InstallReview,
  type OfferedSource,
  type ReviewSkill,
  type ReviewSummary,
  type SkillChanged,
  type SkillLevel,
  type SkillSource,
  type SkillUpdateOffer,
  type SkillUpdates,
  type SourceBrowse,
  type StudioLevel,
  type UpdateFile,
  type UpdateReview,
} from "./bindings";
import { counted } from "./dispatches";
import { Button, ButtonRow } from "./ui/button/Button";
import { Notice } from "./Notice";
import { Select } from "./ui/select/Select";
import { Field, SettingRow } from "./settings/components";
import { labelOf, picking, sameLevel } from "./skillChanges";

/** One change's answer, as the core's commands give it. */
type Answer = { status: "ok"; data: SkillChanged } | { status: "error"; error: string };

/** What the sources section has open. */
type Open =
  /** Add source, empty, or filled in with an offered source (SK-22) for the person to confirm. */
  | { what: "add"; offered?: OfferedSource }
  | { what: "browse"; source: SkillSource; browse: SourceBrowse }
  | { what: "install"; source: SkillSource; review: InstallReview }
  | { what: "update"; offer: SkillUpdateOffer; review: UpdateReview };

/**
 * **Skill sources in the studio** (SK-10, #2773; ADR 0100): the project's sources, each one
 * browsed in place, an install reviewed summary first with every file one click away, a source
 * only the team's record names confirmed before it is asked anything, and each newer commit of
 * an installed skill offered as a diff of its summary and files, applied only when the person
 * accepts it. What is marked, what is empty, each commit's short name and what accepting says
 * are the core's own.
 *
 * **The offered sources** (SK-22, #2785; decision 42): the first-party catalogue and the
 * well-known sources the project does not browse yet, each with why it is offered. One click
 * opens Add source filled in with it, which the person confirms; nothing is added or installed
 * before that, and each install is still its own review.
 */
export function SkillSources({
  plane,
  levels,
  start,
  busy,
  change,
  changed,
  headingRef,
}: {
  plane: string;
  levels: readonly StudioLevel[];
  /** The level an install lands at unless the person picks another: where they came from. */
  start: SkillLevel;
  busy: boolean;
  change: (label: string, run: () => Promise<Answer>) => void;
  changed: number;
  /** The section's heading, which the empty studio's Browse sources moves to. */
  headingRef?: Ref<HTMLHeadingElement>;
}) {
  const [sources, setSources] = useState<readonly SkillSource[]>([]);
  /** The sources offered with one click, as the core lists them (`skills::first_open`). */
  const [offered, setOffered] = useState<readonly OfferedSource[]>([]);
  const [unread, setUnread] = useState<string>();
  const [updates, setUpdates] = useState<SkillUpdates>();
  const [open, setOpen] = useState<Open>();
  const [waiting, setWaiting] = useState<string>();
  const [refused, setRefused] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands
      .skillSources(plane)
      .then((answer) => {
        if (!live) return;
        if (answer.status === "ok" && Array.isArray(answer.data)) {
          setSources(answer.data);
          setUnread(undefined);
        } else if (answer.status === "error") setUnread(answer.error);
      })
      .catch(() => undefined);
    void commands
      .skillSourcesOffered(plane)
      .then((answer) => {
        if (live && answer.status === "ok" && Array.isArray(answer.data)) setOffered(answer.data);
      })
      .catch(() => undefined);
    void commands
      .skillUpdates(plane)
      .then((answer) => {
        if (live && answer.status === "ok" && Array.isArray(answer.data?.offers))
          setUpdates(answer.data);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, changed]);

  /** Runs a read that may take the network, saying what it waits on and why it failed. */
  const ask = <T,>(
    what: string,
    run: () => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
    then: (data: T) => void,
  ) => {
    setWaiting(what);
    setRefused(undefined);
    void run()
      .then((answer) => {
        if (answer.status === "ok") then(answer.data);
        else setRefused(answer.error);
      })
      .catch((err: unknown) => setRefused(String(err)))
      .finally(() => setWaiting(undefined));
  };
  const browse = (source: SkillSource) =>
    ask(
      `Reading ${source.label}…`,
      () => commands.skillSourceBrowse(plane, source),
      (data) => setOpen({ what: "browse", source, browse: data }),
    );
  const review = (source: SkillSource, name: string) =>
    ask(
      `Reviewing ${name}…`,
      () => commands.skillInstallReview(plane, source, [name]),
      (data) => setOpen({ what: "install", source, review: data }),
    );
  const reviewUpdate = (offer: SkillUpdateOffer) =>
    ask(
      `Reviewing the update of ${offer.name}…`,
      () => commands.skillUpdateReview(plane, offer.level, offer.name),
      (data) => setOpen({ what: "update", offer, review: data }),
    );
  const done = (label: string, run: () => Promise<Answer>) => {
    setOpen(undefined);
    change(label, run);
  };

  return (
    <section className="skill-sources" aria-label="Skill sources" data-testid="skill-sources">
      <h3 ref={headingRef} tabIndex={-1}>
        Skill sources
      </h3>
      <p className="honest">
        A skill source is a git repository of skills. An install copies a skill into this project,
        pinned to the commit you reviewed; a newer commit is offered here, and changes nothing until
        you accept it.
      </p>
      {updates !== undefined && updates.offers.length > 0 && (
        <ul className="skill-updates" aria-label="Updates">
          {updates.offers.map((offer) => (
            <li key={`${offer.label}\u0000${offer.name}`} data-testid={`update-${offer.name}`}>
              <code>{offer.name}</code> · {offer.label}: a newer commit, {offer.newer.short}
              {offer.edited && " (edited here since it was installed)"}{" "}
              <Button
                size="sm"
                isDisabled={busy || waiting !== undefined}
                onPress={() => reviewUpdate(offer)}
              >
                Review update
              </Button>
            </li>
          ))}
        </ul>
      )}
      {updates?.unconfirmed.map((one) => (
        <p
          key={`${one.remote.url}\u0000${one.remote.branch ?? ""}`}
          className="honest"
          data-testid="skill-source-unconfirmed"
        >
          {one.remote.label}, where {one.skills.join(", ")} came from, is not checked on this
          machine: purlis asks it nothing until you confirm it.{" "}
          <Button
            size="sm"
            isDisabled={busy}
            aria-label={`Confirm ${one.remote.label}`}
            onPress={() =>
              done("The source was not confirmed", () =>
                commands.skillSourceConfirm(plane, one.remote),
              )
            }
          >
            Confirm
          </Button>
        </p>
      ))}
      {updates?.unasked.map((one) => (
        <p key={`${one.label}\u0000${one.name}\u0000${one.why}`} className="honest">
          {one.name === "" ? one.why : `${one.name} (${one.label}): ${one.why}`}
        </p>
      ))}
      {unread !== undefined && (
        <p className="honest" data-testid="skill-sources-unread">
          {unread}
        </p>
      )}
      {unread === undefined && sources.length === 0 && open === undefined && (
        <p className="honest" data-testid="skill-sources-empty">
          This project has no skill sources yet. Add source takes a repository&apos;s https:// URL,
          or a folder on this machine.
        </p>
      )}
      {offered.length > 0 && open?.what !== "add" && (
        <section className="skill-offered" aria-label="Offered sources">
          <h4>Sources to start from</h4>
          <ul className="skill-source-list">
            {offered.map((one) => (
              <li
                key={`${one.source.remote.url}\u0000${one.source.folder ?? ""}`}
                data-testid="skill-source-offered"
              >
                <FolderGit2 className="node-icon" aria-hidden="true" /> <strong>{one.name}</strong>
                {one.offering === "catalogue" && " · from purlis"}: {one.why}{" "}
                <Button
                  size="sm"
                  isDisabled={busy}
                  aria-label={`Add ${one.name}`}
                  onPress={() => setOpen({ what: "add", offered: one })}
                >
                  Add…
                </Button>
              </li>
            ))}
          </ul>
        </section>
      )}
      <ul className="skill-source-list">
        {sources.map((source) => (
          <li
            key={`${source.remote.url}\u0000${source.remote.branch ?? ""}`}
            data-testid="skill-source"
          >
            <FolderGit2 className="node-icon" aria-hidden="true" /> {source.label}
            {source.kept === "local" && " · on this machine"}
            {source.vault !== null && ` · signs in with ${source.vault}`}{" "}
            <Button
              size="sm"
              isDisabled={waiting !== undefined}
              onPress={() => browse(source)}
              aria-label={`Browse ${source.label}`}
            >
              Browse
            </Button>
            <Button
              size="sm"
              isDisabled={busy}
              aria-label={`Remove source ${source.label}`}
              onPress={() =>
                done("The source was not removed", () =>
                  commands.skillSourceRemove(plane, source.remote),
                )
              }
            >
              Remove source
            </Button>
          </li>
        ))}
      </ul>
      {open === undefined && (
        <Button onPress={() => setOpen({ what: "add" })} isDisabled={busy}>
          Add source
        </Button>
      )}
      {waiting !== undefined && (
        <p className="pending" aria-busy="true">
          {waiting}
        </p>
      )}
      {refused !== undefined && (
        <Notice
          cause="skill-source"
          tone="trouble"
          label="The skill source could not be read"
          onDismiss={() => setRefused(undefined)}
        >
          <span data-testid="skill-source-refused">{refused}</span>
        </Notice>
      )}
      {open?.what === "add" && (
        <AddSource
          offered={open.offered}
          busy={busy}
          onCancel={() => setOpen(undefined)}
          onAdd={(source) =>
            done("The source was not added", () => commands.skillSourceAdd(plane, source))
          }
        />
      )}
      {open?.what === "browse" && (
        <section className="skill-change" aria-label={`Skills in ${open.source.label}`}>
          <h4>
            {open.source.label} at {open.browse.commit.short}
          </h4>
          {open.browse.skills.length === 0 && (
            <p className="honest">
              This source holds no skill under its folder: a skill is a folder with its own
              instructions.
            </p>
          )}
          <ul>
            {open.browse.skills.map((skill) => (
              <li key={skill.folder} data-testid={`offered-${skill.name}`}>
                <code>{skill.name}</code> {skill.description}
                {skill.installed.length > 0 && ` · installed: ${skill.installed.join(", ")}`}{" "}
                <Button
                  size="sm"
                  isDisabled={waiting !== undefined}
                  onPress={() => review(open.source, skill.name)}
                  aria-label={`Review ${skill.name}`}
                >
                  Review install
                </Button>
              </li>
            ))}
          </ul>
          {open.browse.not_skills.map((one) => (
            <p key={one.folder} className="honest">
              {one.folder}: {one.why}
            </p>
          ))}
          <Button onPress={() => setOpen(undefined)}>Close</Button>
        </section>
      )}
      {open?.what === "install" && (
        <InstallPanel
          review={open.review}
          levels={levels}
          start={start}
          busy={busy}
          onCancel={() => setOpen(undefined)}
          onInstall={(level) =>
            done("The skill was not installed", () =>
              commands.skillInstall(plane, open.source, level, open.review.accepted, null),
            )
          }
        />
      )}
      {open?.what === "update" && (
        <UpdatePanel
          review={open.review}
          busy={busy}
          onCancel={() => setOpen(undefined)}
          onAccept={() =>
            done("The skill was not updated", () =>
              commands.skillUpdate(plane, open.offer.level, open.offer.name, open.review.accepted),
            )
          }
        />
      )}
    </section>
  );
}

/** Add source: its URL, and optionally its branch, its folder and its vault entry. Filled in
 *  with an offered source, it is the step that asks before that source is added. */
function AddSource({
  offered,
  busy,
  onCancel,
  onAdd,
}: {
  offered?: OfferedSource;
  busy: boolean;
  onCancel: () => void;
  onAdd: (source: SkillSource) => void;
}) {
  const [url, setUrl] = useState(offered?.source.remote.url ?? "");
  const [branch, setBranch] = useState(offered?.source.remote.branch ?? "");
  const [folder, setFolder] = useState(offered?.source.folder ?? "");
  const [vault, setVault] = useState("");
  const [key, setKey] = useState("");
  const [local, setLocal] = useState(false);
  const blank = (text: string) => (text.trim() === "" ? null : text.trim());
  return (
    <form
      className="skill-change"
      aria-label={offered === undefined ? "Add source" : `Add ${offered.name}`}
      onSubmit={(event) => {
        event.preventDefault();
        onAdd({
          remote: { url: url.trim(), branch: blank(branch), label: url.trim() },
          folder: blank(folder),
          vault: blank(vault),
          key: blank(key),
          kept: local ? "local" : "shared",
          label: url.trim(),
        });
      }}
    >
      {offered !== undefined && (
        <p className="honest" data-testid="skill-source-offered-asks">
          Add {offered.name} to this project&apos;s skill sources? purlis then reads its list of
          skills when you browse it. Nothing is installed until you review a skill from it.
        </p>
      )}
      <SettingRow
        label="Repository"
        help="An https:// URL, or a folder on this machine by its full path."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={url}
            placeholder="https://github.com/acme/skills"
            onChange={setUrl}
          />
        )}
      />
      <SettingRow
        label="Branch"
        help="Empty for the repository's own default."
        control={(ids) => <Field kind="text" ids={ids} value={branch} onChange={setBranch} />}
      />
      <SettingRow
        label="Folder"
        help="Where its skills are, such as skills. Empty for its top."
        control={(ids) => <Field kind="text" ids={ids} value={folder} onChange={setFolder} />}
      />
      <SettingRow
        label="Signs in with vault"
        help="For a private repository: the vault, then the entry that holds its token. The token stays in the vault, and only this machine signs in with it."
        control={(ids) => <Field kind="text" ids={ids} value={vault} onChange={setVault} />}
      />
      <SettingRow
        label="Entry"
        help="The entry in that vault."
        control={(ids) => <Field kind="text" ids={ids} value={key} onChange={setKey} />}
      />
      <Select
        aria-label="Who browses it"
        value={local ? "local" : "shared"}
        options={[
          { value: "shared", label: "The whole project" },
          { value: "local", label: "Only this machine" },
        ]}
        onValueChange={(to) => setLocal(to === "local")}
      />
      <ButtonRow>
        <Button variant="primary" type="submit" isDisabled={busy || url.trim() === ""}>
          Add it
        </Button>
        <Button onPress={onCancel}>Cancel</Button>
      </ButtonRow>
    </form>
  );
}

/** A summary, risky items marked, as the core reads and marks it. */
export function Summary({ summary, testid }: { summary: ReviewSummary; testid?: string }) {
  if (summary.empty)
    return (
      <p className="honest" data-testid={testid}>
        Instructions only: no scripts, network hosts, ! commands or writes outside the workspace.
      </p>
    );
  const mark = (risky: boolean) =>
    risky ? <TriangleAlert className="node-icon risky" aria-label="Risky" /> : null;
  return (
    <ul className="skill-summary" data-testid={testid}>
      {summary.scripts.map((one) => (
        <li key={`s${one.path}`} data-risky={one.risks.length > 0}>
          {mark(one.risks.length > 0)} Script <code>{one.path}</code> runs with {one.runs}
          {one.risks.length > 0 && `: ${one.risks.map((risk) => risk.said).join(", ")}`}
        </li>
      ))}
      {summary.hosts.map((one) => (
        <li key={`h${one.host}`} data-risky={one.risky}>
          {mark(one.risky)} Network host <code>{one.host}</code>, named in {one.files.join(", ")}
        </li>
      ))}
      {summary.commands.map((one) => (
        <li key={`c${one.path}${one.line}${one.text}`} data-risky={true}>
          {mark(true)} ! command <code>{one.text}</code> ({one.path}, line {one.line})
        </li>
      ))}
      {summary.outside.map((one) => (
        <li key={`o${one.path}${one.line}${one.text}`} data-risky={true}>
          {mark(true)} Writes outside the workspace: <code>{one.text}</code> ({one.path}, line{" "}
          {one.line})
        </li>
      ))}
      {summary.unseen.map((one) => (
        <li key={`u${one.path}${one.line}`} data-risky={true}>
          {mark(true)} Hidden text in {one.path}, line {one.line}:{" "}
          {one.kinds.map((kind) => kind.said).join(", ")}
        </li>
      ))}
      {summary.not_text.map((path) => (
        <li key={`n${path}`}>
          <code>{path}</code> is not text, so it is not shown
        </li>
      ))}
    </ul>
  );
}

/** One skill of a review: its summary first, then every file, each one click away. */
function SkillReviewed({ skill }: { skill: ReviewSkill }) {
  const [shown, setShown] = useState<string>();
  const file = skill.files.find((one) => one.path === shown);
  return (
    <section className="skill-review" aria-label={`Review of ${skill.name}`}>
      <h4>
        <code>{skill.name}</code> {skill.description}
      </h4>
      {skill.brought_by !== null && (
        <p className="honest">Comes along: {skill.brought_by} refers to it.</p>
      )}
      <Summary summary={skill.summary} testid={`summary-${skill.name}`} />
      {skill.lint.map((line) => (
        <p key={line} className="honest">
          {line}
        </p>
      ))}
      <ul className="skill-review-files" aria-label={`Files of ${skill.name}`}>
        {skill.files.map((one) => (
          <li key={one.path} data-risky={one.risky}>
            <Button
              size="sm"
              variant="ghost"
              aria-label={`Show ${one.path}`}
              onPress={() => setShown(shown === one.path ? undefined : one.path)}
            >
              {one.path}
            </Button>{" "}
            {one.risky && <TriangleAlert className="node-icon risky" aria-label="Risky" />}
          </li>
        ))}
      </ul>
      {file !== undefined && (
        <pre className="skill-review-file" data-testid="skill-review-file">
          {file.text ?? "This file is not text, so it is not shown."}
        </pre>
      )}
      {skill.left_out.length > 0 && (
        <ul className="honest" aria-label={`Not installed from ${skill.name}`}>
          {skill.left_out.map((one) => (
            <li key={one.path}>
              Not installed: <code>{one.path}</code>, {one.said}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** The install review: each skill, then where it lands, and Install. */
function InstallPanel({
  review,
  levels,
  start,
  busy,
  onCancel,
  onInstall,
}: {
  review: InstallReview;
  levels: readonly StudioLevel[];
  start: SkillLevel;
  busy: boolean;
  onCancel: () => void;
  onInstall: (level: SkillLevel) => void;
}) {
  const [level, setLevel] = useState<SkillLevel>(start);
  const picker = picking(
    levels.map((one) => one.level),
    (one) => labelOf(levels, one),
    sameLevel,
    level,
  );
  const risky = review.skills.reduce((n, skill) => n + skill.summary.marked, 0);
  return (
    <section className="skill-change" aria-label="Install review" data-testid="install-review">
      <p className="honest">
        {counted(review.skills.length, "skill", "skills")} at {review.commit.short}
        {risky > 0 && `, ${counted(risky, "risky item", "risky items")} marked`}. The review is read
        from the files: a script can do more when it runs, and the chat&apos;s sandbox bounds that.
        Installing copies all of them or none.
      </p>
      {review.skills.map((skill) => (
        <SkillReviewed key={skill.name} skill={skill} />
      ))}
      <p className="honest" data-testid="install-level">
        It lands in the {labelOf(levels, level)} skills.{" "}
        <Select
          aria-label="Lands in"
          value={picker.value}
          options={picker.options}
          onValueChange={(to) => {
            const chosen = picker.chosen(to);
            if (chosen !== undefined) setLevel(chosen);
          }}
        />
      </p>
      <ButtonRow>
        <Button variant="primary" isDisabled={busy} onPress={() => onInstall(level)}>
          Install
        </Button>
        <Button onPress={onCancel}>Cancel</Button>
      </ButtonRow>
    </section>
  );
}

/** Each file's change, folded. */
function Changes({ files }: { files: readonly UpdateFile[] }) {
  return (
    <>
      {files.map((file) => (
        <details key={file.path} className="skill-update-file">
          <summary>
            <code>{file.path}</code> {file.change}
          </summary>
          <pre>{file.diff === "" ? "Not text, so not shown." : file.diff}</pre>
        </details>
      ))}
    </>
  );
}

/** An update's review: what the summary gains and loses, each file's diff, the edits made here
 *  and the skills that come along, then Accept. */
function UpdatePanel({
  review,
  busy,
  onCancel,
  onAccept,
}: {
  review: UpdateReview;
  busy: boolean;
  onCancel: () => void;
  onAccept: () => void;
}) {
  return (
    <section
      className="skill-change"
      aria-label={`Update of ${review.name}`}
      data-testid="update-review"
    >
      <p className="honest">
        {review.name}: {review.pinned.short} to {review.newer.short}.
        {!review.from_pin &&
          " The pinned commit is no longer in the source, so this compares with the copy here."}
      </p>
      <h4>New in the summary</h4>
      <Summary summary={review.added} testid="update-added" />
      <h4>Gone from the summary</h4>
      <Summary summary={review.removed} testid="update-removed" />
      <h4>The source&apos;s change</h4>
      <Changes files={review.files} />
      {review.edited.length > 0 && (
        <>
          <h4>Your edits since the install</h4>
          <p className="honest" data-testid="update-edited">
            Accepting replaces these: {review.edited.join(", ")}
          </p>
          <Changes files={review.local_edits} />
        </>
      )}
      <SkillReviewed skill={review.skill} />
      {review.brought.map((skill) => (
        <SkillReviewed key={skill.name} skill={skill} />
      ))}
      <ButtonRow>
        <Button variant="primary" isDisabled={busy} onPress={onAccept}>
          Accept update
        </Button>
        <Button onPress={onCancel}>Not now</Button>
      </ButtonRow>
    </section>
  );
}
