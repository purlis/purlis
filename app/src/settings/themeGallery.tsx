// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState, useSyncExternalStore, type KeyboardEvent } from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import {
  commands,
  type GalleryCard,
  type TheirsDrawn,
  type ThemeAt,
  type ThemeGallery,
  type WindowOption,
  type YourTheme,
} from "../bindings";
import { drawWhatIsInForce, THEME_FOLDERS_CHANGED } from "../Extensions";
import { listen } from "../here";
import { projectThemeChanged } from "../projectTheme";
import { useTabStop } from "../roving";
import { ask, putBack, putBackSoon, showWhileOn } from "../theme/hoverPreview";
import {
  BUILT_IN,
  CLASSIC,
  SYSTEM,
  moreContrastAsked,
  personAsked,
  personLook,
  systemIsLight,
  THEME_ALIASES,
  type Theme,
} from "../theme/theme";
import { Choice, SettingActions, type Badge, type RowIds } from "./components";
import { askThemeChat } from "./links";
import { useReadOnlySaid } from "../ReadOnlyLine";
import type { LiveSetting, SettingsGroup } from "./groups";
import { OptionsList } from "./themeOptions";
import { useOnScreen, useSpecimen } from "./themeSpecimens";
import { Button } from "../ui/button/Button";
import { Tooltip } from "../ui/tooltip/Tooltip";

/**
 * **Settings › Theme is a gallery** (spec #2313, #2331): a card per theme a choice at this level
 * may name, each with the theme specimen in each of its appearances; pointing at a card (or
 * moving to it with the arrow keys) draws it live in the window, and leaving puts the person's
 * theme back; pressing it chooses it. Under the cards, the chosen theme's appearances and
 * options, where each value comes from, the theme check's findings, and what a card does to
 * the person's own themes: **Duplicate as my theme** and **Open folder**.
 *
 * **The engine answers everything** (`theme_gallery`, `purlis_core::theme::gallery`): which
 * themes the level reaches, what each draws (`theme_card_drawn`), what the check found, and
 * what Duplicate makes. The window draws the cards and applies the theme it is handed.
 *
 * **The level is where the tab is**: You is the machine default, the picker that was missing;
 * Project and Workspace are the person's choice for them, on this machine. Each is written
 * where the theme levels read it (#2323): the machine default by `theme_choose_machine`, the
 * others by `theme_choose`, and the chosen theme's option values by `theme_choose_option`
 * (#2527), each value with a badge naming the level whose choice made it.
 *
 * **A theme folder's card names its level** (#2516): two folders of one name at two levels are
 * two cards, each saying where it comes from, told apart by the card's `key`; a choice names the
 * level of the folder it picked, so a folder of the name at another level is never drawn in its
 * place.
 *
 * **The keyboard**: the cards are one Tab stop, a listbox, moved through with the arrow keys
 * (`roving.ts`), the chosen card the stop at rest. Moving to a card draws it as pointing does;
 * Enter or Space chooses it. Nothing here takes the focus by itself, so nothing that writes is
 * ever where the focus lands.
 */
export const THEME_GROUP = {
  help: "Every theme you can choose here. Point at a card, or move to it with the arrow keys, to see it in the window; press it to choose it.",
  kept: "Kept on this machine, never in a project file.",
} satisfies Partial<SettingsGroup>;

/** **The Theme group's rows**, at the level `at` names (the machine's with `null`), under the
 *  group whose address is `group`. */
export function themeRows(group: string, at: ThemeAt | null): LiveSetting[] {
  return [
    galleryRow(group, at),
    appearanceRow(group, at),
    optionsRow(group, at),
    checkRow(group, at),
    folderRow(group, at),
  ];
}

// ---------------------------------------------------------------------------------------
// The gallery's answer, kept per level while a tab shows it
// ---------------------------------------------------------------------------------------

/** A level's gallery: the core's answer, `undefined` until it has said, or why it could not. */
type Read = ThemeGallery | undefined | { trouble: string };

const answers = new Map<string, Read>();
const listeners = new Set<() => void>();
const asking = new Set<string>();

/** Each level a row shows, by its key, to ask again when a theme folder changes. */
const places = new Map<string, ThemeAt | null>();

/** The key a level's gallery is kept by. */
function keyOf(at: ThemeAt | null): string {
  return at === null ? "" : `${at.plane}\u0000${at.workspace ?? ""}`;
}

