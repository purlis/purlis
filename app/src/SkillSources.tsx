// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState, type ReactNode } from "react";
import { Plus, RotateCw } from "lucide-react";
import {
  commands,
  type InstallReview,
  type OfferedSkill,
  type OfferedSource,
  type ReviewSkill,
  type ReviewSummary,
  type SkillChanged,
  type SkillLevel,
  type SkillSource,
  type SourceChecked,
  type SkillUpdateOffer,
  type SkillUpdates,
  type StudioLevel,
  type UpdateFile,
  type UpdateReview,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice, type NoticeAction } from "./Notice";
import { Choice, Field, SettingRow } from "./settings/components";
import { labelOf, picking, sameLevel } from "./skillChanges";
import { sourceKey, type SourcesAt } from "./skillStudio";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { Card, CardTitle } from "./ui/card/Card";
import { Fact, Facts } from "./ui/card/Facts";
import { Dialog } from "./ui/dialog/Dialog";
import { Disclosure } from "./ui/disclosure/Disclosure";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { List, ListRow } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ViewListHead } from "./ui/panel/ViewFrame";
import { ReviewPage } from "./ui/review-page/ReviewPage";

/**
 * **The Sources pages** (#3687; spec #3679, the amendment's S5, `prototype/skill-sources` A):
 * where skills are installed from (SK-10, #2773; ADR 0100). The person's own sources come first,
 * then the sources offered to start from, and each opens a page of its own; installing or
 * updating a skill is a review page with its action fixed at the foot. What is marked, each
 * commit's short name and what accepting says are the core's own.
 *
 * - **Sources**: the updates waiting, then each source the project's record names that this
 *   machine has not confirmed (purlis asks it nothing until it is), then the person's sources as
 *   rows, each with what its last check found ("14 skills, 2 installed · checked 3 hours ago",
 *   #3840, read from what that check recorded and never by asking the source), then the offered
 *   sources as cards (SK-22, decision 42). Add source… is a short dialog:
 *   the repository and who can browse it, with its branch, its folder and its sign-in under More.
 * - **A source's page**: its skills at its newest commit, each marked Installed, Update
 *   available, or with what its files do ("runs a script", "reaches 1 host"), with Check for
 *   updates and Remove source…, which keeps every skill installed from it.
 * - **The install review**: the pinned commit, what each skill adds with the risky items marked,
 *   each file one click away under its own mark, and where it lands, with Install at the foot.
 * - **The update review**: what changes in what it adds, the source's diff, the edits made here
 *   since the install, then Accept update or Not now at the foot.
 *
 * Both reviews are consent surfaces: they follow the review page's rules (`ui/review-page`), and
 * a main action that cannot be pressed says why.
 */

/** One change's answer, as the core's commands give it. */
type Answer = { status: "ok"; data: SkillChanged } | { status: "error"; error: string };

/** A read that may fail: what it answered, or why it could not. */
type Read<T> = { data: T; refused?: undefined } | { data?: undefined; refused: string };

/** Asks `run`, and answers what it read or why it failed, never throwing. */
async function reading<T>(
  run: () => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
): Promise<Read<T>> {
  try {
    const answer = await run();
    return answer.status === "ok" ? { data: answer.data } : { refused: answer.error };
  } catch (err) {
    return { refused: String(err) };
  }
}

/** Who can browse a source, as Add source asks and a source's row says. */
const WHO: Record<SkillSource["kept"], string> = {
  shared: "The whole project",
  local: "Only this machine",
};

/** What the Sources page says it holds, under its name. */
export const SOURCES_LEAD =
  "A source is a git repository of skills. An install copies a skill into this project, pinned to the commit you reviewed.";

/** What an action that cannot be pressed says while another change is being made. */
const BUSY = "Another change is being made.";

/** The update offered for `name` from the source `key`, if there is one. */
function offerFor(updates: SkillUpdates | undefined, key: string, name: string) {
  return updates?.offers.find((one) => sourceKey(one.remote) === key && one.name === name);
}

/**
 * **When a source was last checked** (#3840), `seconds` ago: an age, as the record of the last
 * browse or updates check gives it, never a time nobody saw. A check after now (a clock that
 * moved) is just now.
 */
export function checkedAgo(seconds: number, now: Date): string {
  if (seconds < 60) return "checked just now";
  if (seconds < 3600)
    return `checked ${counted(Math.floor(seconds / 60), "minute", "minutes")} ago`;
  if (seconds < 86_400)
    return `checked ${counted(Math.floor(seconds / 3600), "hour", "hours")} ago`;
  const days = Math.floor(seconds / 86_400);
  if (days === 1) return "checked yesterday";
  if (days < 30) return `checked ${days} days ago`;
  const at = new Date(now.getTime() - seconds * 1000);
  return `checked ${at.toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" })}`;
}

/**
 * **What the last check of a source found** (#3840): its skills and how many are installed from
 * it, then when it was checked, read with the source list and never by asking the source. `null`
 * where nothing is known: a source this machine never checked, with nothing installed from it.
 */
