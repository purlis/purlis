// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, useSyncExternalStore, type KeyboardEvent } from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import {
  commands,
  type GalleryCard,
  type ThemeAt,
  type ThemeGallery,
  type WindowOption,
  type YourTheme,
} from "../bindings";
import { drawWhatIsInForce } from "../Extensions";
import { projectThemeChanged } from "../projectTheme";
import { useTabStop } from "../roving";
import { putBack, putBackSoon, showWhileOn } from "../theme/hoverPreview";
import {
  BUILT_IN,
  CLASSIC,
  SYSTEM,
  systemAsksMoreContrast,
  systemIsLight,
  THEME_ALIASES,
  type Theme,
} from "../theme/theme";
import { Choice, SettingActions, type Badge, type RowIds } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";
import { OptionsList, useOptionsInForce } from "./themeOptions";
import { useSpecimen } from "./themeSpecimens";

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
 * others by `theme_choose`.
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

/** The key a level's gallery is kept by. */
function keyOf(at: ThemeAt | null): string {
  return at === null ? "" : `${at.plane}\u0000${at.workspace ?? ""}`;
}

function told() {
  for (const listener of listeners) listener();
}

/** Asks the core for the gallery at `at` again, and tells every row when it has said. */
function reread(at: ThemeAt | null): void {
  const key = keyOf(at);
  if (asking.has(key)) return;
  asking.add(key);
  void commands
    .themeGallery(at)
    .then(
      (answer) => {
        // Nothing shows this level any more: the answer is not kept.
        if (!interest.has(key)) return;
        const given = answer.status === "ok" ? (answer.data as ThemeGallery | null) : null;
        answers.set(
          key,
          given !== null && typeof given === "object" && Array.isArray(given.cards)
            ? given
            : { trouble: answer.status === "error" ? answer.error : "purlis did not answer" },
        );
      },
      (why: unknown) => {
        if (interest.has(key)) answers.set(key, { trouble: String(why) });
      },
    )
    .finally(() => {
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
    };
    // `key` is `at`, as a value.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return read;
}

/** How many rows show each level's gallery. */
const interest = new Map<string, number>();

/** For tests: forget every gallery read. */
export function forgetGalleries(): void {
  answers.clear();
  asking.clear();
  interest.clear();
}

/** Whether `read` is the core's answer. */
function answered(read: Read): read is ThemeGallery {
  return read !== undefined && !("trouble" in read);
}

/** The card the person's choice here names: a card by its id, Purlis Classic by any id one of
 *  its appearances has had. */
function cardNamed(gallery: ThemeGallery, id: string | undefined | null): GalleryCard | undefined {
  if (id == null) return undefined;
  const found = gallery.cards.find((one) => one.id === id);
  if (found !== undefined) return found;
  const known = Object.prototype.hasOwnProperty.call(THEME_ALIASES, id) || id in BUILT_IN;
  return known || id === SYSTEM ? gallery.cards.find((one) => one.id === CLASSIC) : undefined;
}

/** **The chosen card**: the person's choice here, else what the levels draw here. */
function chosenCard(gallery: ThemeGallery): GalleryCard | undefined {
  return cardNamed(gallery, gallery.yours?.theme) ?? cardNamed(gallery, gallery.drawn);
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

/** Writes the person's choice at `at`: the machine default with no project. */
async function choose(
  at: ThemeAt | null,
  theme: string | null,
  appearance: string | null,
): Promise<string | undefined> {
  const done =
    at === null
      ? await commands.themeChooseMachine(theme, appearance)
      : await commands.themeChoose(at.plane, at.workspace, theme, appearance);
  if (done.status === "error") return done.error;
  if (at === null) void drawWhatIsInForce();
  else projectThemeChanged(at.plane);
  reread(at);
  return undefined;
}

/** The appearance a new choice of `card` keeps: following the system, when the last choice
 *  did and `card` can; else its first. */
function appearanceFor(card: GalleryCard, yours: YourTheme | null): string | null {
  if (yours?.theme === card.id) return yours.appearance;
  return yours?.appearance === SYSTEM && card.follows === null ? SYSTEM : null;
}

// ---------------------------------------------------------------------------------------
// What a card draws while it is pointed at
// ---------------------------------------------------------------------------------------

/** Each card's theme as the core answered it, by level, card and stamp. */
const drawnThemes = new Map<string, Theme | null>();
/** Bumped by every card entered: an answer for one already left draws nothing. */
let entering = 0;

/** Draws `card` while it is pointed at or focused, in the appearance it would be chosen in. */
function enter(at: ThemeAt | null, card: GalleryCard, yours: YourTheme | null): void {
  if (card.refused) return;
  const mine = ++entering;
  const appearance = appearanceFor(card, yours) ?? SYSTEM;
  const light = systemIsLight();
  const more = systemAsksMoreContrast();
  const key = `${keyOf(at)}\u0000${card.id}\u0000${card.stamp}\u0000${appearance}\u0000${light}\u0000${more}`;
  const kept = drawnThemes.get(key);
  if (kept) {
    showWhileOn(kept);
    return;
  }
  void commands
    .themeCardDrawn(at, card.id, appearance, light, more)
    .then((answer) => {
      const theme =
        answer.status === "ok" && answer.data?.theme
          ? (answer.data.theme as unknown as Theme)
          : null;
      drawnThemes.set(key, theme);
      if (theme !== null && mine === entering) showWhileOn(theme);
    })
    .catch(() => undefined);
}

/** The card was left: the person's theme goes back once the move is over. */
function leave(): void {
  entering++;
  putBackSoon();
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
        void choose(at, card.id, appearanceFor(card, was)).then((why) => {
          setError(why === undefined ? undefined : [why]);
          setBefore(why === undefined ? { was } : undefined);
        });
      };
      return {
        grouped: true,
        error,
        undo:
          before === undefined
            ? undefined
            : () => {
                const was = before.was;
                setBefore(undefined);
                void choose(at, was?.theme ?? null, was?.appearance ?? null).then((why) =>
                  setError(why === undefined ? undefined : [why]),
                );
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
  const stop = useTabStop(
    chosen?.id,
    (gallery?.cards ?? []).map((card) => card.id),
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
  const badge = levelBadge(gallery, chosen);
  return (
    <div className="flex flex-col gap-2">
      <p className="ui-setting-help">
        {gallery.yours !== null ? "Chosen here" : "Drawn here"}:{" "}
        {chosen?.name ?? "your own theme.json"} <SourceBadge badge={badge} />
      </p>
      <RovingFocusGroup.Root asChild loop {...stop}>
        <div
          id={ids.id}
          role="listbox"
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
          className="grid grid-cols-2 gap-3"
          onPointerLeave={leave}
        >
          {gallery.cards.map((card) => {
            const isChosen = card.id === chosen?.id;
            const marks = [
              isChosen ? (gallery.yours !== null ? "Chosen here" : "Drawn here") : undefined,
              card.refused
                ? "Refused"
                : card.warnings.length > 0
                  ? `${card.warnings.length} ${card.warnings.length === 1 ? "warning" : "warnings"}`
                  : undefined,
            ].filter((one): one is string => one !== undefined);
            return (
              <RovingFocusGroup.Item key={card.id} asChild tabStopId={card.id}>
                <div
                  role="option"
                  aria-selected={isChosen}
                  aria-disabled={card.refused || undefined}
                  data-theme-card={card.id}
                  className={
                    "flex flex-col gap-1 rounded-md border border-border-subtle p-2 " +
                    "hover:bg-surface-hover aria-selected:border-accent-base"
                  }
                  onPointerEnter={() => enter(at, card, gallery.yours)}
                  onFocus={() => enter(at, card, gallery.yours)}
                  onBlur={leave}
                  onClick={() => onPick(card)}
                  onKeyDown={keyed(card)}
                >
                  <Pictures at={at} card={card} />
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
    <span className="ui-setting-badge" title={badge.says} aria-description={badge.says}>
      {badge.label}
    </span>
  );
}

/** The most appearances a card shows a picture of; the rest are said. */
const MOST_PICTURES = 3;

/** A card's pictures: the specimen in each of its appearances. */
function Pictures({ at, card }: { at: ThemeAt | null; card: GalleryCard }) {
  if (card.refused)
    return <p className="text-sm text-text-secondary">{card.said[0] ?? "purlis refused it"}</p>;
  const shown = card.appearances.slice(0, MOST_PICTURES);
  const more = card.appearances.length - shown.length;
  return (
    <div className="flex gap-1">
      {shown.map((one) => (
        <Picture key={one.id} at={at} card={card} appearance={one.id} />
      ))}
      {more > 0 && <span className="text-sm text-text-secondary">+{more}</span>}
    </div>
  );
}

function Picture({
  at,
  card,
  appearance,
}: {
  at: ThemeAt | null;
  card: GalleryCard;
  appearance: string;
}) {
  const picture = useSpecimen(at, card.id, card.stamp, appearance);
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
    <span
      className="block aspect-video w-full min-w-0 flex-1 rounded-sm bg-surface-sunken"
      title={picture.state === "none" ? `No picture: ${picture.why}` : undefined}
    />
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
      const value =
        gallery !== undefined && yours !== null && cardNamed(gallery, yours.theme) === card
          ? (yours.appearance ?? "")
          : "";
      return {
        grouped: true,
        error,
        control: (ids) =>
          card === undefined || card.refused ? (
            <p id={ids.id} className="ui-setting-help">
              {card === undefined
                ? "Reading the themes…"
                : "The chosen theme is refused, so purlis draws its own in its place."}
            </p>
          ) : (
            <div className="flex flex-col gap-1">
              <Choice
                kind="radio"
                ids={ids}
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
                  void choose(at, card.id, to === "" ? null : to).then((why) =>
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

/** **The chosen theme's options**: each value, and a badge saying where it comes from. */
function optionsRow(group: string, at: ThemeAt | null): LiveSetting {
  return {
    id: `${group}.options`,
    label: "Options",
    help: "The chosen theme's options: the value of each, and what decided it.",
    writes: "nothing",
    useControl: function useThemeOptions() {
      const read = useGallery(at);
      const gallery = answered(read) ? read : undefined;
      const card = gallery === undefined ? undefined : chosenCard(gallery);
      const drawnHere =
        gallery !== undefined &&
        card !== undefined &&
        card.id === cardNamed(gallery, gallery.drawn)?.id;
      const inForce = useOptionsInForce();
      const options: readonly WindowOption[] | undefined =
        card === undefined ? undefined : drawnHere ? inForce : card.options;
      return {
        grouped: true,
        control: (ids) => (
          <OptionsList options={options} ids={ids} none="The chosen theme has no options." />
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
              <p className="ui-setting-help">Reading the themes…</p>
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

/** **What a card does to the person's own themes**: Duplicate as my theme, and Open folder. */
function folderRow(group: string, at: ThemeAt | null): LiveSetting {
  return {
    id: `${group}.folder`,
    label: "This theme",
    help: "Duplicate the chosen theme as a theme folder of your own that extends it, or open its folder.",
    writes: "this machine",
    useControl: function useThemeFolder() {
      const read = useGallery(at);
      const gallery = answered(read) ? read : undefined;
      const card = gallery === undefined ? undefined : chosenCard(gallery);
      const [said, setSaid] = useState<string>();
      const [error, setError] = useState<readonly string[]>();
      const duplicate = () => {
        if (card === undefined) return;
        void commands.themeDuplicate(at, card.id).then((made) => {
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
          .themeOpenFolder(at, card.id)
          .then((done) => setError(done.status === "error" ? [done.error] : undefined));
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
              <button
                type="button"
                className="ui-button"
                // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
                tabIndex={0}
                disabled={card === undefined || !card.duplicable}
                title={
                  card !== undefined && !card.duplicable
                    ? "A theme of your own extends the built-in theme or another of your own."
                    : undefined
                }
                onClick={duplicate}
              >
                Duplicate as my theme
              </button>
              <button
                type="button"
                className="ui-button"
                // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
                tabIndex={0}
                disabled={card === undefined || !card.folder}
                title={
                  card !== undefined && !card.folder
                    ? `${card.name} has no theme folder: it is ${card.from === "built-in" ? "built into purlis" : "an extension's"}.`
                    : undefined
                }
                onClick={open}
              >
                Open folder
              </button>
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
