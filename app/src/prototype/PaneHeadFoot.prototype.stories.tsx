// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, NOT FOR MERGE. Spec #5191, "the pane head and the pane foot" (M70), on the branch
 * `prototype/pane-head-foot`.
 *
 * The question: what does a chat pane look like with two lines, the head (who the chat is,
 * what it does, what you can do) and the quiet foot under the terminal (where it runs, where it
 * came from, how healthy it is), and at which widths does each drop its parts?
 *
 * Drawn with the real families (Button, IconButton, Badge, Count, Tooltip, PersonaMark) and the
 * theme's tokens; the stores, the terminal and the notices band are stubs. Every press says
 * which existing catalogue row it would run, in the line under the pane. The model
 * (`paneHeadFoot.prototype.ts`) answers the parts, printed under each pane.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  Bell,
  Columns2,
  CornerDownRight,
  Ellipsis,
  Rows2,
  ScrollText,
  ShieldHalf,
  Square,
  TriangleAlert,
  X,
} from "lucide-react";
import { createContext, useContext, useState, type ReactNode } from "react";
import { expect, waitFor, within } from "storybook/test";
import { PersonaMark } from "../PersonaMark";
import { Badge, Count } from "../ui/badge/Badge";
import { Button, ButtonRow, IconButton } from "../ui/button/Button";
import { TIPS_BELOW, Tooltip } from "../ui/tooltip/Tooltip";
import { layoutOf, type FootPart, type HeadPart, type PaneKind } from "./paneHeadFoot.prototype";
import "./paneHeadFoot.prototype.css";

type Variant = "A" | "B" | "C";

const VARIANT_SAYS: Record<Variant, string> = {
  A: "A · dots: parts joined by “·”, muted words, the ctx bar under its number.",
  B: "B · plain: no separators, wider gaps, the ctx bar beside its number.",
  C: "C · strip: as A, on a raised ground with a hairline above, apart from the terminal.",
};

/** What one pane shows: the facts each case gives it. */
type Case = {
  kind: PaneKind;
  persona: string;
  name: string;
  crumbs?: string[];
  state: string;
  harness: string;
  workspace: string;
  elsewhere?: string;
  from?: string;
  skills?: string;
  sandbox: string;
  ctx: number;
  cache: number;
  notices?: number;
};

const CHAT: Case = {
  kind: "chat",
  persona: "steward",
  name: "M69 chats, tabs and panes: build",
  state: "Working",
  harness: "Claude Code",
  workspace: "smart-ide",
  elsewhere: "ops",
  from: "steward 3",
  skills: "2 skills missing",
  sandbox: "floor",
  ctx: 62,
  cache: 91,
  notices: 2,
};

const TASK: Case = {
  ...CHAT,
  kind: "task",
  crumbs: ["steward 2", "fix-login"],
  name: "fix-login",
  state: "Working",
  harness: "Codex",
  elsewhere: undefined,
  from: "steward 2",
  skills: undefined,
  ctx: 18,
  cache: 74,
  notices: 0,
};

const ENDED: Case = {
  ...CHAT,
  name: "Release notes for 0.42",
  kind: "ended",
  state: "Ended",
  from: undefined,
  skills: undefined,
  ctx: 87,
  cache: 40,
  notices: 0,
};

const TERMINAL = [
  "⏺ Read app/src/LayoutPanes.tsx (1680 lines)",
  "⏺ Update(app/src/paneHead.ts)",
  "  ⎿  Updated with 14 additions and 3 removals",
  "",
  "✻ Thinking…",
  "",
  "> _",
].join("\n");

/** Where a press is said: the stand-in for the catalogue. */
const Pressed = createContext<(what: string) => void>(() => {});

function useSay(what: string) {
  const say = useContext(Pressed);
  return () => say(what);
}

/** The persona, its state dot on its corner (`ChatPersonaState` stands in). */
function Persona({ of, ended }: { of: Case; ended: boolean }) {
  return (
    <span className="proto-persona">
      <PersonaMark persona={of.persona} tip={`${of.persona}, ${of.state.toLowerCase()}`} />
      <span className="proto-dot" data-state={ended ? "ended" : "working"} aria-hidden="true" />
    </span>
  );
}

