// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import * as Dialog from "./overlays/dialog";
import { ApproveExtension } from "./ApproveExtension";
import {
  commands,
  type ExtensionAsk,
  type ExtensionTheme,
  type InstalledExtensions,
  type ThemeReview,
  type ThemeTrouble,
  type WindowDraft,
  type WindowOption,
  type PlaneId,
} from "./bindings";
import { extensionsChanged } from "./extensionsOn";
import { listen } from "./here";
import { Choice, SettingActions } from "./settings/components";
import { plainLine } from "./settings/iconsPick";
import { iconThemeComplaints, offeredIconThemes, projectThemeChanged } from "./projectTheme";
import {
  BUILT_IN,
  CLASSIC,
  CLASSIC_THEME,
  crossfadeIn,
  drawIn,
  inForce,
  PREFERS_LIGHT,
  PREFERS_MORE_CONTRAST,
  moreContrastAsked,
  onPersonLook,
  personAsked,
  personLook,
  sameTheme,
  systemIsLight,
  THEME_ALIASES,
  type Theme,
} from "./theme/theme";
import {
  atCreation,
  forgetTheirTheme,
  sayAboutThisMachine,
  sayTheFloors,
  theirFloors,
  theirThemeOnce,
} from "./windowprefs";
import { drawNatively, forgetNativeWindow } from "./theme/native";
import { AnswerBar } from "./AnswerBar";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { yourEditor } from "./yourEditor";
import { setDraftHere } from "./themeDrafts";
import { Button } from "./ui/button/Button";

/**
 * What has contributed what to this window, and the question charter asks before anything new
 * contributes at all.
 *
 * **This is ADR 0041's registry, and the runtime is behind it, not in it.** An extension is a
 * directory the operator points at; charter reads its manifest, hashes its whole directory,
 * lists what it says it brings — each capability it asks for first (ADR 0053), then its themes,
 * panels, views and program — and puts none of it in force until it is approved. A declared
 * program is started by the core's executor when the operator opens one of its views, and
 * never from here: this dialog only reads, asks and records the answer. A manifest asking for a
 * capability this charter does not know is listed as refused, in the core's sentence naming it.
 *
 * **A built-in extension ships inside charter** (charter-app#339) and is trusted through the
 * app, so it has nothing to review. It is marked "built-in", and it is turned off on this
 * machine rather than removed — the app would bring a removed one back at the next read. A
 * project or a workspace turns it off as it turns off any extension (Project settings).
 *
 * **It is called an extension and not a plugin, everywhere, deliberately.** `enabledPlugins` in
 * the first-open dialog is Claude Code's plugin list, travelling in a project's committed
 * settings. Two things called "plugin" in two lists in one window is a consent surface that has
 * stopped being read (ADR 0041).
 *
 * **The honest sentence is not written here.** {@link ExtensionAsk.runs_as_you} comes out of
 * `purlis_core::extension::RUNS_AS_YOU` and is rendered as given. A dialog that composed its
 * own words about what an extension can reach would drift kinder than the truth one edit at a
 * time — and the truth is that a subprocess runs as the operator does, so what is listed is a
 * declaration and never a limit. A prompt that implied a cage would be worse than no prompt: it
 * would manufacture confidence charter cannot back.
 */