/** The gallery's one listener for the theme folders changing, once it hears them: what stops
 *  it. */
let following: Promise<() => void> | undefined;

/** Which life of the galleries this is: an answer to an ask of an earlier one is dropped. */
let life = 0;

/**
 * **Every gallery shown is asked again when a theme folder changes on disk** (#2324): the core
 * tells the window `theme-folders-changed`, and the cards, their specimens and what is drawn are
 * read again as the core reads them. One listener for the life of the window.
 */
function followFolders(): void {
  if (following !== undefined) return;
  const heard = listen(THEME_FOLDERS_CHANGED, () => {
    for (const [key, at] of places) if (interest.has(key)) reread(at);
  });
  following = heard;
  heard.catch(() => {
    if (following === heard) following = undefined;
  });
}

function told() {
  for (const listener of listeners) listener();
}

/** Asks the core for the gallery at `at` again, and tells every row when it has said. */
function reread(at: ThemeAt | null): void {
  const key = keyOf(at);
  if (asking.has(key)) return;
  asking.add(key);
  const asked = life;
  void commands
    .themeGallery(at)
    .then(
      (answer) => {
        // Nothing shows this level any more, or the galleries were forgotten since: the answer
        // is not kept.
        if (asked !== life || !interest.has(key)) return;
        const given = answer.status === "ok" ? (answer.data as ThemeGallery | null) : null;
        answers.set(
          key,
          given !== null && typeof given === "object" && Array.isArray(given.cards)
            ? given
            : { trouble: answer.status === "error" ? answer.error : "purlis did not answer" },
        );
      },
      (why: unknown) => {
        if (asked === life && interest.has(key)) answers.set(key, { trouble: String(why) });
      },
    )
    .finally(() => {
      if (asked !== life) return;
      asking.delete(key);
      told();
    });
}

/** **The gallery at `at`**, asked when the first row shows it and again after each thing a row
 *  writes. */