export function checkedSaid(one: SourceChecked | undefined, now: Date): string | null {
  if (one === undefined) return null;
  const counts =
    one.skills !== null
      ? `${counted(one.skills, "skill", "skills")}, ${one.installed} installed`
      : one.installed > 0
        ? `${one.installed} installed`
        : null;
  const when = one.at === null ? null : checkedAgo(now.getTime() / 1000 - one.at, now);
  const said = [counts, when].filter((part) => part !== null);
  return said.length === 0 ? null : said.join(" · ");
}

/** When a source was checked `at` (seconds since the epoch), as its page's lead says it. */
function checkedWhen(at: number | null | undefined): string | null {
  if (at === null || at === undefined) return null;
  const now = new Date();
  return checkedAgo(now.getTime() / 1000 - at, now);
}

/** What a source's row and its page say of it under its name, with what its last check found. */
function aboutSource(source: SkillSource, checked: string | null = null): string {
  return [
    source.remote.url,
    source.folder === null ? null : `folder ${source.folder}`,
    source.remote.branch === null ? null : `branch ${source.remote.branch}`,
    checked,
    WHO[source.kept],
    source.vault === null ? null : `signs in with ${source.vault}`,
  ]
    .filter((one) => one !== null)
    .join(" · ");
}

/** **What the Sources pages read**: the sources, the offered ones and the updates, read again
 *  each time something changed or Try again is pressed. */
function useSourcesRead(plane: string, changed: number) {
  const [asked, setAsked] = useState(0);
  const [sources, setSources] = useState<Read<readonly SkillSource[]>>();
  const [offered, setOffered] = useState<readonly OfferedSource[]>([]);
  const [updates, setUpdates] = useState<Read<SkillUpdates>>();
  const [checked, setChecked] = useState<readonly SourceChecked[]>([]);
  useEffect(() => {
    let live = true;
    // What the last checks recorded (#3840): read with the list, and again once the updates
    // check, which records each source it asked, has answered.
    const readChecked = () =>
      void reading(() => commands.skillSourcesChecked(plane)).then((read) => {
        if (live && Array.isArray(read.data)) setChecked(read.data);
      });
    void reading(() => commands.skillSources(plane)).then((read) => {
      if (live) setSources(read);
    });
    void reading(() => commands.skillSourcesOffered(plane)).then((read) => {
      if (live && read.data !== undefined) setOffered(read.data);
    });
    readChecked();
    void reading(() => commands.skillUpdates(plane)).then((read) => {
      if (live) setUpdates(read);
      readChecked();
    });
    return () => {
      live = false;
    };
  }, [plane, changed, asked]);
  return { sources, offered, updates, checked, again: () => setAsked((n) => n + 1) };
}

/** What the last check of `source` recorded, among `checked`. */
function checkedOf(checked: readonly SourceChecked[], source: SkillSource) {
  const key = sourceKey(source.remote);
  return checked.find((one) => sourceKey(one.remote) === key);
}

/** **Rows held while a read is asked** (S11): the layout stays, and after about a second a line
 *  says what is being read. */
function Reading({ what }: { what: string }) {
  const [late, setLate] = useState(false);
  useEffect(() => {
    const after = window.setTimeout(() => setLate(true), 1000);
    return () => window.clearTimeout(after);
  }, []);
  return (
    <div className="skill-studio-placeholders" aria-busy="true" data-testid="skill-sources-reading">
      {[0, 1, 2].map((at) => (
        <div key={at} className="skill-studio-placeholder" aria-hidden="true" />
      ))}
      <p role="status" className="honest">
        {late ? `Reading ${what}…` : ""}
      </p>
    </div>
  );
}

/** **A read in place**: rows held while it is asked, why it failed with Try again, or what it
 *  read, without hiding the rest of the page. */
function Held<T>({
  read,
  what,
  again,
  children,
}: {
  read: Read<T> | undefined;
  /** What is read, as the sentences name it: "this project's skill sources". */
  what: string;
  again: () => void;
  children: (data: T) => ReactNode;
}) {
  if (read === undefined) return <Reading what={what} />;
  if (read.refused !== undefined)
    return (
      <EmptyState
        headline={`purlis could not read ${what}`}
        body={read.refused}
        variant="panel"
        role="status"
        testid="skill-sources-unread"
        action={
          <Button size="sm" icon={<RotateCw aria-hidden="true" />} onPress={again}>
            Try again
          </Button>
        }
      />
    );
  return <>{children(read.data)}</>;
}

/**
 * **What the Sources page shows at `at`**: Sources itself, a source's page, or a review. Each is
 * a step of the tab's history, so back and forward walk them; a change made on one goes on to
 * where the prototype goes (an install to its source's page, an update and a removal to
 * Sources).
 */