export function Extensions({ onClose }: { onClose: () => void }) {
  const [listed, setListed] = useState<InstalledExtensions | null>(null);
  const [asking, setAsking] = useState<ExtensionAsk | null>(null);
  const [went, setWent] = useState<string | null>(null);
  const [iconTrouble, setIconTrouble] = useState<ReadonlyMap<string, IconTrouble[]>>(new Map());

  // What each approved extension's icon theme got wrong (#1145), read again with the list: an
  // approval or a removal changes which icon themes there are.
  useEffect(() => {
    let gone = false;
    void offeredIconThemes().then((offered) => {
      if (!gone) setIconTrouble(iconTroubleOf(offered));
    });
    return () => {
      gone = true;
    };
  }, [listed]);

  const reread = useCallback(async () => {
    const answered = await commands.installedExtensions();
    if (answered.status === "error") {
      setWent(answered.error);
      return;
    }
    setWent(null);
    setListed(answered.data);
    // An approval or a removal changes what every project has on, and which themes there are.
    extensionsChanged();
    projectThemeChanged();
    await drawWhatIsInForce({ reread: true });
  }, []);

  // The first read, written as `Opener`'s is: the command's own promise, a `gone` flag, and
  // the state set inside the callback. Not `void reread()` — `react-hooks/set-state-in-effect`
  // reads a call in an effect body as synchronous however far the setState is from it, and the
  // shape that satisfies it is also the one that stops a dialog closed mid-read from setting
  // state on nothing.
  useEffect(() => {
    let gone = false;
    void commands
      .installedExtensions()
      .then((answered) => {
        if (gone) return;
        if (answered.status === "error") setWent(answered.error);
        else setListed(answered.data);
      })
      // A call that never reached the core is said, as its refusal is (#630): a dialog left
      // reading for ever says nothing true.
      .catch((why: unknown) => {
        if (!gone) setWent(`purlis could not read the installed extensions: ${String(why)}`);
      });
    void drawWhatIsInForce({ reread: true });
    return () => {
      gone = true;
    };
  }, []);

  const add = async () => {
    const picked = await commands.pickExtension();
    if (picked.status === "error") return setWent(picked.error);
    if (picked.data === null) return;
    const read = await commands.installExtension(picked.data);
    if (read.status === "error") return setWent(read.error);
    // The core wrote it into the record, unapproved: listed as it is now, whatever the answer.
    // One that contributes only themes is a set of theme folders, and asks nothing (ADR 0098).
    await reread();
    if (read.data !== null) setAsking(read.data);
  };

  const approve = async (ask: ExtensionAsk) => {
    // The fingerprint that was on screen, handed straight back. What is recorded is what was
    // read out loud, so a write between the drawing and the click is not consented to — it
    // makes the next launch ask again (charter-app#123's shape).
    const said = await commands.approveExtension(ask.id, ask.path, ask.fingerprint);
    setAsking(null);
    if (said.status === "error") return setWent(said.error);
    await reread();
  };

  const forget = async (id: string) => {
    const said = await commands.forgetExtension(id);
    if (said.status === "error") return setWent(said.error);
    await reread();
  };

  /** The record, in your editor (#1296): the core opens this machine's own record and no
   *  other file, so the window names the editor and nothing else. */
  const openRecord = async () => {
    const editor = yourEditor();
    if (editor === undefined)
      return setWent("Choose your editor in Settings › You › Editor first, then open it here.");
    const said = await commands.openExtensionRecord(editor);
    setWent(said.status === "error" ? said.error : null);
  };

  const turn = async (id: string, on: boolean) => {
    const said = await commands.setExtensionOn(id, on);
    if (said.status === "error") return setWent(said.error);
    await reread();
  };

  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="ui-scrim" />
        <Dialog.Content className="ui-dialog warning extensions" aria-labelledby="extensions">
          <Dialog.Title id="extensions">Extensions</Dialog.Title>
          {/* `tabIndex={0}` on every button in this dialog, per `docs/ui-primitives.md`
              (charter-app#186), and this is the surface where it mattered most. Its controls
              are `Add an extension…`, then a `Review` and a `Remove` per installed row, then
              `Done` — so Radix's focus scope covered the first and the last, and every row's
              two buttons sat in the middle, on an engine that will not tab to a `<button>`
              whose `tabindex` is not written down. Reviewing or removing an extension was a
              mouse-only act, on the one surface in the window that is about trust. */}
          <Button onPress={() => void add()}>Add an extension…</Button>

          {/* What went wrong, drawn as trouble (#630). Not yet a live region: `Notice.guard`
              counts every one by file, and that guard is another lane's this round. */}
          {went && <p className="trouble">{went}</p>}

          {listed === null && !went && <p className="pending">Reading the installed extensions…</p>}

          {listed?.unreadable && (
            <>
              <p className="came-back">
                purlis could not read this machine&rsquo;s extension record, so nothing an extension
                declares is in force: {listed.unreadable}
              </p>
              {/* The sentence names the file; once it is mended, one press reads it again
                  rather than closing and reopening the dialog (NO-8, #1233). Approving writes
                  nothing over a record purlis could not read, so reading is the only fix, and
                  Open in your editor is where it is mended (#1296). A row of their own under
                  the sentence, not two buttons inside it (#1719). */}
              <SettingActions>
                <Button onPress={() => void reread()}>Read again</Button>
                <Button onPress={() => void openRecord()}>Open in your editor</Button>
              </SettingActions>
            </>
          )}

          <h3>purlis&rsquo;s own themes</h3>
          <ul className="contributes">
            {(listed?.built_in_themes ?? []).map((name) => (
              <li key={name}>
                <code>{name}</code>
              </li>
            ))}
          </ul>

          <h3>Installed</h3>
          {/* A claim, then the way out as the window names it (`docs/ui-copy.md`, #630). */}
          {listed && listed.extensions.length === 0 && (
            <EmptyState
              variant="panel"
              headline="No extensions installed yet"
              body="Add an extension… points purlis at the directory that holds one."
            />
          )}
          <ul className="extension-rows">
            {(listed?.extensions ?? []).map((row) => (
              <li key={row.id} data-standing={row.standing} data-source={row.source}>
                <span className="name" id={`name-${row.id}`}>
                  {row.name}
                </span>
                {row.source === "app" && <span className="built-in">built-in</span>}
                <code className="where">{row.path}</code>
                <span className="standing">
                  {row.on
                    ? standingReads(row.standing)
                    : "off on this machine — contributing nothing"}
                </span>
                {/* Said as it arrives (#630): a row's trouble is read out, not only drawn. */}
                {row.refused && (
                  <span className="came-back" role="status">
                    {row.refused}
                  </span>
                )}
                {row.themes_in_force.length > 0 && (
                  <span className="in-force">Drawing: {row.themes_in_force.join(", ")}</span>
                )}
                {/* Why the theme engine refused each colour theme, or what it left out (#2321). */}
                {row.theme_trouble.map((trouble) => (
                  <span key={trouble.name} className="came-back theme-trouble" role="status">
                    {themeTroubleSaid(trouble)}
                  </span>
                ))}
                {(iconTrouble.get(row.id) ?? []).map((trouble) => (
                  <span key={trouble.name} className="came-back icon-trouble" role="status">
                    {troubleSaid(trouble)}
                  </span>
                ))}
                {row.ask && <Button onPress={() => setAsking(row.ask)}>Review</Button>}
                {row.source === "app" ? (
                  // On or off the moment it is turned: the settings set's switch (#1860), named
                  // by the extension and the words beside it, which turn it too.
                  <span className="extension-on">
                    <Choice
                      ids={{
                        id: `on-${row.id}`,
                        labelledBy: `name-${row.id} on-${row.id}-said`,
                      }}
                      kind="switch"
                      checked={row.on}
                      onCheckedChange={(on) => void turn(row.id, on)}
                    />
                    <label htmlFor={`on-${row.id}`} id={`on-${row.id}-said`}>
                      On this machine
                    </label>
                  </span>
                ) : (
                  <Button onPress={() => void forget(row.id)}>Remove</Button>
                )}
              </li>
            ))}
          </ul>

          {(listed?.dropped ?? []).length > 0 && (
            <>
              <h3>Dropped from the record</h3>
              <ul className="contributes">
                {listed?.dropped.map((why) => (
                  <li key={why}>{why}</li>
                ))}
              </ul>
            </>
          )}

          {asking && (
            <ApproveExtension
              ask={asking}
              onApprove={(ask) => void approve(ask)}
              onCancel={() => setAsking(null)}
            />
          )}

          <AnswerBar>
            <Button onPress={onClose}>Done</Button>
          </AnswerBar>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/** One contributed icon theme that got something wrong, and what. */
type IconTrouble = { name: string; complaints: readonly string[] };

/** The most complaints said about one icon theme, and the longest one said whole: a theme
 *  that is wrong everywhere is said to be in a line, not a page. */
const MOST_COMPLAINTS = 3;
const LONGEST_COMPLAINT = 160;

/** Each extension's icon themes that got something wrong, by the extension's id. */
function iconTroubleOf(offered: readonly ExtensionTheme[]): Map<string, IconTrouble[]> {
  const by = new Map<string, IconTrouble[]>();
  for (const one of offered) {
    const complaints = iconThemeComplaints(one.text);
    if (complaints.length === 0) continue;
    by.set(one.extension, [...(by.get(one.extension) ?? []), { name: one.name, complaints }]);
  }
  return by;
}

/** At most {@link MOST_COMPLAINTS} complaints, each cut to a line, and how many more. */
function complaintsSaid(complaints: readonly string[]): string {
  const cut = (text: string) => plainLine(text, LONGEST_COMPLAINT);
  const said = complaints.slice(0, MOST_COMPLAINTS).map(cut).join("; ");
  const more = complaints.length - MOST_COMPLAINTS;
  return `${said}${more > 0 ? `; and ${more} more` : ""}`;
}

/** One icon theme's complaints as a sentence, bounded: its text is the extension's. */
export function troubleSaid({ name, complaints }: IconTrouble): string {
  return `Icon theme “${plainLine(name)}”, drawn as purlis’s own where it is wrong: ${complaintsSaid(complaints)}.`;
}

/**
 * One colour theme's trouble as a sentence, bounded as an icon theme's is: why the theme engine
 * refused it, so Purlis is drawn in its place, or what it left out of a theme it draws. The
 * words are the engine's; `purlis theme check` says them in full.
 */
export function themeTroubleSaid({ name, complaints, refused }: ThemeTrouble): string {
  const theme = `Theme “${plainLine(name)}”`;
  if (refused) return `${theme} is not drawn, and Purlis is: ${complaintsSaid(complaints)}.`;
  return `${theme} is drawn without what purlis left out: ${complaintsSaid(complaints)}.`;
}

/** What a standing says, in the words to put in front of the operator. */
function standingReads(standing: string): string {
  if (standing === "approved") return "approved";
  if (standing === "themes") return "themes only — needs no approval";
  if (standing === "changed") return "changed since you approved it — contributing nothing";
  return "not approved — contributing nothing";
}

/** Whose themes may be drawn: the project in front's extensions, or every approved one when no
 *  project is in front (charter-app#253, ADR 0048). */
let allowed: ReadonlySet<string> | "every" = "every";
/** What the project in front picked (charter-app#273), as its file holds it — `purlis-dark`,
 *  `purlis-light` (or an id they had before), `system` or `<extension>/<theme>` — or `null`
 *  when it picked nothing. */
let picked: string | null = null;
/** The project and workspace in front (#2323), whose theme levels and theme folders the engine
 *  reads; `null` with no project in front. A workspace of `null` is the plane root. */
let where: { plane: PlaneId; workspace: string | null } | null = null;
/** Whether the next theme drawn is a switch to another workspace's, which crossfades. */
let switching = false;
/** The projects and workspaces whose in-plane picks were mapped into the person's choices
 *  this launch, so each is asked once. */
const adopted = new Set<string>();
/** Whether the core is to survey the extensions' themes again: an approval or a removal changes
 *  which there are. */
let surveyAgain = false;
/** Bumped by every draw: a draw that waited on the core and was overtaken draws nothing. */
let drawing = 0;

/**
 * Draws the theme the project in front has — `on` is what that project has on
 * (`extensionsOn.ts`, the core's `extension::project::resolve`), or `"every"` with no project in
 * front; `pick` is what it picked (`projectTheme.ts`, the core's `extension::project::theme`).
 */
export function drawThemeFor(
  on: ReadonlySet<string> | "every",
  pick: string | null = null,
  at?: { plane: PlaneId; workspace?: string },
) {
  const next = at === undefined ? null : { plane: at.plane, workspace: at.workspace ?? null };
  // A switch to another workspace (or project) repaints with a short crossfade; the first draw
  // of the launch is at once.
  if (where !== null && next !== null)
    switching ||= where.plane !== next.plane || where.workspace !== next.workspace;
  allowed = on;
  picked = pick;
  where = next;
  if (next !== null) adoptOnce(next.plane, next.workspace);
  return drawWhatIsInForce();
}

/**
 * **The options of the theme the window draws** (#2325), each with the value it is drawn with
 * and whether a choice made it: the core's theme engine answers (`theme_in_force`), for what
 * the project in front picked and has on. Settings › Theme shows them; nothing is drawn.
 * An answer the core cannot give is no options.
 */
export async function themeOptionsInForce(): Promise<readonly WindowOption[]> {
  const on = allowed === "every" ? null : [...allowed];
  try {
    const answered = await commands.themeInForce(
      picked,
      on,
      systemIsLight(),
      moreContrastAsked(),
      false,
      where,
      null,
      personAsked(),
    );
    if (answered.status !== "ok") return [];
    const given = answered.data as typeof answered.data | null;
    return Array.isArray(given?.options) ? given.options : [];
  } catch {
    return [];
  }
}

/**
 * **Maps today's in-plane theme picks into the person's choices once** (#2323): the project's
 * local file's pick and a LOCAL workspace's become the person's own, kept on this machine, so they
 * survive a re-clone. The core copies only where nothing is kept yet, so it says something at most
 * once; what it says is a Notice.
 */
function adoptOnce(plane: PlaneId, workspace: string | null) {
  const key = `${plane}\u0000${workspace ?? ""}`;
  if (adopted.has(key)) return;
  adopted.add(key);
  void commands
    .themeLevelsAdopt(plane, workspace)
    .then((said) => {
      if (said.status !== "ok" || !Array.isArray(said.data) || said.data.length === 0) return;
      sayAboutThisMachine("theme choice", {
        severity: "warn",
        detail: said.data.join(" "),
        remedy: "change it in Settings › Theme, which says where the theme drawn comes from",
      });
      projectThemeChanged(plane);
    })
    .catch(() => undefined);
}

/** For tests: forget what was asked and for whom. */
export function forgetExtensionThemes() {
  allowed = "every";
  picked = null;
  where = null;
  switching = false;
  adopted.clear();
  surveyAgain = false;
  following = false;
  unfollowSystem?.();
  unfollowSystem = undefined;
  unfollowContrast?.();
  unfollowContrast = undefined;
  unfollowLook?.();
  unfollowLook = undefined;
  unfollowFolders?.();
  unfollowFolders = undefined;
  forgetTheirTheme();
  forgetNativeWindow();
}

/** How to stop following the system's appearance, while it is followed. */
let unfollowSystem: (() => void) | undefined;

/**
 * Asks again when the operating system turns light or dark, while the theme in force follows it.
 * One listener for the life of the window, started by the first theme that follows it.
 */
function followSystem() {
  if (unfollowSystem !== undefined || typeof matchMedia !== "function") return;
  const query = matchMedia(PREFERS_LIGHT);
  const changed = () => {
    if (following) void drawWhatIsInForce();
  };
  query.addEventListener("change", changed);
  unfollowSystem = () => query.removeEventListener("change", changed);
}

/** How to stop following the system's contrast. */
let unfollowContrast: (() => void) | undefined;

/**
 * **Draws again when the operating system asks for more contrast, or stops asking** (#2328),
 * whatever the theme in force: one that has an appearance tagged high contrast draws it. One
 * listener for the life of the window, started by the first draw.
 */
function followContrast() {
  if (unfollowContrast !== undefined || typeof matchMedia !== "function") return;
  const query = matchMedia(PREFERS_MORE_CONTRAST);
  const changed = () => void drawWhatIsInForce();
  query.addEventListener("change", changed);
  unfollowContrast = () => query.removeEventListener("change", changed);
}

/** The event the core sends this window when a theme folder it draws from changed on disk
 *  (`themewatch.rs`, #2324). */
export const THEME_FOLDERS_CHANGED = "theme-folders-changed";

/** How to stop hearing the theme folders change. */
let unfollowFolders: (() => void) | undefined;

/**
 * **Draws again when a theme folder the window draws from changes on disk** (#2324): the core
 * watches the folders `theme_in_force` reads, and says when one changed; the window asks again,
 * the one way it ever asks, so the floors and the font approvals hold on a reload as at launch.
 * What a level chooses may name a folder just made, so the picks are asked again too. One
 * listener for the life of the window, started by the first draw.
 */
function followThemeFolders() {
  if (unfollowFolders !== undefined) return;
  let gone = false;
  const unfollow = () => {
    gone = true;
    void stop.then((off) => off?.()).catch(() => undefined);
  };
  const stop = listen<{ survey?: unknown }>(THEME_FOLDERS_CHANGED, (said) => {
    if (gone) return;
    projectThemeChanged();
    void drawWhatIsInForce({ reread: said.payload?.survey === true });
  }).catch(() => {
    // Not heard this time: the next draw listens again.
    if (unfollowFolders === unfollow) unfollowFolders = undefined;
    return undefined;
  });
  unfollowFolders = unfollow;
}

/**
 * Says, or takes back, that the theme chosen is not drawn and why (#2324): the engine refused
 * it — a save that is not a theme, a value out of its bounds, or a project's font file that
 * changed since the project was approved — and Purlis is drawn in its place.
 *
 * **When the engine says the project's approval would let it be drawn** (`review`, #2647), the
 * Notice offers Review and allow for the project in front — its trust ask, asked again in place
 * — only when what the project asks for again is font files alone (`fonts`). When it asks for
 * more (`more`), or this machine never approved it (`unapproved`), the Notice offers no button
 * and says the project is reviewed where it is opened.
 */
function sayTheRefused(refused: ThemeTrouble | null | undefined, review: ThemeReview | null) {
  if (refused == null) {
    sayAboutThisMachine("theme refused", undefined);
    return;
  }
  const plane = review === "fonts" ? where?.plane : undefined;
  sayAboutThisMachine("theme refused", {
    severity: "warn",
    detail: themeTroubleSaid(refused),
    remedy:
      plane !== undefined
        ? "review the font files that changed and allow them, and the theme is drawn with them; or fix it in its theme folder"
        : review === "more"
          ? "this project asks for more than font files since you approved it, so it is not allowed from a theme; close the project and open it again to review everything it asks for, or fix it in its theme folder"
          : review === "unapproved"
            ? "this project has not been approved on this machine, so its font files are not allowed from a theme; close the project and open it again to review everything it asks for, or fix it in its theme folder"
            : "fix it in its theme folder, and it is drawn again when you save; a project's font file is drawn once you approve the project again",
    ...(plane === undefined ? {} : { review: plane }),
  });
}

/** Whether the theme in force follows the system's appearance, as the core last said. */
let following = false;

/**
 * **What a project switch draws without asking**: a built-in the project picked (by its id or
 * an id it had before), or the person's own theme where the core said no level of the project in
 * front chooses — both resolved by the core and handed to the window as it was created, and both
 * what the engine answers for those picks (`purlis_core::theme::resolve_levels`). So the switch
 * repaints in the frame it happens in, with no frame of the last project's theme while the core
 * is asked. Anything else is the core's to say: with no project in front, the person's machine
 * default may choose a theme folder (#2602), so the core is asked.
 */
function handedFor(pick: string | null): Theme | undefined {
  // What was handed was drawn with the person's colours and contrast as the window was created:
  // once Settings changed them, the core is asked (#2925).
  if (personLook() !== lookHanded) return undefined;
  const has = (table: object, key: string) => Object.prototype.hasOwnProperty.call(table, key);
  if (pick === null) return where === null ? undefined : theirThemeOnce();
  if (pick === CLASSIC) return CLASSIC_THEME;
  const id = has(THEME_ALIASES, pick) ? THEME_ALIASES[pick] : pick;
  return has(BUILT_IN, id) ? BUILT_IN[id] : undefined;
}

/** The person's look the window was handed its themes in: what {@link handedFor} draws. */
const lookHanded = personLook();

/** How to stop following the person's look, while it is followed. */
let unfollowLook: (() => void) | undefined;

/** **Draws again when Settings changes the person's colours or contrast** (#2925): the engine
 *  draws them into the theme it answers with. One follower for the life of the window, started
 *  by the first draw. */
function followLook() {
  unfollowLook ??= onPersonLook(() => void drawWhatIsInForce());
}

/** Says, or takes back, which of the person's colours the theme in force leaves out, and why
 *  (#2925): each would put a pair under its floor in this theme. */
function sayYourColours(mine: readonly string[] | undefined) {
  sayAboutThisMachine(
    "your colours",
    mine !== undefined && mine.length > 0
      ? {
          severity: "warn",
          detail: mine.join("; "),
          remedy: "pick another colour in Settings, or clear it to draw the theme's",
          settings: "you.colours",
        }
      : undefined,
  );
}

/** Puts `theme` in force, unless it is already what is drawn — so the terminal is told only of
 *  a change — and draws the native window in it (`theme/native.ts`), which is told only of a
 *  change too. */
function draw(theme: Theme) {
  const crossfade = switching;
  switching = false;
  // The native window is told even when the page draws nothing new: whether it follows the
  // system may have changed, and it is told only of a change.
  drawNatively(theme, following);
  if (sameTheme(theme, inForce())) return;
  if (crossfade) crossfadeIn(theme);
  else drawIn(theme);
}

/**
 * Puts the theme the project in front has on the document: **the core's theme engine decides
 * it** (`theme_in_force`, `purlis_core::theme::resolve`) and the window only applies it.
 *
 * **After the first frame, never before it.** `main.tsx` draws what the window was handed as it
 * was created, so nothing is asked on the way to the first paint (ADR 0026's 2 s cold start).
 * What a project picked, and an extension's theme, which is a disk read and a fingerprint of
 * every file the extension declares, land afterwards — when the window knows which project is in
 * front (`App.tsx`). The visible cost is one repaint for an operator whose project picked a
 * theme, and the alternative is a slower launch for everybody.
 *
 * The order is the engine's: the project's pick, else the person's own `theme.json`, else the
 * first theme from an extension the project has on, else purlis's own. A built-in pick and the
 * person's own theme are drawn at once from what the window was handed ({@link handedFor}); the
 * core is asked for the rest. Nothing here throws; a
 * theme the core cannot answer keeps the window as it is.
 */
export async function drawWhatIsInForce({ reread = false } = {}): Promise<void> {
  const mine = ++drawing;
  followContrast();
  followLook();
  followThemeFolders();
  if (reread) surveyAgain = true;
  const handed = handedFor(picked);
  if (handed !== undefined) {
    following = false;
    sayTheRefused(null, null);
    // The person's own theme was drawn with the floors held as the window was created; a
    // built-in clears them.
    const theirs = atCreation().theme;
    sayTheFloors(picked === null ? theirFloors(theirs) : null, {
      theirs: theirs.path || undefined,
    });
    draw(handed);
    // A chat's theme draft where the window is draws in place of the pick (#2332): only the
    // core knows of one, so it is asked after this frame, and draws only when there is one.
    if (where !== null) void drawADraftOver(mine);
    else sayTheDraft(null);
    return;
  }
  const again = surveyAgain;
  const on = allowed === "every" ? null : [...allowed];
  let answered: Awaited<ReturnType<typeof commands.themeInForce>>;
  try {
    answered = await commands.themeInForce(
      picked,
      on,
      systemIsLight(),
      moreContrastAsked(),
      again,
      where,
      null,
      personAsked(),
    );
  } catch {
    return;
  }
  // An answer that is not one (a window outside the app, a test's core) keeps what is drawn.
  const given = answered.status === "ok" ? (answered.data as typeof answered.data | null) : null;
  if (given === null || typeof given !== "object") return;
  if (again) surveyAgain = false;
  if (mine !== drawing) return;
  sayTheDraft(given.draft);
  following = given.follows_system === true;
  if (following) followSystem();
  sayTheRefused(given.refused, given.review ?? null);
  sayYourColours(Array.isArray(given.mine) ? given.mine : undefined);
  if (given.theme) {
    const theme = given.theme as unknown as Theme;
    // The person's own theme file, drawn (no level chose): its way out is Use built-in.
    const mine = theirThemeOnce();
    const path = atCreation().theme.path;
    const theirs = mine !== undefined && sameTheme(theme, mine) && path ? path : undefined;
    sayTheFloors(typeof given.floors === "string" ? given.floors : null, { theirs });
    draw(theme);
  }
}

/** What the core said of the theme draft where the window is (#2332), for its strip. */
function sayTheDraft(draft: WindowDraft | null | undefined) {
  const here = where !== null && draft != null && typeof draft === "object" ? draft : null;
  setDraftHere(
    here === null || where === null
      ? null
      : { plane: where.plane, workspace: where.workspace, draft: here },
  );
  sayTheDraftRefused(here);
}

/**
 * **A draft the engine refuses is said as a refused theme is** (#2332, #2647): in a Notice, with
 * Review and allow only by the rules of any refused theme. The core says `fonts` only where every
 * font file the draft waits on is one the project brings where the draft is kept to, and the
 * project asks again about font files alone: its trust ask is then what lifts the refusal.
 */
function sayTheDraftRefused(draft: WindowDraft | null) {
  if (draft === null || !Array.isArray(draft.refused) || draft.refused.length === 0) {
    sayAboutThisMachine("theme draft refused", undefined);
    return;
  }
  const plane = draft.review === "fonts" ? where?.plane : undefined;
  sayAboutThisMachine("theme draft refused", {
    severity: "warn",
    detail: `Theme draft “${plainLine(draft.folder)}” is not drawn: ${complaintsSaid(draft.refused)}.`,
    remedy:
      plane !== undefined
        ? "Review and allow approves the project's own font files, the same bytes the draft carries, and the draft is drawn with them; or fix the draft and save it again"
        : "fix the draft and save it again; a font file of the project's is drawn once you approve the project again",
    ...(plane === undefined ? {} : { review: plane }),
  });
}

/**
 * **A draft over a pick the window drew at once** (#2332): the core is asked as it is for every
 * other theme, and only an answer that names a draft it draws is drawn. Anything else keeps the
 * pick, which is what the core would have answered.
 */
async function drawADraftOver(mine: number): Promise<void> {
  const on = allowed === "every" ? null : [...allowed];
  let answered: Awaited<ReturnType<typeof commands.themeInForce>>;
  try {
    answered = await commands.themeInForce(
      picked,
      on,
      systemIsLight(),
      moreContrastAsked(),
      false,
      where,
      null,
      personAsked(),
    );
  } catch {
    return;
  }
  const given = answered.status === "ok" ? (answered.data as typeof answered.data | null) : null;
  if (mine !== drawing) return;
  if (given === null || typeof given !== "object") return;
  sayTheDraft(given.draft);
  if (given.draft?.drawn !== true || !given.theme) return;
  following = given.follows_system === true;
  if (following) followSystem();
  sayTheFloors(typeof given.floors === "string" ? given.floors : null, {});
  draw(given.theme as unknown as Theme);
}

/**
 * **Compare** (#2332): while it is held the window draws the theme as kept, without the draft;
 * let go, and it draws what is in force again. Asked of the core the one way a theme is asked.
 */
export async function compareWithKept(held: boolean): Promise<void> {
  if (!held) {
    await drawWhatIsInForce();
    return;
  }
  const mine = ++drawing;
  const on = allowed === "every" ? null : [...allowed];
  let answered: Awaited<ReturnType<typeof commands.themeInForce>>;
  try {
    answered = await commands.themeInForce(
      picked,
      on,
      systemIsLight(),
      moreContrastAsked(),
      false,
      where,
      true,
      personAsked(),
    );
  } catch {
    return;
  }
  const given = answered.status === "ok" ? (answered.data as typeof answered.data | null) : null;
  if (mine !== drawing || given === null || typeof given !== "object" || !given.theme) return;
  draw(given.theme as unknown as Theme);
}