function useGallery(at: ThemeAt | null): Read {
  const key = keyOf(at);
  const read = useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => answers.get(key),
  );
  useEffect(() => {
    followFolders();
    places.set(key, at);
    interest.set(key, (interest.get(key) ?? 0) + 1);
    if (!answers.has(key)) reread(at);
    return () => {
      const left = (interest.get(key) ?? 1) - 1;
      if (left > 0) {
        interest.set(key, left);
        return;
      }
      // No row shows it any more: the next one to show it asks again, so a gallery is never older
      // than the visit to the tab that shows it.
      interest.delete(key);
      answers.delete(key);
      places.delete(key);
    };
    // `key` is `at`, as a value.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return read;
}

/** How many rows show each level's gallery. */
const interest = new Map<string, number>();

/** For tests: forget every gallery read, and stop hearing the theme folders, so nothing one
 *  test started reaches the next. */
export function forgetGalleries(): void {
  life++;
  answers.clear();
  asking.clear();
  interest.clear();
  places.clear();
  drawnThemes.clear();
  void following?.then((stop) => stop()).catch(() => undefined);
  following = undefined;
}

/** Whether `read` is the core's answer. */
function answered(read: Read): read is ThemeGallery {
  return read !== undefined && !("trouble" in read);
}

/** The card a choice names: a card by its id, of the level the choice names (the nearest of
 *  the name when it names none, as the cards are nearest first); Purlis Classic by any id one of
 *  its appearances has had, and by the old `system` id while no theme of the person's own is
 *  drawn (#2615). */
function cardNamed(
  gallery: ThemeGallery,
  id: string | undefined | null,
  from: string | null = null,
): GalleryCard | undefined {
  if (id == null) return undefined;
  const found = gallery.cards.find(
    (one) => one.id === id && (from === null || one.level === null || one.level === from),
  );
  if (found !== undefined) return found;
  const known = Object.prototype.hasOwnProperty.call(THEME_ALIASES, id) || id in BUILT_IN;
  return known || (id === SYSTEM && gallery.theirs === null)
    ? gallery.cards.find((one) => one.id === CLASSIC)
    : undefined;
}

/** **The person's own `theme.json`, drawn**, as the gallery's line says it (#2615): with what
 *  following the system does with it. */
export function theirsSaid(theirs: TheirsDrawn): string {
  if (theirs.follows_system) return "your own theme.json, following the system";
  return theirs.note === null ? "your own theme.json" : `your own theme.json (${theirs.note})`;
}

/** What the rows under the cards say while the person's own `theme.json` is drawn: it is no
 *  card, so there is nothing of a card's to show. */
const THEIRS_NO_CARD =
  "Your own theme.json is drawn here, and it is no card: choose a theme above to pick its appearance.";

/** The card the person's own choice here names. */
function yoursCard(gallery: ThemeGallery): GalleryCard | undefined {
  return gallery.yours === null
    ? undefined
    : cardNamed(gallery, gallery.yours.theme, gallery.yours.from);
}

/** **The chosen card**: the person's choice here, else what the levels draw here, as the core
 *  says it. */
function chosenCard(gallery: ThemeGallery): GalleryCard | undefined {
  return gallery.cards.find((one) => one.key === gallery.chosen) ?? yoursCard(gallery);
}

/** Where what is drawn here comes from, as the gallery's badge says it. */
function levelBadge(gallery: ThemeGallery, card: GalleryCard | undefined): Badge {
  const name = card?.name ?? "the theme drawn";
  if (gallery.yours !== null)
    return {
      label: "Your choice",
      says: `${name} is your choice here, kept on this machine.`,
    };
  const words: Record<string, string> = {
    "workspace-choice": "Workspace",
    "workspace-default": "Workspace default",
    "project-choice": "Project",
    "project-default": "Team",
    "machine-default": "This machine",
    purlis: "Purlis",
  };
  return {
    label: words[gallery.level] ?? "Drawn here",
    says: `Nothing is chosen here, so ${gallery.from} decides: ${name}.`,
  };
}

/** What a card's source is called on it. */
function fromSaid(card: GalleryCard): string {
  switch (card.from) {
    case "built-in":
      return "Built in";
    case "machine":
      return "Yours, on this machine";
    case "project":
      return "This project's";
    case "workspace":
      return "This workspace's";
    default:
      return card.extension ? `From ${card.extension}` : "From an extension";
  }
}

/** An appearance as a person reads it: its id, with its tag when the id is not it. */
function appearanceSaid(one: GalleryCard["appearances"][number]): string {
  const tag = one.high_contrast ? `${one.appearance}, high contrast` : one.appearance;
  return one.id === one.appearance && !one.high_contrast ? one.id : `${one.id} (${tag})`;
}

// ---------------------------------------------------------------------------------------
// Choosing, writing where the level reads it
// ---------------------------------------------------------------------------------------

/** Writes the person's choice at `at`: the machine default with no project. `from` is the level
 *  of the theme folder it picks. */
async function choose(
  at: ThemeAt | null,
  theme: string | null,
  appearance: string | null,
  from: string | null,
): Promise<string | undefined> {
  const done =
    at === null
      ? await commands.themeChooseMachine(theme, appearance, from)
      : await commands.themeChoose(at.plane, at.workspace, theme, appearance, from);
  if (done.status === "error") return done.error;
  redraw(at);
  return undefined;
}

/** The window draws what a change at `at` chose, and the gallery is asked again. */
function redraw(at: ThemeAt | null): void {
  if (at === null) void drawWhatIsInForce();
  else projectThemeChanged(at.plane);
  reread(at);
}

/** Whether `yours` is a choice of `card`: its theme, of the level `card` is, where it names one. */
function isYours(card: GalleryCard, yours: YourTheme | null): boolean {
  return (
    yours !== null &&
    yours.theme === card.id &&
    (yours.from === null || card.level === null || yours.from === card.level)
  );
}

/** The appearance a new choice of `card` keeps: following the system, when the last choice
 *  did and `card` can; else its first. */
function appearanceFor(card: GalleryCard, yours: YourTheme | null): string | null {
  if (yours !== null && isYours(card, yours)) return yours.appearance;
  return yours?.appearance === SYSTEM && card.follows === null ? SYSTEM : null;
}

// ---------------------------------------------------------------------------------------
// What a card draws while it is pointed at
// ---------------------------------------------------------------------------------------

/** Each card's theme as the core answered it, by level, card and stamp. */
const drawnThemes = new Map<string, Theme | null>();

/** Draws `card` while it is pointed at or focused, in the appearance it would be chosen in.
 *  `cards` is the listbox: a preview is put back once it is out of the document. */
function enter(
  at: ThemeAt | null,
  card: GalleryCard,
  yours: YourTheme | null,
  cards: Element | null,
): void {
  const ticket = ask();
  if (card.refused) {
    putBackSoon();
    return;
  }
  const appearance = appearanceFor(card, yours) ?? SYSTEM;
  const light = systemIsLight();
  const more = moreContrastAsked();
  const values = isYours(card, yours) ? JSON.stringify(yours?.options ?? {}) : "";
  const key = `${keyOf(at)}\u0000${card.key}\u0000${card.stamp}\u0000${appearance}\u0000${values}\u0000${light}\u0000${more}\u0000${personLook()}`;
  const kept = drawnThemes.get(key);
  if (kept) {
    showWhileOn(kept, ticket, cards);
    return;
  }
  void commands
    .themeCardDrawn(at, card.id, card.level, appearance, light, more, personAsked())
    .then((answer) => {
      const theme =
        answer.status === "ok" && answer.data?.theme
          ? (answer.data.theme as unknown as Theme)
          : null;
      if (theme !== null) drawnThemes.set(key, theme);
      if (theme !== null) showWhileOn(theme, ticket, cards);
    })
    .catch(() => undefined);
}

// ---------------------------------------------------------------------------------------
// The rows
// ---------------------------------------------------------------------------------------

/** **The cards**: a listbox, one Tab stop, each card the theme's name, where it comes from,
 *  its pictures and its marks. */
function galleryRow(group: string, at: ThemeAt | null): LiveSetting {
  return {
    id: `${group}.gallery`,
    label: "Themes",
    help: "Point at a theme to see it in the window, and press it to make it yours here.",
    writes: "this machine",
    useControl: function useThemeCards() {
      const read = useGallery(at);
      const [error, setError] = useState<readonly string[]>();
      const [before, setBefore] = useState<{ was: YourTheme | null } | undefined>();
      // The tab going puts the person's theme back.
      useEffect(() => () => putBack(), []);
      const pick = (card: GalleryCard) => {
        if (!answered(read) || card.refused) return;
        const was = read.yours;
        putBack();
        void choose(at, card.id, appearanceFor(card, was), card.level).then((why) => {
          setError(why === undefined ? undefined : [why]);
          setBefore(why === undefined ? { was } : undefined);
        });
      };
      const yours = answered(read) ? read.yours : null;
      return {
        grouped: true,
        error,
        // Back to no choice here: the levels underneath decide, as the badge then says.
        reset: {
          label: "Use the default",
          disabled: yours === null,
          onReset: () => {
            const was = yours;
            putBack();
            void choose(at, null, null, null).then((why) => {
              setError(why === undefined ? undefined : [why]);
              setBefore(why === undefined ? { was } : undefined);
            });
          },
        },
        undo:
          before === undefined
            ? undefined
            : () => {
                const was = before.was;
                setBefore(undefined);
                void choose(
                  at,
                  was?.theme ?? null,
                  was?.appearance ?? null,
                  was?.from ?? null,
                ).then((why) => setError(why === undefined ? undefined : [why]));
              },
        control: (ids) => <Cards read={read} at={at} ids={ids} onPick={pick} />,
      };
    },
  };
}

function Cards({
  read,
  at,
  ids,
  onPick,
}: {
  read: Read;
  at: ThemeAt | null;
  ids: RowIds;
  onPick: (card: GalleryCard) => void;
}) {
  const gallery = answered(read) ? read : undefined;
  const chosen = gallery === undefined ? undefined : chosenCard(gallery);
  const list = useRef<HTMLDivElement>(null);
  const owner = useRef<{
    pointer: GalleryCard | null;
    focus: GalleryCard | null;
    aimed: string | null;
  }>({ pointer: null, focus: null, aimed: null });
  const stop = useTabStop(
    chosen?.key,
    (gallery?.cards ?? []).map((card) => card.key),
  );
  if (gallery === undefined)
    return (
      <p id={ids.id} className="ui-setting-help" role="status">
        {read !== undefined && "trouble" in read
          ? `purlis could not read the themes here: ${read.trouble}`
          : "Reading the themes…"}
      </p>
    );
  const keyed = (card: GalleryCard) => (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onPick(card);
  };
  // One preview for the pointer and the keyboard: the card the pointer is on, else the one
  // the focus is on; the person's theme goes back only when neither is on a card.
  const aim = () => {
    const on = owner.current.pointer ?? owner.current.focus;
    if (on?.key === owner.current.aimed) return;
    owner.current.aimed = on?.key ?? null;
    if (on === null) putBackSoon();
    else enter(at, on, gallery.yours, list.current);
  };
  const point = (card: GalleryCard | null) => {
    owner.current.pointer = card;
    aim();
  };
  const focus = (card: GalleryCard | null) => {
    owner.current.focus = card;
    aim();
  };
  const badge = levelBadge(gallery, chosen);
  return (
    <div className="flex flex-col gap-2">
      <p className="ui-setting-help">
        {gallery.yours !== null ? "Chosen here" : "Drawn here"}:{" "}
        {chosen?.name ??
          (gallery.theirs !== null ? theirsSaid(gallery.theirs) : "your own theme.json")}{" "}
        <SourceBadge badge={badge} />
      </p>
      <RovingFocusGroup.Root asChild loop {...stop}>
        <div
          ref={list}
          id={ids.id}
          role="listbox"
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
          className="grid grid-cols-2 gap-3"
          onPointerLeave={() => point(null)}
        >
          {gallery.cards.map((card) => {
            const isChosen = card.key === chosen?.key;
            const marks = [
              // A built-in theme that is not the default yet says so (#2336).
              card.preview ? "Preview" : undefined,
              isChosen ? (gallery.yours !== null ? "Chosen here" : "Drawn here") : undefined,
              card.refused
                ? "Refused"
                : card.warnings.length > 0
                  ? `${card.warnings.length} ${card.warnings.length === 1 ? "warning" : "warnings"}`
                  : undefined,
            ].filter((one): one is string => one !== undefined);
            return (
              <RovingFocusGroup.Item key={card.key} asChild tabStopId={card.key}>
                <div
                  role="option"
                  aria-selected={isChosen}
                  aria-disabled={card.refused || undefined}
                  aria-describedby={card.refused ? `${ids.id}-${card.key}-refused` : undefined}
                  data-theme-card={card.id}
                  data-theme-level={card.level ?? undefined}
                  className={
                    "flex flex-col gap-1 rounded-md border border-border-subtle p-2 " +
                    "hover:bg-surface-hover aria-selected:border-accent-base"
                  }
                  onPointerEnter={() => point(card)}
                  onFocus={() => focus(card)}
                  onBlur={() => focus(null)}
                  // A refused theme draws Purlis: there is nothing to choose.
                  onClick={card.refused ? undefined : () => onPick(card)}
                  onKeyDown={card.refused ? undefined : keyed(card)}
                >
                  <Pictures
                    at={at}
                    card={card}
                    yours={gallery.yours}
                    refusedId={`${ids.id}-${card.key}-refused`}
                  />
                  <span className="text-text-primary">{card.name}</span>
                  <span className="text-sm text-text-secondary">
                    {[fromSaid(card), ...marks].join(" · ")}
                  </span>
                </div>
              </RovingFocusGroup.Item>
            );
          })}
        </div>
      </RovingFocusGroup.Root>
    </div>
  );
}

/** Where a value comes from: the level's word, with its sentence as the badge's title and its
 *  accessible description, as the options' badges are. */
function SourceBadge({ badge }: { badge: Badge }) {
  return (
    <Tooltip tip={badge.says}>
      <span className="ui-setting-badge" aria-description={badge.says}>
        {badge.label}
      </span>
    </Tooltip>
  );
}

/** The most appearances a card shows a picture of; the rest are said. */
const MOST_PICTURES = 3;

/** A card's pictures: the specimen in each of its appearances, drawn with the option values
 *  the person chose for it here, as pointing at it draws it (#2527). */
function Pictures({
  at,
  card,
  yours,
  refusedId,
}: {
  at: ThemeAt | null;
  card: GalleryCard;
  /** The person's choice here: a card of its theme is pictured with its values. */
  yours: YourTheme | null;
  /** The id of the line saying why a refused card is refused: the card's description. */
  refusedId: string;
}) {
  // Only a card on screen is pictured (#2614): the rest are asked for as they scroll in.
  const [box, seen] = useOnScreen();
  if (card.refused)
    return (
      <p id={refusedId} className="text-sm text-text-secondary">
        {card.said[0] ?? "purlis refused it"}
      </p>
    );
  const shown = card.appearances.slice(0, MOST_PICTURES);
  const more = card.appearances.length - shown.length;
  // A picture is kept by what it is drawn with: the theme's stamp and the values chosen here.
  const values = isYours(card, yours) ? JSON.stringify(yours?.options ?? {}) : "";
  return (
    <div ref={box} className="flex gap-1">
      {shown.map((one) => (
        <Picture
          key={one.id}
          at={at}
          card={card}
          stamp={`${card.stamp}\u0000${values}`}
          appearance={one.id}
          seen={seen}
        />
      ))}
      {more > 0 && <span className="text-sm text-text-secondary">+{more}</span>}
    </div>
  );
}

function Picture({
  at,
  card,
  stamp,
  appearance,
  seen,
}: {
  at: ThemeAt | null;
  card: GalleryCard;
  /** What the picture is kept by. */
  stamp: string;
  appearance: string;
  /** Whether its card is on screen: a picture is asked for only then. */
  seen: boolean;
}) {
  const picture = useSpecimen(at, card.id, card.level, stamp, appearance, seen);
  // The card names the theme; a picture adds nothing a screen reader needs.
  if (picture.state === "drawn")
    return (
      <img
        src={picture.src}
        alt=""
        className="block aspect-video w-full min-w-0 flex-1 rounded-sm object-cover object-top"
      />
    );
  return (
    <Tooltip tip={picture.state === "none" ? `No picture: ${picture.why}` : undefined}>
      <span className="block aspect-video w-full min-w-0 flex-1 rounded-sm bg-surface-sunken" />
    </Tooltip>
  );
}

/** **The chosen theme's appearances**: following the system, or one by its id. */
function appearanceRow(group: string, at: ThemeAt | null): LiveSetting {
  return {
    id: `${group}.appearance`,
    label: "Appearance",
    help: "Which of the chosen theme's appearances is drawn: one of them, or follow the system.",
    writes: "this machine",
    useControl: function useThemeAppearance() {
      const read = useGallery(at);
      const [error, setError] = useState<readonly string[]>();
      const gallery = answered(read) ? read : undefined;
      const card = gallery === undefined ? undefined : chosenCard(gallery);
      const yours = gallery?.yours ?? null;
      // Only a choice made here has an appearance to change: with none, the levels underneath
      // decide, and picking an appearance would quietly choose the card drawn.
      const chosenHere = gallery !== undefined && card !== undefined && yoursCard(gallery) === card;
      const value = chosenHere ? (yours?.appearance ?? "") : "";
      return {
        grouped: true,
        error,
        control: (ids) =>
          card === undefined || card.refused ? (
            <p id={ids.id} className="ui-setting-help">
              {card !== undefined
                ? "The chosen theme is refused, so purlis draws its own in its place."
                : gallery?.theirs != null
                  ? THEIRS_NO_CARD
                  : "Reading the themes…"}
            </p>
          ) : (
            <div className="flex flex-col gap-1">
              {!chosenHere && (
                <p className="ui-setting-help" id={`${ids.id}-why`}>
                  {yours !== null && gallery !== undefined && yoursCard(gallery) === undefined
                    ? `Your choice here, ${yours.theme}, is no theme this level has, so the levels underneath decide: choose a theme above to pick its appearance.`
                    : "Nothing is chosen here: choose a theme above to pick its appearance."}
                </p>
              )}
              <Choice
                kind="radio"
                ids={
                  chosenHere
                    ? ids
                    : {
                        ...ids,
                        // Why the radios are off, said with them.
                        describedBy: [ids.describedBy, `${ids.id}-why`].filter(Boolean).join(" "),
                      }
                }
                disabled={!chosenHere}
                options={[
                  {
                    value: "",
                    label: "Its first",
                    says: card.appearances[0] ? appearanceSaid(card.appearances[0]) : undefined,
                  },
                  ...card.appearances.map((one) => ({ value: one.id, label: appearanceSaid(one) })),
                  {
                    value: SYSTEM,
                    label: "Follow the system",
                    says: card.follows ?? undefined,
                  },
                ]}
                value={value}
                onValueChange={(to) =>
                  void choose(at, card.id, to === "" ? null : to, card.level).then((why) =>
                    setError(why === undefined ? undefined : [why]),
                  )
                }
              />
            </div>
          ),
      };
    },
  };
}

/** **The chosen theme's options**: each value, choosable at this level (#2527), with a badge
 *  naming the level whose choice made it. */
function optionsRow(group: string, at: ThemeAt | null): LiveSetting {
  return {
    id: `${group}.options`,
    label: "Options",
    help: "The chosen theme's options: pick a value for each, kept here on this machine. The badge says what decided each value.",
    writes: "this machine",
    useControl: function useThemeOptions() {
      const read = useGallery(at);
      const [error, setError] = useState<readonly string[]>();
      const [said, setSaid] = useState<readonly string[]>();
      const [keeping, setKeeping] = useState(false);
      const gallery = answered(read) ? read : undefined;
      const card = gallery === undefined ? undefined : chosenCard(gallery);
      const options: readonly WindowOption[] | undefined =
        card === undefined ? undefined : card.refused ? [] : gallery?.options;
      const pick = (option: WindowOption, value: string | null) => {
        // Nothing to keep: the value is already the one chosen, or the default already drawn.
        if (value === null ? !option.chosen : option.chosen && value === option.value) return;
        setKeeping(true);
        void commands
          .themeChooseOption(at, option.id, value)
          .then((done) => {
            if (done.status === "error") {
              setError([done.error]);
              return;
            }
            setError(undefined);
            setSaid(done.data.length > 0 ? done.data : undefined);
            redraw(at);
          })
          .finally(() => setKeeping(false));
      };
      return {
        grouped: true,
        error,
        control: (ids) => (
          <div className="flex flex-col gap-1">
            <OptionsList
              options={options}
              ids={ids}
              none="The chosen theme has no options."
              onChoose={pick}
              disabled={keeping}
            />
            {said?.map((line) => (
              <p key={line} className="ui-setting-help" role="status">
                {line}
              </p>
            ))}
          </div>
        ),
      };
    },
  };
}

/** **The theme check's findings** for the chosen theme, as `purlis theme check` says them. */
function checkRow(group: string, at: ThemeAt | null): LiveSetting {
  return {
    id: `${group}.check`,
    label: "Theme check",
    help: "What the theme check found in the chosen theme, in every appearance and option value: the same as purlis theme check.",
    writes: "nothing",
    useControl: function useThemeCheck() {
      const read = useGallery(at);
      const gallery = answered(read) ? read : undefined;
      const card = gallery === undefined ? undefined : chosenCard(gallery);
      const lines =
        card === undefined
          ? undefined
          : [
              ...card.said.map((one) => (card.refused ? `Refused: ${one}` : `Left out: ${one}`)),
              ...card.floors,
              ...card.warnings,
            ];
      return {
        grouped: true,
        control: (ids) => (
          <div
            id={ids.id}
            role="group"
            className="ui-setting-status"
            aria-labelledby={ids.labelledBy}
            aria-describedby={ids.describedBy}
          >
            {lines === undefined ? (
              <p className="ui-setting-help">
                {gallery?.theirs != null ? THEIRS_NO_CARD : "Reading the themes…"}
              </p>
            ) : lines.length === 0 ? (
              <p className="ui-setting-help">
                No findings: {card?.name} can be read in every appearance.
              </p>
            ) : (
              <ul className="ui-setting-help">
                {lines.map((line, at) => (
                  <li key={at}>{line}</li>
                ))}
              </ul>
            )}
          </div>
        ),
      };
    },
  };
}

/**
 * What **Use in the whole project** does (#2708): the theme is the project's, and the project's
 * next save commits it for everyone, even from a workspace that is local.
 */
export const MOVE_TO_PROJECT_SAYS =
  "Moves this theme to the project's themes, for every workspace. The project's next save commits it for everyone, even from a local workspace.";

/** Why New theme and Edit with a chat wait for a project: a chat works in one. */
const NO_CHAT_HERE =
  "A chat makes a theme in a project: open Settings › Theme at the Project or Workspace level.";

/**
 * **What a card does to the person's own themes**: Duplicate as my theme, and Open folder; and
 * the moves up a level (#2332): **Use in the whole project** moves a workspace's theme to the
 * project, and **Copy to my machine** copies a project's or a workspace's theme to the person's
 * own, with only the font files approved with the project. **Edit with a chat** and **New theme**
 * (#2333) open a chat at this level on purlis's theme skill, its prompt typed and never sent.
 */
function folderRow(group: string, at: ThemeAt | null): LiveSetting {
  return {
    id: `${group}.folder`,
    label: "This theme",
    help: "Duplicate the chosen theme as a theme folder of your own that extends it, open its folder, have a chat edit it or make a new one, or move it up a level: a workspace's to the whole project, which the project's next save commits for everyone, a project's or a workspace's to your machine.",
    writes: "this machine",
    useControl: function useThemeFolder() {
      const read = useGallery(at);
      const gallery = answered(read) ? read : undefined;
      const card = gallery === undefined ? undefined : chosenCard(gallery);
      const [said, setSaid] = useState<string>();
      const [error, setError] = useState<readonly string[]>();
      const readOnly = useReadOnlySaid();
      const duplicate = () => {
        if (card === undefined) return;
        void commands.themeDuplicate(at, card.id, card.level).then((made) => {
          if (made.status === "error") {
            setError([made.error]);
            return;
          }
          setError(undefined);
          setSaid(`Made ${made.data}, a theme of your own that extends ${card.name}.`);
          reread(at);
        });
      };
      const open = () => {
        if (card === undefined) return;
        void commands
          .themeOpenFolder(at, card.id, card.level)
          .then((done) => setError(done.status === "error" ? [done.error] : undefined));
      };
      const moved = (done: { status: "ok"; data: string } | { status: "error"; error: string }) => {
        if (done.status === "error") {
          setError([done.error]);
          return;
        }
        setError(undefined);
        setSaid(done.data);
        reread(at);
        void drawWhatIsInForce();
      };
      const workspace = at?.workspace ?? null;
      // New theme and Edit with a chat (#2333): the project's window opens the chat here, its
      // prompt typed and never sent.
      // Why no theme chat opens here: no project at the machine's level, and a read-only one
      // could not keep the draft.
      const noChat =
        at === null
          ? NO_CHAT_HERE
          : readOnly === undefined
            ? undefined
            : `${readOnly} A theme made here could not be kept.`;
      const chat = (edited: GalleryCard | undefined) => {
        if (at === null || noChat !== undefined) return;
        askThemeChat({
          plane: at.plane,
          workspace,
          theme: edited?.id ?? null,
          from: edited?.level ?? null,
        });
      };
      const inWorkspace = card?.from === "workspace" && workspace !== null;
      const inProject = card?.from === "project" || inWorkspace;
      const toProject = () => {
        if (card === undefined || at === null || workspace === null) return;
        void commands.themeMoveToProject(at.plane, workspace, card.id).then(moved);
      };
      const toMachine = () => {
        if (card === undefined || at === null) return;
        void commands
          .themeCopyToMachine(at.plane, inWorkspace ? workspace : null, card.id)
          .then(moved);
      };
      return {
        grouped: true,
        error,
        control: (ids) => (
          <div
            id={ids.id}
            role="group"
            aria-labelledby={ids.labelledBy}
            className="flex flex-col gap-1"
          >
            <SettingActions>
              <Button
                isDisabled={card === undefined || !card.duplicable}
                tooltip={
                  card !== undefined && !card.duplicable
                    ? "A theme of your own extends the built-in theme or another of your own."
                    : undefined
                }
                onPress={duplicate}
              >
                Duplicate as my theme
              </Button>
              <Button
                isDisabled={card === undefined || !card.folder}
                tooltip={
                  card !== undefined && !card.folder
                    ? `${card.name} has no theme folder: it is ${card.from === "built-in" ? "built into purlis" : "an extension's"}.`
                    : undefined
                }
                onPress={open}
              >
                Open folder
              </Button>
              <Button
                isDisabled={card === undefined || noChat !== undefined}
                tooltip={noChat}
                onPress={() => chat(card)}
              >
                Edit with a chat
              </Button>
              <Button
                isDisabled={gallery === undefined || noChat !== undefined}
                tooltip={noChat}
                onPress={() => chat(undefined)}
              >
                New theme
              </Button>
              <Button
                isDisabled={!inWorkspace}
                tooltip={
                  card !== undefined && !inWorkspace
                    ? `${card.name} is not a theme of this workspace.`
                    : MOVE_TO_PROJECT_SAYS
                }
                onPress={toProject}
              >
                Use in the whole project
              </Button>
              <Button
                isDisabled={!inProject}
                tooltip={
                  card !== undefined && !inProject
                    ? `${card.name} is not a theme of this project.`
                    : undefined
                }
                onPress={toMachine}
              >
                Copy to my machine
              </Button>
            </SettingActions>
            {said !== undefined && (
              <p className="ui-setting-help" role="status">
                {said}
              </p>
            )}
          </div>
        ),
      };
    },
  };
}