export function SkillSourcesPage({
  plane,
  at,
  onGo,
  onOpenSkill,
  levels,
  start,
  busy,
  change,
  changed,
}: {
  plane: string;
  /** The page inside Sources, or none for Sources itself. */
  at: SourcesAt | undefined;
  /** Goes to a page inside Sources, or to Sources itself: a step of the tab's history. */
  onGo: (to: SourcesAt | undefined) => void;
  /** Opens an installed skill in the Library. */
  onOpenSkill: (name: string) => void;
  levels: readonly StudioLevel[];
  /** The level an install lands at unless the person picks another: where they came from. */
  start: SkillLevel;
  busy: boolean;
  change: (label: string, run: () => Promise<Answer>) => void;
  changed: number;
}) {
  const read = useSourcesRead(plane, changed);
  const [adding, setAdding] = useState<{ offered?: OfferedSource }>();
  // Where the tab is now, for a change that ends after the person has gone somewhere else.
  const here = useRef(at);
  useEffect(() => {
    here.current = at;
  }, [at]);
  /** A change, and where the tab goes once it is made: only from the page it was made on, so a
   *  person who went on meanwhile is never pulled back. */
  const changeThen = (label: string, run: () => Promise<Answer>, to?: SourcesAt) => {
    const from = at;
    change(label, () =>
      run().then((answer) => {
        if (answer.status === "ok" && here.current === from) onGo(to);
        return answer;
      }),
    );
  };
  const sources = read.sources?.data ?? [];
  const updates = read.updates?.data;
  const sourceAt =
    at === undefined ? undefined : sources.find((one) => sourceKey(one.remote) === at.source);
  const toSource = (one: SourcesAt): SourcesAt => ({
    at: "source",
    source: one.source,
    label: one.label,
  });

  let page: ReactNode;
  if (at === undefined)
    page = (
      <SourcesList
        read={read}
        busy={busy}
        onGo={onGo}
        onAdd={(offered) => setAdding({ offered })}
        onConfirm={(remote) =>
          change("The source was not confirmed", () => commands.skillSourceConfirm(plane, remote))
        }
      />
    );
  else if (at.at === "update")
    page = (
      <UpdateReviewPage
        plane={plane}
        at={at}
        offer={offerFor(updates, at.source, at.skill)}
        updatesRead={read.updates !== undefined}
        busy={busy}
        onCancel={() => onGo(sourceAt === undefined ? undefined : toSource(at))}
        onAccept={(offer, review) =>
          changeThen("The skill was not updated", () =>
            commands.skillUpdate(plane, offer.level, offer.name, review.accepted),
          )
        }
      />
    );
  else if (sourceAt === undefined)
    page = (
      <Held read={read.sources} what="this project's skill sources" again={read.again}>
        {() => (
          <EmptyState
            headline={`${at.label} is not among this project's sources`}
            body="It was removed, or it is kept on another machine."
            testid="skill-source-gone"
            action={<Button onPress={() => onGo(undefined)}>Go to Sources</Button>}
          />
        )}
      </Held>
    );
  else if (at.at === "install")
    page = (
      <InstallReviewPage
        plane={plane}
        source={sourceAt}
        name={at.skill}
        levels={levels}
        start={start}
        busy={busy}
        onCancel={() => onGo(toSource(at))}
        onInstall={(review, level) =>
          changeThen(
            "The skill was not installed",
            () => commands.skillInstall(plane, sourceAt, level, review.accepted, null),
            toSource(at),
          )
        }
      />
    );
  else
    page = (
      <SourcePage
        plane={plane}
        source={sourceAt}
        updates={updates}
        checked={checkedOf(read.checked, sourceAt)?.at ?? null}
        busy={busy}
        changed={changed}
        onGo={onGo}
        onOpenSkill={onOpenSkill}
        onChecked={read.again}
        onConfirm={() =>
          change("The source was not confirmed", () =>
            commands.skillSourceConfirm(plane, sourceAt.remote),
          )
        }
        onRemove={() =>
          changeThen("The source was not removed", () =>
            commands.skillSourceRemove(plane, sourceAt.remote),
          )
        }
      />
    );
  return (
    <>
      {page}
      {adding !== undefined && (
        <AddSourceDialog
          offered={adding.offered}
          busy={busy}
          onCancel={() => setAdding(undefined)}
          onAdd={(source) => {
            setAdding(undefined);
            change("The source was not added", () => commands.skillSourceAdd(plane, source));
          }}
        />
      )}
    </>
  );
}

// --- Sources -----------------------------------------------------------------------------------

/** **The Sources page**: the updates and the unconfirmed sources, which need the person, then
 *  their sources, then the ones offered to start from. */