function Head({ of, parts }: { of: Case; parts: HeadPart[] }) {
  const has = (p: HeadPart) => parts.includes(p);
  const ended = of.kind === "ended";
  const say = useContext(Pressed);
  const notices = of.notices ?? 0;
  return (
    // Tooltips below the head (#5189): a hint at the bell's end would cover Split right.
    <div className="proto-head" {...TIPS_BELOW}>
      <div className="proto-head-chips">
        {has("persona") && <Persona of={of} ended={ended} />}
        {has("crumbs") && of.crumbs ? (
          <span className="proto-crumbs">
            {of.crumbs.slice(0, -1).map((c) => (
              <Button
                key={c}
                inline
                variant="ghost"
                onPress={() => say(`Go to ${c} (chat.show)`)}
              >
                {c}
              </Button>
            ))}
            <span aria-hidden="true">›</span>
            <b>{of.crumbs[of.crumbs.length - 1]}</b>
            <span>{of.state.toLowerCase()}</span>
          </span>
        ) : (
          <span className="proto-name">{of.name}</span>
        )}
        {has("mark") && (
          <span className="proto-mark">
            {!has("persona") && <span className="proto-dot" aria-hidden="true" />}
            {has("markWord") ? of.state : <span className="sr-only">{of.state}</span>}
          </span>
        )}
      </div>
      <div className="proto-actions">
        {has("stateButton") &&
          (of.kind === "task" ? (
            has("buttonWord") ? (
              <Button
                size="sm"
                variant="quiet"
                icon={<ScrollText aria-hidden="true" />}
                aria-label="Brief of fix-login"
                onPress={() => say("Brief (task.brief)")}
              >
                Brief
              </Button>
            ) : (
              <IconButton
                size="sm"
                variant="quiet"
                icon={<ScrollText aria-hidden="true" />}
                aria-label="Brief of fix-login"
                onPress={() => say("Brief (task.brief)")}
              />
            )
          ) : has("buttonWord") ? (
            <Button
              size="sm"
              variant="quiet"
              icon={<Square aria-hidden="true" />}
              aria-label="Stop"
              onPress={() => say("Stop (chat.stop)")}
            >
              Stop
            </Button>
          ) : (
            <IconButton
              size="sm"
              variant="quiet"
              icon={<Square aria-hidden="true" />}
              aria-label="Stop"
              onPress={() => say("Stop (chat.stop)")}
            />
          ))}
        <IconButton
          size="sm"
          variant="quiet"
          icon={<Ellipsis aria-hidden="true" />}
          aria-label={`More for ${of.name}`}
          onPress={() => say("⋯ (the chat's list)")}
        />
        <span className="proto-bell">
          <IconButton
            size="sm"
            variant="quiet"
            icon={<Bell aria-hidden="true" />}
            aria-label={`Pane notices: ${notices} ${notices === 1 ? "notice" : "notices"}`}
            tooltip="Minimise"
            onPress={() => say("Bell: minimise the notices (pane.notices)")}
          />
          {notices > 0 && <Count aria-hidden="true">{notices}</Count>}
        </span>
        <IconButton
          size="sm"
          variant="quiet"
          icon={<Columns2 aria-hidden="true" />}
          aria-label="Split right"
          onPress={() => say("Split right (pane.split.right)")}
        />
        {has("splitDown") && (
          <IconButton
            size="sm"
            variant="quiet"
            icon={<Rows2 aria-hidden="true" />}
            aria-label="Split down"
            onPress={() => say("Split down (pane.split.down)")}
          />
        )}
        {has("close") && (
          <IconButton
            size="sm"
            variant="quiet"
            icon={<X aria-hidden="true" />}
            aria-label="Close pane"
            onPress={() => say("Close (pane.close)")}
          />
        )}
      </div>
    </div>
  );
}

function Sep({ variant }: { variant: Variant }) {
  return variant === "B" ? null : (
    <span className="proto-sep" aria-hidden="true">
      ·
    </span>
  );
}

