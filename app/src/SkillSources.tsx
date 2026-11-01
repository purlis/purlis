// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, type ReactNode } from "react";
import { Plus, RotateCw } from "lucide-react";
import {
  commands,
  type InstallReview,
  type InstallSetReview,
  type OfferedSkill,
  type OfferedSource,
  type ReviewSkill,
  type ReviewSummary,
  type SkillChanged,
  type SkillGrant,
  type SkillLevel,
  type SkillSource,
  type SourceChecked,
  type SkillUpdateOffer,
  type SkillRename,
  type SkillUpdates,
  type SkillWidening,
  type StudioLevel,
  type UpdateFile,
  type UpdateReview,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice, type NoticeAction } from "./Notice";
import { Choice, Field, SettingRow } from "./settings/components";
import { labelOf, picking, sameLevel } from "./skillChanges";
import { nameProblem } from "./SkillChangeDialogs";
import { GrantPicker } from "./SkillLevelPickers";
import { TextField } from "./ui/field/Field";
import { NotRead, ReadingRows } from "./SkillPanels";
import { sourceKey, type SourcesAt } from "./skillStudio";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { Card, CardTitle } from "./ui/card/Card";
import { Checkbox } from "./ui/choice/Checkbox";
import { Fact, Facts } from "./ui/card/Facts";
import { Dialog } from "./ui/dialog/Dialog";
import { Disclosure } from "./ui/disclosure/Disclosure";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { List, ListRow } from "./ui/list/List";
import { RowBelow, RowLabel, RowLead, RowTrail } from "./ui/list/Row";
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
 *   each file one click away under its own mark, and where it lands, with Install at the foot. At
 *   the project level it is granted there too, the root's until the person picks another, and
 *   says which project skill it relies on that grant widens (#4185, #4172).
 * - **Install all…** (#4024): the source's skills ticked, every one not installed yet, to untick
 *   and narrow; then one review of those ticked, where they land chosen once at its top, each
 *   skill's risky items marked and its files read when it is unfolded, and one Install for all of
 *   them as one change. Past what one read of the source may take, it says so and asks to install
 *   them in two parts.
 * - **The update review**: what changes in what it adds, where it reaches (what comes along lands
 *   with the updated skill's grant, and which project skill it relies on that grant widens,
 *   #4186), the source's diff, the edits made here since the install, then Accept update or Not
 *   now at the foot.
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
  if (days < 30) return `checked ${counted(days, "day", "days")} ago`;
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

/** Now, drawn again each minute, so an age said on screen keeps up with the clock. */
function useMinute(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const ticking = window.setInterval(() => setNow(new Date()), 60_000);
    return () => window.clearInterval(ticking);
  }, []);
  return now;
}

/** When a source was checked `at` (seconds since the epoch), as its page's lead says it. */
function checkedWhen(at: number | null | undefined, now: Date): string | null {
  if (at === null || at === undefined) return null;
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
function useSourcesRead(plane: string, changed: number, onTheList: boolean) {
  const [asked, setAsked] = useState(0);
  const [sources, setSources] = useState<Read<readonly SkillSource[]>>();
  const [offered, setOffered] = useState<readonly OfferedSource[]>([]);
  const [updates, setUpdates] = useState<Read<SkillUpdates>>();
  const [checked, setChecked] = useState<readonly SourceChecked[]>([]);
  // Bumped as the updates check answers, which records each source it asked.
  const [answered, setAnswered] = useState(0);
  useEffect(() => {
    let live = true;
    void reading(() => commands.skillSources(plane)).then((read) => {
      if (live) setSources(read);
    });
    void reading(() => commands.skillSourcesOffered(plane)).then((read) => {
      if (live && read.data !== undefined) setOffered(read.data);
    });
    void reading(() => commands.skillUpdates(plane)).then((read) => {
      if (live) setUpdates(read);
      if (live) setAnswered((n) => n + 1);
    });
    return () => {
      live = false;
    };
  }, [plane, changed, asked]);
  // What the last checks recorded (#3840): read with the list, again once the updates check has
  // answered, and again on coming back to the list, since a source's page browses its source.
  useEffect(() => {
    if (!onTheList) return;
    let live = true;
    void reading(() => commands.skillSourcesChecked(plane)).then((read) => {
      if (live && Array.isArray(read.data)) setChecked(read.data);
    });
    return () => {
      live = false;
    };
  }, [plane, changed, asked, answered, onTheList]);
  return { sources, offered, updates, checked, again: () => setAsked((n) => n + 1) };
}

/** What the last check of `source` recorded, among `checked`. */
function checkedOf(checked: readonly SourceChecked[], source: SkillSource) {
  const key = sourceKey(source.remote);
  return checked.find((one) => sourceKey(one.remote) === key);
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
  if (read === undefined) return <ReadingRows what={what} testid="skill-sources-reading" />;
  if (read.refused !== undefined)
    return (
      <NotRead
        what={what}
        why={read.refused}
        onAgain={again}
        testid="skill-sources-unread"
        size="sm"
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
  stillAt,
  onOpenSkill,
  levels,
  workspaces,
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
  /** Whether the tab is still at `from`, asked when a change ends: the tab's own place, so a
   *  person who went to Library meanwhile, or a page drawn again, is judged by where the tab is
   *  and not by this page's life (#3973). */
  stillAt: (from: SourcesAt | undefined) => boolean;
  /** Opens an installed skill in the Library. */
  onOpenSkill: (name: string) => void;
  levels: readonly StudioLevel[];
  /** The project's workspaces, which an install at the project level may be granted to. */
  workspaces: readonly string[];
  /** The level an install lands at unless the person picks another: where they came from. */
  start: SkillLevel;
  busy: boolean;
  change: (label: string, run: () => Promise<Answer>) => void;
  changed: number;
}) {
  const read = useSourcesRead(plane, changed, at === undefined);
  const [adding, setAdding] = useState<{ offered?: OfferedSource }>();
  /** A change, and where the tab goes once it is made: only from the page it was made on, so a
   *  person who went on meanwhile is never pulled back. */
  const changeThen = (label: string, run: () => Promise<Answer>, to?: SourcesAt) => {
    const from = at;
    change(label, () =>
      run().then((answer) => {
        if (answer.status === "ok" && stillAt(from)) onGo(to);
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
            commands.skillUpdate(plane, offer.level, offer.name, review.accepted, review.grant),
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
  else if (at.at === "choose")
    page = (
      <ChooseSkillsPage
        plane={plane}
        source={sourceAt}
        updates={updates}
        changed={changed}
        onCancel={() => onGo(toSource(at))}
        onOpenSkill={onOpenSkill}
        onUpdate={(skill) => onGo({ at: "update", source: at.source, label: at.label, skill })}
        onReview={(skills) =>
          onGo({ at: "install-all", source: at.source, label: at.label, skills })
        }
        kept={at.ticked}
      />
    );
  else if (at.at === "install-all")
    page = (
      <InstallSetReviewPage
        // A new set is a new review, and what was unticked in the last one is forgotten.
        key={at.skills.join("\u0000")}
        plane={plane}
        source={sourceAt}
        named={at.skills}
        levels={levels}
        workspaces={workspaces}
        start={start}
        busy={busy}
        // Back to the list as it was ticked.
        onCancel={() =>
          onGo({ at: "choose", source: at.source, label: at.label, ticked: at.skills })
        }
        onInstall={(review, level, grant, renames) =>
          changeThen(
            "The skills were not installed",
            () => commands.skillInstall(plane, sourceAt, level, review.accepted, grant, renames),
            toSource(at),
          )
        }
      />
    );
  else if (at.at === "install")
    page = (
      <InstallReviewPage
        plane={plane}
        source={sourceAt}
        name={at.skill}
        levels={levels}
        workspaces={workspaces}
        start={start}
        busy={busy}
        onCancel={() => onGo(toSource(at))}
        onInstall={(review, level, grant, renames) =>
          changeThen(
            "The skill was not installed",
            () => commands.skillInstall(plane, sourceAt, level, review.accepted, grant, renames),
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
  const now = useMinute();
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
  const now = useMinute();
  const unconfirmed = updates?.unconfirmed.some((one) => sourceKey(one.remote) === key) === true;
  const at = (skill: string) => ({ source: key, label: source.label, skill });
  const offered = (skill: OfferedSkill) => offerFor(updates, key, skill.name);
  /** Why Install all… cannot be pressed: the skills are not read yet, or none is left. */
  const allWhy =
    read?.data === undefined
      ? read === undefined
        ? "It is being read now."
        : "Its skills could not be read."
      : read.data.skills.length === 0
        ? "This source holds no skill."
        : read.data.skills.every((skill) => skill.installed.length > 0)
          ? "Every skill here is installed."
          : undefined;
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
            {aboutSource(source, checkedWhen(read?.data?.checked ?? checked, now))}
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
            isDisabled={allWhy !== undefined}
            tooltip={allWhy}
            onPress={() => onGo({ at: "choose", source: key, label: source.label })}
          >
            Install all…
          </Button>
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

/** The change a review is of, as its lines name it. */
type Change = "install" | "update";

/**
 * **What a reviewed skill says of each skill it refers to and leaves as it is** (#4032, #4172,
 * #4186): its chats there have it already, or it is a project skill whose grant the change
 * widens to where the change reaches, and each workspace that turned it off, whose chats get the
 * reviewed skill without it.
 */
function Relies({
  skill,
  widenings,
  by,
}: {
  skill: ReviewSkill;
  /** None while what the change widens is not known: it is read, or could not be. */
  widenings: readonly SkillWidening[] | undefined;
  by: Change;
}) {
  return (
    <>
      {skill.already.map((one) => {
        const widened = widenings?.find((each) => each.name === one);
        return (
          <div key={one} data-testid={`already-${skill.name}-${one}`}>
            {widenings === undefined ? (
              <p className="honest">
                It refers to <code>{one}</code>, which the project has: it is left as it is.
              </p>
            ) : widened?.added != null ? (
              <p className="honest">
                It refers to <code>{one}</code>, which is granted to {widened.granted}: it is left
                as it is, and the {by} grants it to {widened.added} too.
              </p>
            ) : (
              <p className="honest">
                It refers to <code>{one}</code>, which its chats there have already: it is left as
                it is.
              </p>
            )}
            {widened?.turned_off.map((ws) => (
              <p key={ws} className="honest">
                {ws} turned <code>{one}</code> off, so its chats get {skill.name} without it.
              </p>
            ))}
          </div>
        );
      })}
    </>
  );
}

/** How the person settled a skill's name clash: unsettled until they choose. */
type Settling = { how?: "skip" | "rename"; name?: string };

/** The new name offered, or what the person typed over it. */
function newName(skill: ReviewSkill, settling: Settling | undefined): string {
  return settling?.name ?? skill.clash?.offered ?? "";
}

/** Why `skill`'s new name cannot be used, as it is typed. The core checks again at Install. */
function newNameProblem(skill: ReviewSkill, settling: Settling | undefined): string | undefined {
  const wanted = newName(skill, settling).trim();
  if (wanted === "") return "Give it a name.";
  return nameProblem(wanted, skill.name, [skill.name, ...(skill.clash?.taken ?? [])], []);
}

/**
 * **The choices a review's name clashes wait for** (ADR 0100, amended 2026-11-01): each clash
 * unsettled until the person skips it or names it, and the renames Install hands the core.
 */
function useClashes(skills: readonly ReviewSkill[], onSkip?: (name: string) => void) {
  const [settled, setSettled] = useState<Readonly<Record<string, Settling>>>({});
  const clashing = skills.filter((skill) => skill.clash !== null);
  const skipped = clashing.filter((skill) => settled[skill.name]?.how === "skip");
  const renaming = clashing.filter((skill) => settled[skill.name]?.how === "rename");
  const open = clashing.filter((skill) => settled[skill.name]?.how === undefined);
  const bad = renaming.some((skill) => newNameProblem(skill, settled[skill.name]) !== undefined);
  const wanted = renaming.map((skill) => newName(skill, settled[skill.name]).trim());
  const twice = wanted.some((name, at) => wanted.indexOf(name) !== at);
  const settle = (name: string, change: Settling) =>
    setSettled((was) => ({ ...was, [name]: { ...was[name], ...change } }));
  return {
    renames: renaming.map((skill) => ({
      from: skill.name,
      to: newName(skill, settled[skill.name]).trim(),
    })),
    why:
      open.length > 0
        ? open.length === 1
          ? `Choose what to do with ${open[0]?.name}: its name is taken.`
          : `Choose what to do with the ${open.length} skills whose names are taken.`
        : bad
          ? "Fix the new name."
          : twice
            ? "Two skills cannot have the same name."
            : skipped.length === skills.length && skills.length > 0
              ? "Every skill is skipped, so there is nothing to install."
              : undefined,
    choice: (skill: ReviewSkill, busy: boolean) =>
      skill.clash === null ? undefined : (
        <ClashChoice
          skill={skill}
          settling={settled[skill.name]}
          busy={busy}
          onSettle={(change) => {
            settle(skill.name, change);
            if (change.how === "skip") onSkip?.(skill.name);
          }}
        />
      ),
  };
}

/** **A name that is taken**: what holds it, and the two ways on, in the skill's own review. */
function ClashChoice({
  skill,
  settling,
  busy,
  onSettle,
}: {
  skill: ReviewSkill;
  settling: Settling | undefined;
  busy: boolean;
  onSettle: (change: Settling) => void;
}) {
  const problem = settling?.how === "rename" ? newNameProblem(skill, settling) : undefined;
  return (
    <div className="skill-clash" data-testid={`clash-${skill.name}`}>
      <SettingRow
        label="This name is taken"
        grouped
        help={`${skill.clash?.held_by ?? ""}. A new name changes only its name line and folder: its text stays as the source wrote it.`}
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            options={[
              { value: "skip", label: "Skip it" },
              { value: "rename", label: "Install it under a new name" },
            ]}
            value={settling?.how ?? ""}
            disabled={busy}
            onValueChange={(to) => onSettle({ how: to === "skip" ? "skip" : "rename" })}
          />
        )}
      />
      {settling?.how === "rename" && (
        <TextField
          label={`New name for ${skill.name}`}
          value={newName(skill, settling)}
          isDisabled={busy}
          onChange={(name) => onSettle({ name })}
          error={problem}
          data-testid={`new-name-${skill.name}`}
        />
      )}
    </div>
  );
}

/** The grant an install starts with: chats at the project's root alone (ADR 0100 §2). */
const ROOT: SkillGrant = { to: "root" };

/** Whether an install at `level` is granted: one into the project's shared skills alone. */
function grantedAt(level: SkillLevel): boolean {
  return level.at === "project" && !level.local;
}

/**
 * **Where an install reaches** (#4185, ADR 0100 §2): the grant picked in its review, the root's
 * until the person picks another, at the project level alone; and what it asks of the project
 * skills `review` relies on (#4172), asked again each time the grant changes, never the source.
 * Install waits for that answer, so what it widens is shown before it is pressed.
 */
function useInstallGrant(plane: string, level: SkillLevel, review: InstallReview | undefined) {
  const [grant, setGrant] = useState<SkillGrant>(ROOT);
  const asks = grantedAt(level);
  const already = [...new Set(review?.skills.flatMap((skill) => skill.already) ?? [])];
  // Nothing beyond the root is widened, so only a grant past it asks.
  const asked = review !== undefined && asks && grant.to !== "root" && already.length > 0;
  const { read, again } = useRead(
    asked
      ? `${JSON.stringify(level)}\u0000${JSON.stringify(grant)}\u0000${already.join("\u0000")}`
      : undefined,
    () => commands.skillInstallWidenings(plane, level, grant, already),
  );
  return {
    /** The grant Install hands the core: none where it is not a project install. */
    grant: asks ? grant : null,
    picked: grant,
    asks,
    setGrant,
    /** What it widens; none while it is read, or where it could not be. */
    widenings: asked ? read?.data : [],
    /** Why Install waits on it, if it does. */
    why: !asked
      ? undefined
      : read === undefined
        ? "Where it reaches is being read."
        : read.refused !== undefined
          ? "Where it reaches could not be read."
          : undefined,
    read: asked ? read : undefined,
    again,
  };
}

/** The grant row of an install review at the project level, and why what it widens was unread. */
function InstallGrant({
  label,
  workspaces,
  busy,
  reach,
}: {
  /** The picker's name: "Where deploy will reach". */
  label: string;
  workspaces: readonly string[];
  busy: boolean;
  reach: ReturnType<typeof useInstallGrant>;
}) {
  if (!reach.asks) return null;
  return (
    <>
      <SettingRow
        label="Reaches"
        grouped
        help="A project skill reaches only the chats its grant names. Change it afterwards from the Library."
        control={() => (
          <GrantPicker
            label={label}
            workspaces={workspaces}
            now={reach.picked}
            disabled={busy}
            onPick={reach.setGrant}
          />
        )}
      />
      {reach.read?.refused !== undefined && (
        <NotRead
          what="where the install reaches"
          why={reach.read.refused}
          onAgain={reach.again}
          testid="install-reach-unread"
          size="sm"
        />
      )}
    </>
  );
}

/** One skill of a review: what it adds first, then every file under its mark, each readable.
 *  In a review of many (#4024) its box to leave it out comes first, and its files are folded
 *  under one line, read when it is unfolded. */
function SkillReviewed({
  skill,
  pick,
  folded = false,
  widenings = [],
  by = "install",
  relies = true,
  clash,
}: {
  skill: ReviewSkill;
  /** The choice its name's clash waits for, where it has one (ADR 0100, amended 2026-11-01). */
  clash?: ReactNode;
  /** Its box in a review of many: ticked, to untick and leave it out. */
  pick?: ReactNode;
  /** Its files under one line, drawn only once it is unfolded. */
  folded?: boolean;
  /** What the change asks of the project skills it relies on (#4172, #4186); none while it is
   *  not known. */
  widenings?: readonly SkillWidening[] | undefined;
  /** The change, as its lines name it. */
  by?: Change;
  /** Whether it says what it refers to and leaves as it is: not where the page said so above. */
  relies?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const files = (
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
  );
  return (
    <section
      className="skill-review"
      aria-label={`Review of ${skill.name}`}
      data-testid={`reviewed-${skill.name}`}
    >
      {pick}
      <h3 className="ui-text-strong">
        <code>{skill.name}</code> {skill.description}
      </h3>
      {skill.brought_by !== null && (
        <p className="honest">Comes along: {skill.brought_by} refers to it.</p>
      )}
      {clash}
      {skill.notes.map((line) => (
        <p key={line} className="honest" data-testid={`note-${skill.name}`}>
          {line}
        </p>
      ))}
      {relies && <Relies skill={skill} widenings={widenings} by={by} />}
      <h4 className="ui-text-small ui-text-strong">What it adds</h4>
      <Summary summary={skill.summary} testid={`summary-${skill.name}`} />
      {skill.lint.map((line) => (
        <p key={line} className="honest">
          {line}
        </p>
      ))}
      {folded ? (
        <Disclosure
          size="sm"
          label={`Its files (${skill.files.length})`}
          isExpanded={open}
          onExpandedChange={setOpen}
          data-testid={`files-${skill.name}`}
          trail={skill.files.some((one) => one.risky) ? LOOK_CLOSELY : undefined}
        >
          {open && files}
        </Disclosure>
      ) : (
        <>
          <h4 className="ui-text-small ui-text-strong">Its files</h4>
          {files}
        </>
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

/** **The install review**: the pinned commit, each skill with its marks and its files, where it
 *  lands, and Install fixed at the foot. */
function InstallReviewPage({
  plane,
  source,
  name,
  levels,
  workspaces,
  start,
  busy,
  onCancel,
  onInstall,
}: {
  plane: string;
  source: SkillSource;
  name: string;
  levels: readonly StudioLevel[];
  workspaces: readonly string[];
  start: SkillLevel;
  busy: boolean;
  onCancel: () => void;
  onInstall: (
    review: InstallReview,
    level: SkillLevel,
    grant: SkillGrant | null,
    renames: SkillRename[],
  ) => void;
}) {
  const [level, setLevel] = useState<SkillLevel>(start);
  // Read again where it lands changes: a skill it refers to that the level holds already is
  // left as it is, not brought along (#4032).
  const { read, again } = useRead(
    `${sourceKey(source.remote)}\u0000${name}\u0000${JSON.stringify(level)}`,
    () => commands.skillInstallReview(plane, source, [name], level),
  );
  const picker = picking(
    levels.map((one) => one.level),
    (one) => labelOf(levels, one),
    sameLevel,
    level,
  );
  const review = read?.data;
  const reach = useInstallGrant(plane, level, review);
  const clashes = useClashes(review?.skills ?? []);
  const risky = review?.skills.reduce((n, skill) => n + skill.summary.marked, 0) ?? 0;
  const why =
    read === undefined
      ? "The review is being read."
      : review === undefined
        ? "The review could not be read."
        : busy
          ? BUSY
          : (clashes.why ?? reach.why);
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
          if (review !== undefined) onInstall(review, level, reach.grant, clashes.renames);
        },
      }}
    >
      <h2 className="ui-text-strong">
        Install <code>{name}</code> from {source.label}
      </h2>
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
      <InstallGrant
        label={`Where ${name} will reach`}
        workspaces={workspaces}
        busy={busy}
        reach={reach}
      />
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
              <SkillReviewed
                key={skill.name}
                skill={skill}
                widenings={reach.widenings}
                clash={clashes.choice(skill, busy)}
              />
            ))}
          </>
        )}
      </Held>
    </ReviewPage>
  );
}

/**
 * **Install all…, the skills to tick** (#4024, decisions 1 and 2): every skill of the source at
 * its newest commit, each one not installed yet ticked, to untick and narrow. One installed is
 * greyed, its box out of reach, marked Installed with See it in Library; one whose update waits
 * says so, and its update stays on Review update, never in this install. Review goes on to the
 * one review of those ticked.
 */
function ChooseSkillsPage({
  plane,
  source,
  updates,
  changed,
  onCancel,
  onOpenSkill,
  onUpdate,
  onReview,
  kept,
}: {
  plane: string;
  source: SkillSource;
  updates: SkillUpdates | undefined;
  changed: number;
  /** What was ticked when the person came back from the review, kept as they left it. */
  kept?: readonly string[];
  onCancel: () => void;
  onOpenSkill: (name: string) => void;
  onUpdate: (name: string) => void;
  onReview: (names: string[]) => void;
}) {
  const key = sourceKey(source.remote);
  const { read, again } = useRead(`${key}\u0000${changed}`, () =>
    commands.skillSourceBrowse(plane, source),
  );
  const skills = read?.data?.skills ?? [];
  const open = skills.filter((skill) => skill.installed.length === 0).map((skill) => skill.name);
  // Every skill not installed yet, ticked, until the person unticks one (decision 1).
  const [ticked, setTicked] = useState<ReadonlySet<string> | undefined>(
    kept === undefined ? undefined : new Set(kept),
  );
  const chosen = ticked ?? new Set(open);
  const names = open.filter((name) => chosen.has(name));
  const why =
    read === undefined
      ? "Its skills are being read."
      : read.data === undefined
        ? "Its skills could not be read."
        : names.length === 0
          ? "Tick at least one skill."
          : undefined;
  return (
    <ReviewPage
      label={`Install all from ${source.label}`}
      data-testid="install-all-choose"
      summary="Nothing is installed until you review them and press Install."
      onCancel={onCancel}
      main={{
        children:
          names.length === 0 ? "Review" : `Review ${counted(names.length, "skill", "skills")}`,
        byReturn: false,
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: () => onReview(names),
      }}
    >
      <h2 className="ui-text-strong">Install all from {source.label}</h2>
      <Held read={read} what={source.label} again={again}>
        {(data) => (
          <>
            <p className="honest" data-testid="install-all-lead">
              Every skill not installed yet is ticked; untick one to leave it out. Those ticked go
              through one review, and install together as one change, pinned to commit{" "}
              <code>{data.commit.short}</code>.
            </p>
            <List
              aria-label={`Skills to install from ${source.label}`}
              selectionMode="multiple"
              // An installed skill's box is out of reach; its buttons are not.
              disabledBehavior="selection"
              disabledKeys={data.skills
                .filter((skill) => skill.installed.length > 0)
                .map((skill) => skill.name)}
              selectedKeys={chosen}
              onSelectionChange={(keys) =>
                setTicked(
                  new Set(
                    keys === "all" ? open : [...keys].map(String).filter((k) => open.includes(k)),
                  ),
                )
              }
            >
              {data.skills.map((skill) => {
                const row = {
                  installed: skill.installed.length > 0,
                  update: offerFor(updates, key, skill.name),
                  marks: skill.marks ?? [],
                };
                return (
                  <ListRow
                    key={skill.name}
                    id={skill.name}
                    textValue={skill.name}
                    data-testid={`choose-${skill.name}`}
                    data-installed={row.installed || undefined}
                  >
                    <RowLead>
                      <Checkbox
                        slot="selection"
                        aria-label={`Install ${skill.name}`}
                        tooltip={row.installed ? "It is installed already." : undefined}
                      />
                    </RowLead>
                    {/* Installed: greyed, its words stepped back to the meta tier (decision 2). */}
                    <RowLabel className={row.installed ? "ui-text-meta" : undefined}>
                      <code>{skill.name}</code>
                    </RowLabel>
                    <RowTrail>
                      {row.update !== undefined ? (
                        <Badge variant="accent">Update waiting</Badge>
                      ) : (
                        row.installed && <Badge variant="ok">Installed</Badge>
                      )}
                      {!row.installed &&
                        row.marks.map((mark) => (
                          <Badge key={mark} variant="waiting">
                            {mark}
                          </Badge>
                        ))}
                      {row.update !== undefined ? (
                        <Button
                          size="sm"
                          aria-label={`Review update: ${skill.name}`}
                          onPress={() => onUpdate(skill.name)}
                        >
                          Review update
                        </Button>
                      ) : (
                        row.installed && (
                          <Button
                            size="sm"
                            aria-label={`See it in Library: ${skill.name}`}
                            onPress={() => onOpenSkill(skill.name)}
                          >
                            See it in Library
                          </Button>
                        )
                      )}
                    </RowTrail>
                    <RowBelow clamp={2}>
                      {skill.description}
                      {row.installed && ` · installed: ${skill.installed.join(", ")}`}
                      {!row.installed &&
                        skill.marks === null &&
                        " · its files are read when you review it"}
                    </RowBelow>
                  </ListRow>
                );
              })}
            </List>
          </>
        )}
      </Held>
    </ReviewPage>
  );
}

/** The first part of an install in two, as a sentence names it: "the first 12 skills". */
function firstPart(n: number): string {
  return n === 1 ? "the first skill" : `the first ${n} skills`;
}

/** How many risky items `review` marks, over all its skills. */
function riskyIn(review: InstallReview): number {
  return review.skills.reduce((n, skill) => n + skill.summary.marked, 0);
}

/**
 * **Install all…, the one review** (#4024, decisions 3 to 7): where they land, chosen once at
 * the top; the pinned commit; then each skill with its box ticked, what it adds with the risky
 * items marked, and its files read when unfolded; the skills the chosen ones refer to come along,
 * marked, their boxes following the one that brought them. Unticking one reviews the rest again,
 * so Install installs exactly what the page shows, as one change. Past what one read of the
 * source may take, it says so and offers the first part.
 */
function InstallSetReviewPage({
  plane,
  source,
  named,
  levels,
  workspaces,
  start,
  busy,
  onCancel,
  onInstall,
}: {
  plane: string;
  source: SkillSource;
  /** The skills ticked on the list, in its order. */
  named: readonly string[];
  levels: readonly StudioLevel[];
  workspaces: readonly string[];
  start: SkillLevel;
  busy: boolean;
  onCancel: () => void;
  onInstall: (
    review: InstallReview,
    level: SkillLevel,
    grant: SkillGrant | null,
    renames: SkillRename[],
  ) => void;
}) {
  // Those still in this install: unticking one here leaves it out and reads the review again.
  const [names, setNames] = useState<readonly string[]>(named);
  const [level, setLevel] = useState<SkillLevel>(start);
  // Nothing is asked while nothing is ticked. Read again where they land changes: a skill one
  // refers to that the level holds already is left as it is (#4032).
  const { read, again } = useRead(
    names.length === 0
      ? undefined
      : `${sourceKey(source.remote)}\u0000${JSON.stringify(level)}\u0000${names.join("\u0000")}`,
    () => commands.skillInstallSetReview(plane, source, [...names], level),
  );
  const picker = picking(
    levels.map((one) => one.level),
    (one) => labelOf(levels, one),
    sameLevel,
    level,
  );
  const answer: InstallSetReview | undefined = read?.data;
  const review = answer?.kind === "review" ? answer.review : undefined;
  const reach = useInstallGrant(plane, level, review);
  const clashes = useClashes(review?.skills ?? [], (name) =>
    setNames(names.filter((one) => one !== name)),
  );
  // A skill left out that another brings along is in the review, and is not listed as left out.
  const leftOut = named.filter(
    (name) => !names.includes(name) && !review?.skills.some((one) => one.name === name),
  );
  const narrow = (to: readonly string[]) => setNames(named.filter((name) => to.includes(name)));
  const why =
    names.length === 0
      ? "Tick at least one skill."
      : read === undefined
        ? "The review is being read."
        : answer === undefined
          ? "The review could not be read."
          : review === undefined
            ? "It is more than one review can read: install it in two parts."
            : review.skills.length === 0
              ? `Every skill ticked is in the ${labelOf(levels, level)} skills already.`
              : busy
                ? BUSY
                : (clashes.why ?? reach.why);
  const risky = review === undefined ? 0 : riskyIn(review);
  return (
    <ReviewPage
      label={`Install all from ${source.label}`}
      data-testid="install-all-review"
      summary="Nothing is installed until you press Install."
      onCancel={onCancel}
      main={{
        children: "Install",
        byReturn: false,
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: () => {
          if (review !== undefined) onInstall(review, level, reach.grant, clashes.renames);
        },
      }}
    >
      <h2 className="ui-text-strong">
        {review !== undefined && review.skills.length === 0
          ? `Nothing to install from ${source.label}`
          : `Install ${counted(review?.skills.length ?? names.length, "skill", "skills")} from ${source.label}`}
      </h2>
      <SettingRow
        label="Lands in"
        grouped
        help={`They all land in the ${labelOf(levels, level)} skills. Move one afterwards from the Library.`}
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
      <InstallGrant
        label="Where they will reach"
        workspaces={workspaces}
        busy={busy}
        reach={reach}
      />
      {names.length === 0 ? (
        <EmptyState
          headline="Every skill is left out"
          body="Tick one below to review it, or go back to the list."
          variant="panel"
          testid="install-all-none"
        />
      ) : (
        <Held read={read} what="the review of these skills" again={again}>
          {(data) =>
            data.kind === "too-much" ? (
              <EmptyState
                headline="These skills are more than one review can read"
                body={
                  data.fits.length === 0
                    ? `${data.why}. Leave some out, and install the rest after.`
                    : `${data.why}. Install them in two parts: ${firstPart(data.fits.length)} now, then the other ${data.rest.length} with Install all… again.`
                }
                variant="panel"
                role="status"
                testid="install-all-too-much"
                action={
                  data.fits.length === 0 ? (
                    <Button size="sm" onPress={onCancel}>
                      Back to the list
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      variant="primary"
                      // Those there already stay named, so the next review says so again.
                      onPress={() => narrow([...data.fits, ...data.there])}
                    >
                      Review {firstPart(data.fits.length)}
                    </Button>
                  )
                }
              />
            ) : (
              <>
                <p className="honest" data-testid="install-commit">
                  You are reviewing {source.label} at commit <code>{data.review.commit.short}</code>
                  : {counted(data.review.skills.length, "skill", "skills")}
                  {risky > 0 && `, ${counted(risky, "risky item", "risky items")} marked`}. Untick a
                  skill to leave it out. What you install stays pinned to this commit. The review is
                  read from the files: a script can do more when it runs, and the chat&apos;s
                  sandbox bounds that. Installing copies all of them or none.
                </p>
                {data.review.skills.map((skill) => (
                  <SkillReviewed
                    key={skill.name}
                    skill={skill}
                    folded
                    widenings={reach.widenings}
                    clash={clashes.choice(skill, busy)}
                    pick={
                      <Checkbox
                        isSelected
                        isDisabled={skill.brought_by !== null || busy}
                        tooltip={
                          skill.brought_by !== null
                            ? `It comes along while ${skill.brought_by} is installed.`
                            : busy
                              ? BUSY
                              : undefined
                        }
                        onChange={() => narrow(names.filter((name) => name !== skill.name))}
                        data-testid={`keep-${skill.name}`}
                      >
                        Install {skill.name}
                        {skill.summary.marked > 0 &&
                          ` (${counted(skill.summary.marked, "risky item", "risky items")})`}
                      </Checkbox>
                    }
                  />
                ))}
                {data.there.length > 0 && (
                  <section aria-labelledby="install-all-there" className="skill-review">
                    <h3 id="install-all-there" className="ui-text-small ui-text-strong">
                      There already
                    </h3>
                    <p className="honest">
                      {data.there.length === 1
                        ? "Chats there have this one already, so it is left as it is."
                        : "Chats there have these already, so they are left as they are."}
                    </p>
                    {data.there.map((name) => (
                      <Checkbox
                        key={name}
                        isSelected={false}
                        isDisabled
                        tooltip={`${name} is there already.`}
                        data-testid={`there-${name}`}
                      >
                        Install {name}
                      </Checkbox>
                    ))}
                  </section>
                )}
              </>
            )
          }
        </Held>
      )}
      {leftOut.length > 0 && (
        <section aria-labelledby="install-all-left-out" className="skill-review">
          <h3 id="install-all-left-out" className="ui-text-small ui-text-strong">
            Left out of this install
          </h3>
          {leftOut.map((name) => (
            <Checkbox
              key={name}
              isSelected={false}
              isDisabled={busy}
              tooltip={busy ? BUSY : undefined}
              onChange={() => narrow([...names, name])}
              data-testid={`left-out-${name}`}
            >
              Install {name}
            </Checkbox>
          ))}
        </section>
      )}
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

/** What `grant` reaches, inside a sentence: "every workspace", "the root and ops". */
function reachSaid(grant: SkillGrant): string {
  if (grant.to === "only") return `the root and ${grant.workspaces.join(", ")}`;
  return grant.to === "all" ? "every workspace" : "the root alone";
}

/** Whether an update's review says where it reaches (#4186): a skill it brings along lands with
 *  its grant, or a project skill it relies on is widened or turned off somewhere it reaches. */
function reaches(update: UpdateReview): boolean {
  return (
    (update.grant !== null && update.brought.length > 0) ||
    update.widenings.some((one) => update.skill.already.includes(one.name))
  );
}

/** **The update review**: what changes, the source's diff, where it reaches, the edits made
 *  here since the install, then Accept update or Not now fixed at the foot. */
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
          : busy
            ? BUSY
            : undefined;
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
            {reaches(data) && (
              <section aria-labelledby="update-reach" className="skill-review">
                <h3 id="update-reach" className="ui-text-strong">
                  Where it reaches
                </h3>
                {data.grant !== null && data.brought.length > 0 && (
                  <p className="honest" data-testid="update-brought-grant">
                    {data.brought.length === 1
                      ? `${data.brought[0].name}, which comes along, reaches`
                      : "What comes along reaches"}{" "}
                    {reachSaid(data.grant)}, as {data.name} does.
                  </p>
                )}
                <Relies skill={data.skill} widenings={data.widenings} by="update" />
              </section>
            )}
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
              <SkillReviewed
                skill={data.skill}
                widenings={data.widenings}
                by="update"
                relies={!reaches(data)}
              />
            </Disclosure>
            {data.brought.map((skill) => (
              <SkillReviewed
                key={skill.name}
                skill={skill}
                widenings={data.widenings}
                by="update"
              />
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