function SourcesList({
  read,
  busy,
  onGo,
  onAdd,
  onConfirm,
}: {
  read: ReturnType<typeof useSourcesRead>;
  busy: boolean;
  onGo: (to: SourcesAt) => void;
  onAdd: (offered?: OfferedSource) => void;
  onConfirm: (remote: SkillSource["remote"]) => void;
}) {
  const updates = read.updates?.data;
  const offers = updates?.offers ?? [];
  const now = new Date();
  const review = (offer: SkillUpdateOffer) =>
    onGo({
      at: "update",
      source: sourceKey(offer.remote),
      label: offer.remote.label,
      skill: offer.name,
    });
  const fixes = offers.map((offer): NoticeAction => ({
    label: offers.length === 1 ? "Review update" : `Review ${offer.name}`,
    primary: true,
    busy,
    onPress: () => review(offer),
  }));
  const [firstFix, ...otherFixes] = fixes;
  return (
    <div className="skill-sources" data-testid="skill-sources">
      <ViewListHead title="Sources" lead={SOURCES_LEAD}>
        <div className="skill-library-tools">
          <Button
            size="sm"
            variant="primary"
            icon={<Plus aria-hidden="true" />}
            isDisabled={busy}
            tooltip={busy ? BUSY : undefined}
            onPress={() => onAdd()}
          >
            Add source…
          </Button>
        </div>
      </ViewListHead>
      <div className="skill-studio-page">
        {firstFix !== undefined && (
          <Notice
            cause="skill-updates"
            tone="attention"
            label={counted(offers.length, "update waiting", "updates waiting")}
            // Rebuilt so its type says it holds at least one.
            fixes={[firstFix, ...otherFixes]}
          >
            <span data-testid="skill-updates">
              {offers.length === 1
                ? "An installed skill has a newer version: "
                : "Installed skills have newer versions: "}
              {offers.map((offer, at) => (
                <span key={`${sourceKey(offer.remote)}\u0000${offer.name}`}>
                  {at > 0 && ", "}
                  {offer.name} from {offer.remote.label}
                  {offer.edited && " (edited here since it was installed)"}
                </span>
              ))}
              . Nothing changes until you accept it.
            </span>
          </Notice>
        )}
        {updates?.unconfirmed.map((one) => (
          <Notice
            key={sourceKey(one.remote)}
            cause={`skill-source-unconfirmed:${sourceKey(one.remote)}`}
            tone="attention"
            label="A source is not confirmed on this machine"
            fixes={[
              {
                label: "Confirm it",
                off: busy ? BUSY : undefined,
                primary: true,
                onPress: () => onConfirm(one.remote),
              },
            ]}
          >
            <span data-testid="skill-source-unconfirmed">
              {one.remote.label}, where {one.skills.join(", ")} came from, came with the project but
              is not confirmed on this machine, so purlis asks it nothing yet.
            </span>
          </Notice>
        ))}
        {updates?.unasked.map((one) => (
          <p key={`${one.label}\u0000${one.name}\u0000${one.why}`} className="honest">
            {one.name === "" ? one.why : `${one.name} (${one.label}): ${one.why}`}
          </p>
        ))}
        <section aria-labelledby="skill-sources-yours" className="skill-sources-part">
          <h3 id="skill-sources-yours" className="ui-text-small ui-text-strong">
            Your sources
          </h3>
          <Held read={read.sources} what="this project's skill sources" again={read.again}>
            {(sources) =>
              sources.length === 0 ? (
                <EmptyState
                  headline="This project has no skill sources yet"
                  body="A source you add is listed here, with its skills to review and install."
                  variant="panel"
                  testid="skill-sources-empty"
                  action={
                    <Button
                      size="sm"
                      isDisabled={busy}
                      tooltip={busy ? BUSY : undefined}
                      onPress={() => onAdd()}
                    >
                      Add source…
                    </Button>
                  }
                />
              ) : (
                <List
                  aria-label="Your sources"
                  onAction={(key) => {
                    const source = sources.find((one) => sourceKey(one.remote) === key);
                    if (source !== undefined)
                      onGo({ at: "source", source: String(key), label: source.label });
                  }}
                >
                  {sources.map((source) => {
                    const key = sourceKey(source.remote);
                    const waiting = offers.filter((one) => sourceKey(one.remote) === key).length;
                    const unconfirmed = updates?.unconfirmed.some(
                      (one) => sourceKey(one.remote) === key,
                    );
                    return (
                      <ListRow
                        key={key}
                        id={key}
                        textValue={source.label}
                        data-testid="skill-source"
                      >
                        <RowLabel>{source.label}</RowLabel>
                        {(waiting > 0 || unconfirmed === true) && (
                          <RowTrail>
                            {unconfirmed === true && (
                              <Badge variant="waiting">Not confirmed on this machine</Badge>
                            )}
                            {waiting > 0 && (
                              <Badge variant="accent">
                                {counted(waiting, "update", "updates")}
                              </Badge>
                            )}
                          </RowTrail>
                        )}
                        <RowBelow clamp={1}>
                          {aboutSource(source, checkedSaid(checkedOf(read.checked, source), now))}
                        </RowBelow>
                      </ListRow>
                    );
                  })}
                </List>
              )
            }
          </Held>
        </section>
        {read.offered.length > 0 && (
          <section aria-labelledby="skill-sources-offered" className="skill-sources-part">
            <h3 id="skill-sources-offered" className="ui-text-small ui-text-strong">
              Sources to start from
            </h3>
            <ul className="skill-offered-cards" aria-labelledby="skill-sources-offered">
              {read.offered.map((one) => (
                <Card
                  as="li"
                  key={`${sourceKey(one.source.remote)}\u0000${one.source.folder ?? ""}`}
                  data-testid="skill-source-offered"
                >
                  <CardTitle>{one.name}</CardTitle>
                  <p className="honest">{one.why}</p>
                  <ButtonRow size="sm">
                    <Button
                      size="sm"
                      isDisabled={busy}
                      tooltip={busy ? BUSY : undefined}
                      aria-label={`Add ${one.name}`}
                      onPress={() => onAdd(one)}
                    >
                      Add…
                    </Button>
                  </ButtonRow>
                </Card>
              ))}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}

/** **Add source…**: the repository and who can browse it, with its branch, its folder and its
 *  sign-in under More. Filled in with an offered source, it is the step that asks before that
 *  source is added. */
export function AddSourceDialog({
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
  const [kept, setKept] = useState<SkillSource["kept"]>(offered?.source.kept ?? "shared");
  const blank = (text: string) => (text.trim() === "" ? null : text.trim());
  const missing = url.trim() === "";
  const why = missing ? "Type the repository first." : busy ? BUSY : undefined;
  const add = () => {
    if (why !== undefined) return;
    onAdd({
      remote: { url: url.trim(), branch: blank(branch), label: url.trim() },
      folder: blank(folder),
      vault: blank(vault),
      key: blank(key),
      kept,
      label: url.trim(),
    });
  };
  return (
    <Dialog
      title={offered === undefined ? "Add a source" : `Add ${offered.name}`}
      description={
        offered === undefined ? (
          "purlis reads its list of skills when you open it. Nothing is installed until you review a skill from it."
        ) : (
          <span data-testid="skill-source-offered-asks">
            Add {offered.name} to this project&apos;s skill sources? purlis then reads its list of
            skills when you open it. Nothing is installed until you review a skill from it.
          </span>
        )
      }
      onClose={onCancel}
      data-testid="skill-source-add"
      main={{
        children: "Add source",
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: add,
      }}
    >
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
        label="Who can browse it"
        help="The whole project keeps it with the team; only this machine keeps it here."
        control={(ids) => (
          <Choice
            kind="select"
            ids={ids}
            value={kept}
            options={[
              { value: "shared", label: WHO.shared },
              { value: "local", label: WHO.local },
            ]}
            onValueChange={(to) => setKept(to === "local" ? "local" : "shared")}
          />
        )}
      />
      <Disclosure label="More: branch, folder, sign-in">
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
      </Disclosure>
    </Dialog>
  );
}

// --- A source's page ---------------------------------------------------------------------------

/** **A source's page**: its skills at its newest commit, each with its marks and its one way on,
 *  with Check for updates and Remove source…. */
function SourcePage({
  plane,
  source,
  updates,
  checked,
  busy,
  changed,
  onGo,
  onOpenSkill,
  onChecked,
  onConfirm,
  onRemove,
}: {
  plane: string;
  source: SkillSource;
  updates: SkillUpdates | undefined;
  /** When the source was last checked, as recorded (#3840), until its browse here answers. */
  checked: number | null;
  busy: boolean;
  changed: number;
  onGo: (to: SourcesAt) => void;
  onOpenSkill: (name: string) => void;
  /** Check for updates was pressed: the updates are asked again too. */
  onChecked: () => void;
  onConfirm: () => void;
  onRemove: () => void;
}) {
  const key = sourceKey(source.remote);
  const { read, again } = useRead(`${key}\u0000${changed}`, () =>
    commands.skillSourceBrowse(plane, source),
  );
  const [removing, setRemoving] = useState(false);
  const unconfirmed = updates?.unconfirmed.some((one) => sourceKey(one.remote) === key) === true;
  const at = (skill: string) => ({ source: key, label: source.label, skill });
  const offered = (skill: OfferedSkill) => offerFor(updates, key, skill.name);
  /** Where a skill's row goes: its update, its page in the Library, or its install. */
  const onward = (skill: OfferedSkill) => {
    if (offered(skill) !== undefined) onGo({ at: "update", ...at(skill.name) });
    else if (skill.installed.length > 0) onOpenSkill(skill.name);
    else onGo({ at: "install", ...at(skill.name) });
  };
  return (
    <div className="skill-sources" data-testid="skill-source-page">
      <ViewListHead
        title={source.label}
        lead={
          <>
            {aboutSource(source, checkedWhen(read?.data?.checked ?? checked))}
            {read?.data !== undefined && (
              <>
                {" "}
                · at <code>{read.data.commit.short}</code>
              </>
            )}
          </>
        }
      >
        <div className="skill-library-tools">
          <Button
            size="sm"
            icon={<RotateCw aria-hidden="true" />}
            isDisabled={read === undefined}
            tooltip={read === undefined ? "It is being read now." : undefined}
            onPress={() => {
              again();
              onChecked();
            }}
          >
            Check for updates
          </Button>
          <Button
            size="sm"
            isDisabled={busy}
            tooltip={busy ? BUSY : undefined}
            onPress={() => setRemoving(true)}
          >
            Remove source…
          </Button>
        </div>
      </ViewListHead>
      <div className="skill-studio-page">
        {unconfirmed && (
          <Notice
            cause={`skill-source-unconfirmed:${key}`}
            tone="attention"
            label="Not confirmed on this machine"
            fixes={[
              {
                label: "Confirm it",
                off: busy ? BUSY : undefined,
                primary: true,
                onPress: onConfirm,
              },
            ]}
          >
            purlis asks it nothing until you confirm it.
          </Notice>
        )}
        <Held read={read} what={source.label} again={again}>
          {(data) =>
            data.skills.length === 0 ? (
              <EmptyState
                headline="This source holds no skill under its folder"
                body="A skill is a folder with its own instructions, SKILL.md. Check the folder the source was added with."
                variant="panel"
                testid="skill-source-no-skills"
              />
            ) : (
              <List
                aria-label={`Skills in ${source.label}`}
                onAction={(folder) => {
                  const skill = data.skills.find((one) => one.folder === folder);
                  if (skill !== undefined) onward(skill);
                }}
              >
                {data.skills.map((skill) => {
                  const update = offered(skill);
                  const installed = skill.installed.length > 0;
                  return (
                    <ListRow
                      key={skill.folder}
                      id={skill.folder}
                      textValue={skill.name}
                      data-testid={`offered-${skill.name}`}
                    >
                      <RowLabel>
                        <code>{skill.name}</code>
                      </RowLabel>
                      <RowTrail>
                        {installed && <Badge variant="ok">Installed</Badge>}
                        {update !== undefined && <Badge variant="accent">Update available</Badge>}
                        {(skill.marks ?? []).map((mark) => (
                          <Badge key={mark} variant="waiting">
                            {mark}
                          </Badge>
                        ))}
                        <Button
                          size="sm"
                          variant={update !== undefined ? "primary" : undefined}
                          // Named for its row, its words first (WCAG 2.5.3, label in name).
                          aria-label={`${
                            update !== undefined
                              ? "Review update"
                              : installed
                                ? "See it in Library"
                                : "Review install"
                          }: ${skill.name}`}
                          onPress={() => onward(skill)}
                        >
                          {update !== undefined
                            ? "Review update"
                            : installed
                              ? "See it in Library"
                              : "Review install"}
                        </Button>
                      </RowTrail>
                      <RowBelow clamp={2}>
                        {skill.description}
                        {installed && ` · installed: ${skill.installed.join(", ")}`}
                        {skill.marks === null && " · its files are read when you review it"}
                      </RowBelow>
                    </ListRow>
                  );
                })}
              </List>
            )
          }
        </Held>
        {read?.data !== undefined && read.data.not_skills.length > 0 && (
          <p className="honest" data-testid="skill-source-not-skills">
            Not skills:{" "}
            {read.data.not_skills.map((one, at) => (
              <span key={one.folder}>
                {at > 0 && "; "}
                <code>{one.folder}</code>, {one.why}
              </span>
            ))}
            .
          </p>
        )}
      </div>
      {removing && (
        <Dialog
          title={`Remove ${source.label}?`}
          description="Skills you installed from it stay, pinned where they are; they just stop being offered its updates."
          onClose={() => setRemoving(false)}
          data-testid="skill-source-remove"
          main={{
            children: "Remove source",
            variant: "danger",
            isDisabled: busy,
            tooltip: busy ? BUSY : undefined,
            onPress: () => {
              setRemoving(false);
              onRemove();
            },
          }}
        />
      )}
    </div>
  );
}

// --- The reviews -------------------------------------------------------------------------------

/** **A read for a page**: asked each time its `key` is another, or when asked again. */
function useRead<T>(
  key: string | undefined,
  run: () => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
) {
  const [asked, setAsked] = useState(0);
  const [held, setHeld] = useState<{ key: string; read: Read<T> }>();
  const now = key === undefined ? undefined : `${key}\u0000${asked}`;
  useEffect(() => {
    if (now === undefined) return;
    let live = true;
    void reading(run).then((read) => {
      if (live) setHeld({ key: now, read });
    });
    return () => {
      live = false;
    };
    // Read again for another key only: `run` is a new function each draw.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [now]);
  return {
    read: held !== undefined && held.key === now ? held.read : undefined,
    again: () => setAsked((n) => n + 1),
  };
}

/** The mark of a risky item and of a risky file: in the waiting tone, never a colour alone. */
const LOOK_CLOSELY = <Badge variant="waiting">Look closely</Badge>;

/** A summary, risky items marked, as the core reads and marks it. */
export function Summary({ summary, testid }: { summary: ReviewSummary; testid?: string }) {
  if (summary.empty)
    return (
      <p className="honest" data-testid={testid}>
        Instructions only: no scripts, network hosts, ! commands or writes outside the workspace.
      </p>
    );
  const mark = (risky: boolean) => (risky ? LOOK_CLOSELY : null);
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

/** One skill of a review: what it adds first, then every file under its mark, each readable. */
function SkillReviewed({ skill }: { skill: ReviewSkill }) {
  return (
    <section className="skill-review" aria-label={`Review of ${skill.name}`}>
      <h3 className="ui-text-strong">
        <code>{skill.name}</code> {skill.description}
      </h3>
      {skill.brought_by !== null && (
        <p className="honest">Comes along: {skill.brought_by} refers to it.</p>
      )}
      <h4 className="ui-text-small ui-text-strong">What it adds</h4>
      <Summary summary={skill.summary} testid={`summary-${skill.name}`} />
      {skill.lint.map((line) => (
        <p key={line} className="honest">
          {line}
        </p>
      ))}
      <h4 className="ui-text-small ui-text-strong">Its files</h4>
      <div className="skill-review-files" role="group" aria-label={`Files of ${skill.name}`}>
        {skill.files.map((one) => (
          <Disclosure
            key={one.path}
            size="sm"
            label={`Show ${one.path}`}
            data-testid={`file-${one.path}`}
            trail={one.risky ? LOOK_CLOSELY : <Badge variant="ok">Plain</Badge>}
          >
            <pre className="skill-review-file" data-testid="skill-review-file">
              {one.text ?? "This file is not text, so it is not shown."}
            </pre>
          </Disclosure>
        ))}
      </div>
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

/** **The install review**: the pinned commit, each skill with its marks and its files, where it
 *  lands, and Install fixed at the foot. */
function InstallReviewPage({
  plane,
  source,
  name,
  levels,
  start,
  busy,
  onCancel,
  onInstall,
}: {
  plane: string;
  source: SkillSource;
  name: string;
  levels: readonly StudioLevel[];
  start: SkillLevel;
  busy: boolean;
  onCancel: () => void;
  onInstall: (review: InstallReview, level: SkillLevel) => void;
}) {
  const { read, again } = useRead(`${sourceKey(source.remote)}\u0000${name}`, () =>
    commands.skillInstallReview(plane, source, [name]),
  );
  const [level, setLevel] = useState<SkillLevel>(start);
  const picker = picking(
    levels.map((one) => one.level),
    (one) => labelOf(levels, one),
    sameLevel,
    level,
  );
  const review = read?.data;
  const risky = review?.skills.reduce((n, skill) => n + skill.summary.marked, 0) ?? 0;
  const why =
    read === undefined
      ? "The review is being read."
      : review === undefined
        ? "The review could not be read."
        : busy
          ? BUSY
          : undefined;
  return (
    <ReviewPage
      label={`Install ${name} from ${source.label}`}
      data-testid="install-review"
      summary="Nothing is installed until you press Install."
      onCancel={onCancel}
      main={{
        children: "Install",
        byReturn: false,
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: () => {
          if (review !== undefined) onInstall(review, level);
        },
      }}
    >
      <h2 className="ui-text-strong">
        Install <code>{name}</code> from {source.label}
      </h2>
      <Held read={read} what={`the review of ${name}`} again={again}>
        {(data) => (
          <>
            <p className="honest" data-testid="install-commit">
              You are reviewing {source.label} at commit <code>{data.commit.short}</code>:{" "}
              {counted(data.skills.length, "skill", "skills")}
              {risky > 0 && `, ${counted(risky, "risky item", "risky items")} marked`}. What you
              install stays pinned to this commit; a newer one is offered later, and changes nothing
              until you accept it. The review is read from the files: a script can do more when it
              runs, and the chat&apos;s sandbox bounds that. Installing copies all of them or none.
            </p>
            {data.skills.map((skill) => (
              <SkillReviewed key={skill.name} skill={skill} />
            ))}
            <SettingRow
              label="Lands in"
              grouped
              help={`It lands in the ${labelOf(levels, level)} skills.`}
              control={(ids) => (
                <Choice
                  kind="radio"
                  ids={ids}
                  options={picker.options}
                  value={picker.value}
                  disabled={busy}
                  onValueChange={(to) => {
                    const chosen = picker.chosen(to);
                    if (chosen !== undefined) setLevel(chosen);
                  }}
                />
              )}
            />
          </>
        )}
      </Held>
    </ReviewPage>
  );
}

/** Each file's change, folded, its kind of change after its path. */
function Changes({ files, open = false }: { files: readonly UpdateFile[]; open?: boolean }) {
  return (
    <>
      {files.map((file) => (
        <Disclosure
          key={file.path}
          size="sm"
          defaultExpanded={open}
          label={file.path}
          trail={<Badge>{file.change}</Badge>}
        >
          <pre className="skill-review-file">
            {file.diff === "" ? "Not text, so not shown." : file.diff}
          </pre>
        </Disclosure>
      ))}
    </>
  );
}

/** **The update review**: what changes, the source's diff, the edits made here since the
 *  install, then Accept update or Not now fixed at the foot. */
function UpdateReviewPage({
  plane,
  at,
  offer,
  updatesRead,
  busy,
  onCancel,
  onAccept,
}: {
  plane: string;
  at: SourcesAt & { at: "update" };
  /** The update offered, or none where there is no longer one (it was accepted elsewhere). */
  offer: SkillUpdateOffer | undefined;
  /** Whether the updates have been read: until they are, the offer is not known yet. */
  updatesRead: boolean;
  busy: boolean;
  onCancel: () => void;
  onAccept: (offer: SkillUpdateOffer, review: UpdateReview) => void;
}) {
  const { read, again } = useRead(
    offer === undefined ? undefined : `${at.source}\u0000${at.skill}\u0000${offer.newer.id}`,
    // Never asked without an offer (its key is none then); the guard is for the type alone.
    () =>
      offer === undefined
        ? Promise.resolve({ status: "error" as const, error: "There is no update of it." })
        : commands.skillUpdateReview(plane, offer.level, offer.name),
  );
  const review = read?.data;
  const why =
    offer === undefined
      ? "There is no update of it waiting."
      : read === undefined
        ? "The review is being read."
        : review === undefined
          ? "The review could not be read."
          : // It brings along a skill under a built-in skill's name, which accepting refuses (#3728).
            (review.refused ?? (busy ? BUSY : undefined));
  let body: ReactNode;
  if (offer === undefined)
    body = updatesRead ? (
      <EmptyState
        headline={`No update of ${at.skill} is waiting`}
        body="It was accepted, or its source has nothing newer now."
        variant="panel"
        testid="update-none"
      />
    ) : (
      <EmptyState headline="Reading the updates…" variant="panel" role="status" />
    );
  else
    body = (
      <Held read={read} what={`the update of ${at.skill}`} again={again}>
        {(data) => (
          <>
            <Facts aria-label={`The update of ${data.name}`}>
              <Fact label="From">{at.label}</Fact>
              <Fact label="Pinned now">
                <code>{data.pinned.short}</code>
              </Fact>
              <Fact label="Newer">
                <code>{data.newer.short}</code>
              </Fact>
            </Facts>
            {data.refused !== null && (
              <p className="honest" role="note" data-testid="update-refused">
                {data.refused}
              </p>
            )}
            {!data.from_pin && (
              <p className="honest">
                The pinned commit is no longer in the source, so this compares with the copy here.
              </p>
            )}
            <section aria-labelledby="update-what" className="skill-review">
              <h3 id="update-what" className="ui-text-strong">
                What changes
              </h3>
              {data.added.empty && data.removed.empty ? (
                <p className="honest" data-testid="update-same">
                  What it adds stays the same: no script, host or command comes or goes.
                </p>
              ) : (
                <>
                  <h4 className="ui-text-small ui-text-strong">New in what it adds</h4>
                  <Summary summary={data.added} testid="update-added" />
                  <h4 className="ui-text-small ui-text-strong">Gone from what it adds</h4>
                  <Summary summary={data.removed} testid="update-removed" />
                </>
              )}
            </section>
            <section aria-labelledby="update-source" className="skill-review">
              <h3 id="update-source" className="ui-text-strong">
                The source&apos;s change
              </h3>
              <Changes files={data.files} open={data.files.length === 1} />
            </section>
            {data.edited.length > 0 && (
              <section aria-labelledby="update-yours" className="skill-review">
                <h3 id="update-yours" className="ui-text-strong">
                  Your edits since the install
                </h3>
                <p className="honest" data-testid="update-edited">
                  Accepting replaces these: {data.edited.join(", ")}
                </p>
                <Changes files={data.local_edits} />
              </section>
            )}
            <Disclosure label={`All of ${data.name} at ${data.newer.short}`}>
              <SkillReviewed skill={data.skill} />
            </Disclosure>
            {data.brought.map((skill) => (
              <SkillReviewed key={skill.name} skill={skill} />
            ))}
          </>
        )}
      </Held>
    );
  return (
    <ReviewPage
      label={`Update ${at.skill}`}
      data-testid="update-review"
      summary="Chats keep the current version until you accept."
      onCancel={onCancel}
      cancel={{ children: "Not now" }}
      main={{
        children: "Accept update",
        byReturn: false,
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: () => {
          if (offer !== undefined && review !== undefined) onAccept(offer, review);
        },
      }}
    >
      <h2 className="ui-text-strong">
        Update <code>{at.skill}</code>
      </h2>
      {body}
    </ReviewPage>
  );
}