function Foot({
  of,
  parts,
  variant,
  height,
}: {
  of: Case;
  parts: FootPart[];
  variant: Variant;
  height: "tab" | "chrome";
}) {
  const has = (p: FootPart) => parts.includes(p);
  const ended = of.kind === "ended";
  const where = useSay(`Harness and workspace rows (chat.harness, chat.workspace)`);
  const from = useSay(`Go to ${of.from} (chat.from)`);
  const skills = useSay("Skills panel at its warning (chat.skills)");
  const ctx = useSay("Context / Activity view (chat.activity)");
  const tone = of.ctx >= 80 ? "warn" : "ok";
  return (
    <div
      className="proto-foot"
      role="group"
      aria-label={`About ${of.name}`}
      data-variant={variant}
      data-height={height}
      data-ended={ended || undefined}
      {...TIPS_BELOW}
    >
      <div className="proto-foot-start">
        <Button inline variant="ghost" onPress={where}>
          {of.harness}
          {has("workspace") && (of.elsewhere ? "" : ` · ${of.workspace}`)}
        </Button>
        {has("workspace") && of.elsewhere && (
          <Badge variant="accent">
            <span className="sr-only">filed in </span>
            {of.elsewhere}
          </Badge>
        )}
        {has("from") && of.from && (
          <>
            <Sep variant={variant} />
            <Button inline variant="ghost" onPress={from}>
              ↳ from {of.from}
            </Button>
          </>
        )}
        {(has("skills") || has("skillsShort")) && (
          <>
            <Sep variant={variant} />
            {has("skills") ? (
              <Button
                inline
                variant="ghost"
                className="proto-warn"
                onPress={skills}
              >
                ⚠ {of.skills}
              </Button>
            ) : (
              <IconButton
                size="sm"
                variant="quiet"
                className="proto-warn"
                icon={<TriangleAlert aria-hidden="true" />}
                aria-label={of.skills ?? "Skills warning"}
                onPress={skills}
              />
            )}
          </>
        )}
        {has("sandbox") && (
          <Tooltip tip={`${of.sandbox} sandbox`}>
            <span className="proto-sandbox" role="img" aria-label={`${of.sandbox} sandbox`}>
              <ShieldHalf aria-hidden="true" />
            </span>
          </Tooltip>
        )}
      </div>
      <div className="proto-foot-end">
        {ended && <Badge variant="neutral">ended</Badge>}
        {has("ctx") && (
          <Button
            inline
            variant="ghost"
            aria-label={`Context ${of.ctx}% full${ended ? ", when it ended" : ""}`}
            onPress={ctx}
          >
            <span className="proto-ctx">
              <span className="proto-label">ctx</span>
              <span className="proto-value">{of.ctx}%</span>
              <span className="proto-bar" data-tone={tone} aria-hidden="true">
                <span style={{ inlineSize: `${of.ctx}%` }} />
              </span>
            </span>
          </Button>
        )}
        {has("cache") && (
          <span className="proto-cache">
            <span className="proto-label">cache</span>{" "}
            <span className="proto-value">{of.cache}%</span>
          </span>
        )}
      </div>
    </div>
  );
}

/** One pane: head, the band, the terminal, the foot; its model's answer printed under it. */
function Pane({
  of,
  width,
  variant = "A",
  band = false,
  height = "tab",
  caption,
  showModel = true,
}: {
  of: Case;
  width: number;
  variant?: Variant;
  band?: boolean;
  height?: "tab" | "chrome";
  caption?: ReactNode;
  showModel?: boolean;
}) {
  const [said, setSaid] = useState("Nothing pressed");
  const answer = layoutOf(of.kind, width, {
    skillsWarning: of.skills !== undefined,
    from: of.from !== undefined,
    stateButton: of.kind !== "ended",
  });
  return (
    <Pressed.Provider value={(what) => setSaid(`Would run: ${what}`)}>
      <figure style={{ margin: 0, inlineSize: width }}>
        {caption && <figcaption className="proto-caption">{caption}</figcaption>}
        {of.kind === "folded" ? (
          <div className="proto-rail">
            <Persona of={of} ended={false} />
            <span className="proto-rail-name">{of.name}</span>
          </div>
        ) : (
          <div className="proto-pane">
            <Head of={of} parts={answer.head} />
            {band && (
              <div className="proto-band" role="status">
                <p>steward asks: may it push the branch prototype/pane-head-foot?</p>
                <Button size="sm" variant="secondary" onPress={() => setSaid("Would run: Answer")}>
                  Answer
                </Button>
              </div>
            )}
            <pre className="proto-terminal" aria-label="Terminal (stub)">
              {TERMINAL}
            </pre>
            {answer.foot.length > 0 && (
              <Foot of={of} parts={answer.foot} variant={variant} height={height} />
            )}
          </div>
        )}
        <p className="proto-model" role="status">
          {said}
          {showModel &&
            `\nhead: ${answer.head.join(", ")}\nfoot: ${answer.foot.join(", ") || "(none)"}`}
        </p>
      </figure>
    </Pressed.Provider>
  );
}

/** What a width drops against the widest pane, said in a caption. */
function dropsSaid(of: Case, width: number): string {
  const facts = { skillsWarning: of.skills !== undefined, from: of.from !== undefined, stateButton: true };
  const wide = layoutOf(of.kind, 2000, facts);
  const here = layoutOf(of.kind, width, facts);
  const head = wide.head.filter((p) => !here.head.includes(p));
  const foot = wide.foot.filter((p) => !here.foot.includes(p) && p !== "skills");
  const shrunk = here.foot.includes("skillsShort") ? ["skills → ⚠"] : [];
  const all = [
    ...(head.length ? [`head drops ${head.join(", ")}`] : []),
    ...(foot.length || shrunk.length ? [`foot drops ${[...foot, ...shrunk].join(", ")}`] : []),
  ];
  return `${width} px: ${all.length ? all.join("; ") : "nothing dropped"}`;
}

function Widths({ of = CHAT, variant = "A" }: { of?: Case; variant?: Variant }) {
  return (
    <div className="proto-stack">
      {[900, 520, 420, 300].map((w) => (
        <Pane key={w} of={of} width={w} variant={variant} caption={dropsSaid(of, w)} />
      ))}
    </div>
  );
}

/** Free play: every knob the spec leaves open. */
function FreePlay() {
  const [width, setWidth] = useState(520);
  const [kind, setKind] = useState<"chat" | "task" | "ended" | "folded">("chat");
  const [variant, setVariant] = useState<Variant>("A");
  const [band, setBand] = useState(false);
  const [height, setHeight] = useState<"tab" | "chrome">("tab");
  const of = { chat: CHAT, task: TASK, ended: ENDED, folded: { ...CHAT, kind: "folded" as const } }[
    kind
  ];
  return (
    <div className="proto-stack" style={{ inlineSize: 920 }}>
      <ButtonRow size="sm" wraps>
        {[900, 640, 560, 520, 480, 420, 380, 340, 300, 260].map((w) => (
          <Button key={w} variant={w === width ? "primary" : "secondary"} onPress={() => setWidth(w)}>
            {`${w} px`}
          </Button>
        ))}
      </ButtonRow>
      <ButtonRow size="sm" wraps>
        {(["chat", "task", "ended", "folded"] as const).map((k) => (
          <Button key={k} variant={k === kind ? "primary" : "secondary"} onPress={() => setKind(k)}>
            {k}
          </Button>
        ))}
        {(["A", "B", "C"] as const).map((v) => (
          <Button
            key={v}
            variant={v === variant ? "primary" : "secondary"}
            onPress={() => setVariant(v)}
          >
            {`Variant ${v}`}
          </Button>
        ))}
        <Button variant={band ? "primary" : "secondary"} onPress={() => setBand(!band)}>
          Notices band
        </Button>
        <Button
          variant="secondary"
          onPress={() => setHeight(height === "tab" ? "chrome" : "tab")}
        >
          {height === "tab" ? "Foot: tab strip height" : "Foot: chrome height (24 px)"}
        </Button>
      </ButtonRow>
      <p className="proto-caption">{VARIANT_SAYS[variant]}</p>
      <Pane
        key={`${kind}-${width}`}
        of={of}
        width={width}
        variant={variant}
        band={band}
        height={height}
        caption={dropsSaid(of, width)}
      />
    </div>
  );
}

const meta = {
  title: "Prototype (not for merge)/Pane head and foot",
  component: FreePlay,
  parameters: { a11y: { test: "todo" } },
} satisfies Meta<typeof FreePlay>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Every knob: width, pane kind, foot variant, the band, the foot's height. */
export const FreePlayAllKnobs: Story = {};

/** A chat's pane at 900, 520, 420 and 300 px, each caption naming what that width drops. */
export const AtFourWidths: Story = { render: () => <Widths /> };

/** The three looks of the foot side by side, at a wide and a narrow width. */
export const FootVariants: Story = {
  render: () => (
    <div className="proto-row">
      {(["A", "B", "C"] as const).map((v) => (
        <div key={v} className="proto-stack">
          <Pane of={CHAT} width={560} variant={v} caption={VARIANT_SAYS[v]} showModel={false} />
          <Pane of={CHAT} width={420} variant={v} caption={`${v} at 420 px`} showModel={false} />
        </div>
      ))}
    </div>
  ),
};

/** A pane showing a task: the breadcrumb on top, the foot describes the task (its harness,
 *  gauge and skills), `↳ from` names the chat that asked. */
export const ATask: Story = { render: () => <Widths of={TASK} /> };

/** An ended chat: the foot kept with its harness and workspace, the gauge frozen at its last
 *  values, greyed, and marked `ended`. No state button. */
export const AnEndedChat: Story = {
  render: () => (
    <div className="proto-stack">
      <Pane of={ENDED} width={900} caption="Ended, 900 px" />
      <Pane of={ENDED} width={420} caption="Ended, 420 px" />
    </div>
  ),
};

/** The notices band stays under the head, above the terminal; the foot is untouched. */
export const WithNotices: Story = {
  render: () => (
    <div className="proto-stack">
      <Pane of={CHAT} width={900} band caption="Notices band under the head, 900 px" />
      <Pane of={CHAT} width={420} band caption="Notices band under the head, 420 px" />
    </div>
  ),
};

/** A folded pane in a narrow window is a rail and draws no foot; its neighbour keeps its own. */
export const FoldedPane: Story = {
  render: () => (
    <div className="proto-row">
      <Pane of={{ ...CHAT, kind: "folded" }} width={40} showModel={false} />
      <Pane of={CHAT} width={420} caption="Folded rail beside an open pane" />
    </div>
  ),
};

/** The bell sized as its neighbours, and its hint below it, clear of Split right (#5189). */
export const BellAndItsTooltip: Story = {
  render: () => <Pane of={CHAT} width={520} caption="The bell's hint opens below, not over Split right" />,
  play: async ({ canvas }) => {
    const bell = canvas.getByRole("button", { name: /^Pane notices/ });
    const right = canvas.getByRole("button", { name: "Split right" });
    bell.focus();
    const tip = await within(document.body).findByRole("tooltip");
    await waitFor(() => {
      const a = tip.getBoundingClientRect();
      const b = right.getBoundingClientRect();
      const overlaps = a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
      expect(overlaps).toBe(false);
    });
    await expect(bell.getBoundingClientRect().width).toBe(right.getBoundingClientRect().width);
  },
};

const appearances = (label: string) => () => (
  <div className="proto-stack">
    <p className="proto-caption">{label}</p>
    <Pane of={CHAT} width={900} band />
    <Pane of={TASK} width={420} />
    <Pane of={ENDED} width={520} />
  </div>
);

export const AppearanceDark: Story = {
  globals: { theme: "purlis", appearance: "dark", density: "comfortable" },
  render: appearances("Purlis dark"),
};
export const AppearanceLight: Story = {
  globals: { theme: "purlis", appearance: "light", density: "comfortable" },
  render: appearances("Purlis light"),
};
export const AppearanceDarkHighContrast: Story = {
  globals: { theme: "purlis", appearance: "dark-high-contrast", density: "comfortable" },
  render: appearances("Purlis dark high contrast"),
};
export const AppearanceLightHighContrast: Story = {
  globals: { theme: "purlis", appearance: "light-high-contrast", density: "comfortable" },
  render: appearances("Purlis light high contrast"),
};
export const AppearanceCompact: Story = {
  globals: { theme: "purlis", appearance: "dark", density: "compact" },
  render: appearances("Purlis dark, compact density: the foot takes the compact tab strip's height"),
};
