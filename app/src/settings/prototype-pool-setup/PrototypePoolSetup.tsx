// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (spec #4469): Pool setup as one view tab, the step list on the left and one
 * step on the right (Q29). The dark bar at the bottom is the prototype's, not the design.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, LayoutList, Server, X } from "lucide-react";
import { Button, IconButton } from "../../ui/button/Button";
import { Segment, SegmentedControl } from "../../ui/choice/Radio";
import {
  SCENARIOS,
  simOf,
  STATE_WORD,
  stateOf,
  STEP_NAME,
  STEPS,
  type Sim,
  type StepId,
} from "./data";
import { StateIcon } from "./kit";
import { Around, Step, type Api } from "./screens";
import "./prototype.css";

/* ---------- the step list ---------- */

function StepList({ sim, onGo }: { sim: Sim; onGo: (step: StepId) => void }) {
  const steps = STEPS[sim.kind];
  const finished = steps.filter((step) => stateOf(sim, step) === "done").length;
  return (
    <nav className="ps-rail" aria-label="Steps of Pool setup" data-ps-keep="">
      <p className="ps-rail-count">
        {finished} of {steps.length} done
      </p>
      <ol className="ps-steps">
        {steps.map((step, i) => {
          const state = stateOf(sim, step);
          const here = step === sim.at;
          const line = sim.lines[step];
          const word = STATE_WORD[state === "current" ? "later" : state];
          const canGo = state !== "later";
          return (
            <li key={step} className="ps-step" data-state={state} data-here={here ? "" : undefined}>
              <button
                type="button"
                className="ps-step-press"
                aria-current={here ? "step" : undefined}
                disabled={!canGo}
                onClick={() => onGo(step)}
              >
                <span className="ps-step-mark">
                  {state === "done" && StateIcon.done}
                  {state === "attention" && StateIcon.attention}
                  {state === "skipped" && StateIcon.skipped}
                  {state === "waiting" && StateIcon.waiting}
                  {(state === "current" || state === "later") && i + 1}
                </span>
                <span className="ps-step-words">
                  <span className="ps-step-name">{STEP_NAME[step]}</span>
                  {state === "attention" || state === "waiting" || state === "skipped" ? (
                    <span className="ps-step-line" data-tone={state}>
                      {line ?? word}
                    </span>
                  ) : (
                    line && <span className="ps-step-line">{line}</span>
                  )}
                  <span className="ps-vh">{word && !line ? "" : word ? `, ${word}` : ""}</span>
                </span>
              </button>
            </li>
          );
        })}
      </ol>
      <p className="ps-saved">Your choices are saved as you go. Close this tab whenever you like.</p>
    </nav>
  );
}

/* ---------- the window around it ---------- */

function Frame({ title, children, alone }: { title: string; children: React.ReactNode; alone?: boolean }) {
  return (
    <div className="ps-window">
      <div className="ps-tabbar" aria-hidden="true">
        {!alone && <span className="ps-faketab">steward 2</span>}
        <span className="ps-faketab" data-on="">
          <Server size={12} /> {title}
        </span>
      </div>
      {children}
    </div>
  );
}

/* ---------- the prototype's own bar ---------- */

function StateIndex({
  current,
  onPick,
  onClose,
}: {
  current: string;
  onPick: (id: string) => void;
  onClose: () => void;
}) {
  const groups = useMemo(() => {
    const out = new Map<string, typeof SCENARIOS>();
    for (const one of SCENARIOS) out.set(one.group, [...(out.get(one.group) ?? []), one]);
    return [...out];
  }, []);
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    panel.current?.querySelector<HTMLElement>('[aria-current="true"]')?.focus();
  }, []);
  return (
    <div
      className="ps-index"
      ref={panel}
      role="dialog"
      aria-label="Every state"
      onKeyDown={(event) => {
        if (event.key === "Escape") onClose();
      }}
    >
      <div className="ps-index-head">
        <strong>Every state ({SCENARIOS.length})</strong>
        <IconButton aria-label="Close" size="sm" variant="ghost" icon={<X aria-hidden size={14} />} onPress={onClose} />
      </div>
      <div className="ps-index-body">
        {groups.map(([group, items]) => (
          <section key={group}>
            <h2 className="ps-index-group">{group}</h2>
            <ul>
              {items.map((one) => (
                <li key={one.id}>
                  <button
                    type="button"
                    className="ps-index-item"
                    aria-current={one.id === current ? "true" : undefined}
                    onClick={() => onPick(one.id)}
                  >
                    <span>{one.title}</span>
                    <span className="ps-index-shows">{one.shows}</span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </div>
  );
}

export type Look = { appearance: string; theme: string };

function ProtoBar({
  id,
  onPick,
  said,
  look,
  onLook,
  looks,
  event,
}: {
  event?: { label: string; run: () => void };
  id: string;
  onPick: (id: string) => void;
  said: string | undefined;
  look: Look;
  onLook: (to: Look) => void;
  looks: boolean;
}) {
  const [open, setOpen] = useState(false);
  const at = Math.max(0, SCENARIOS.findIndex((one) => one.id === id));
  const step = (by: number) => onPick(SCENARIOS[(at + by + SCENARIOS.length) % SCENARIOS.length].id);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (target.closest("input, textarea, [contenteditable]")) return;
      if (event.key === "[") step(-1);
      if (event.key === "]") step(1);
      if (event.key === "\\") setOpen((was) => !was);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
  return (
    <>
      {open && (
        <StateIndex
          current={id}
          onClose={() => setOpen(false)}
          onPick={(to) => {
            onPick(to);
            setOpen(false);
          }}
        />
      )}
      <div className="ps-protobar" role="region" aria-label="Prototype controls, not part of the design" data-ps-keep="">
        <span className="ps-protobar-tag">Prototype</span>
        <IconButton aria-label="Previous state" size="sm" variant="ghost" icon={<ChevronLeft aria-hidden size={14} />} shortcut="[" onPress={() => step(-1)} />
        <button type="button" className="ps-protobar-name" onClick={() => setOpen(!open)} aria-expanded={open}>
          <LayoutList aria-hidden size={14} />
          <span className="ps-protobar-at">
            {at + 1}/{SCENARIOS.length}
          </span>
          <span className="ps-protobar-title">{SCENARIOS[at].title}</span>
        </button>
        <IconButton aria-label="Next state" size="sm" variant="ghost" icon={<ChevronRight aria-hidden size={14} />} shortcut="]" onPress={() => step(1)} />
        {looks && (
          <>
            <SegmentedControl
              aria-label="Appearance"
              size="sm"
              value={look.appearance}
              onChange={(appearance) => onLook({ ...look, appearance })}
            >
              <Segment value="light">Light</Segment>
              <Segment value="dark">Dark</Segment>
            </SegmentedControl>
            <SegmentedControl
              aria-label="Theme"
              size="sm"
              value={look.theme}
              onChange={(theme) => onLook({ ...look, theme })}
            >
              <Segment value="purlis">Purlis</Segment>
              <Segment value="purlis-classic">Classic</Segment>
            </SegmentedControl>
          </>
        )}
        {event && (
          <Button size="sm" variant="soft-accent" onPress={event.run}>
            {event.label}
          </Button>
        )}
        <span className="ps-vh" role="status">
          {said}
        </span>
      </div>
      {said && (
        <div className="ps-protosaid" aria-hidden="true">
          Prototype: {said}
        </div>
      )}
    </>
  );
}

/* ---------- the tab ---------- */

const AROUND_TAB: Record<string, string> = {
  welcome: "Welcome",
  page: "Pool: dev-box",
  needs: "Needs you",
  remove: "Pool: dev-box",
  uninstall: "Pool: team-gpu",
  cli: "Terminal",
};

/** What happens outside the window, for the screens that wait on it. Prototype only. */
function outsideEvent(sim: Sim): { label: string; to: (s: Sim) => Sim } | undefined {
  const screen = sim.screens[sim.at];
  const clear = (s: Sim, step: StepId) => ({ ...s.states, [step]: undefined });
  if (sim.around) return undefined;
  if (sim.at === "where" && screen === "docker-missing")
    return {
      label: "Pretend: Docker finished installing",
      to: (s) => ({ ...s, screens: { ...s.screens, where: "docker-stopped" } }),
    };
  if (sim.at === "where" && screen === "docker-stopped")
    return {
      label: "Pretend: Docker started",
      to: (s) => ({ ...s, states: clear(s, "where"), screens: { ...s.screens, where: "docker-found" } }),
    };
  if (sim.at === "doctor" && screen === "admin-wait")
    return {
      label: "Pretend: your admin finished",
      to: (s) => ({
        ...s,
        states: { ...s.states, doctor: "done", install: "done" },
        lines: { ...s.lines, doctor: "Nothing missing", install: "Operator 0.9.2 running" },
        screens: { ...s.screens, doctor: "admin-ready" },
      }),
    };
  if (sim.at === "identity" && (screen === "waiting" || screen === "mint-admin"))
    return {
      label: "Pretend: your admin finished",
      to: (s) => ({
        ...s,
        states: clear(s, "identity"),
        lines: { ...s.lines, identity: "acme-purlis-agent on 12 repos" },
        screens: { ...s.screens, identity: "done" },
      }),
    };
  return undefined;
}

function startAt(): string {
  try {
    const asked = new URLSearchParams(window.location.search).get("state");
    if (asked && SCENARIOS.some((one) => one.id === asked)) return asked;
    const hash = window.location.hash.slice(1);
    if (hash && SCENARIOS.some((one) => one.id === hash)) return hash;
  } catch {
    /* a frame without a location: start at the top */
  }
  return "where-docker";
}

export function PrototypePoolSetup({
  look,
  onLook,
  start,
}: {
  look?: Look;
  onLook?: (to: Look) => void;
  start?: string;
}) {
  const [id, setId] = useState(start ?? startAt);
  const [sim, setSim] = useState<Sim>(() => simOf(start ?? startAt()));
  const [said, setSaid] = useState<string>();

  const pick = useCallback((to: string) => {
    setId(to);
    setSim(simOf(to));
    setSaid(undefined);
    try {
      window.history.replaceState(null, "", `#${to}`);
    } catch {
      /* a frame that refuses it: the state still changes */
    }
  }, []);

  useEffect(() => {
    if (!said) return;
    const later = setTimeout(() => setSaid(undefined), 4000);
    return () => clearTimeout(later);
  }, [said]);

  const api: Api = useMemo(() => {
    const next = (s: Sim) => STEPS[s.kind][STEPS[s.kind].indexOf(s.at) + 1];
    const leave = (s: Sim, state: "done" | "skipped", line: string | undefined, screen?: string): Sim => {
      const to = next(s);
      const states = { ...s.states, [s.at]: state };
      if (!to) return { ...s, states, lines: { ...s.lines, [s.at]: line ?? s.lines[s.at] } };
      return {
        ...s,
        at: to,
        resumed: false,
        states: { ...states, [to]: s.states[to] === "skipped" ? undefined : s.states[to] },
        lines: { ...s.lines, [s.at]: line ?? s.lines[s.at] },
        screens: { ...s.screens, [to]: screen ?? (to === "try" ? "ready" : s.screens[to]) },
      };
    };
    return {
      sim,
      show: (screen, state, line) =>
        setSim((s) => ({
          ...s,
          resumed: false,
          screens: { ...s.screens, [s.at]: screen },
          states: state ? { ...s.states, [s.at]: state === "current" ? undefined : state } : s.states,
          lines: line ? { ...s.lines, [s.at]: line } : s.lines,
        })),
      done: (line, screen) =>
        setSim((s) => leave(s, "done", line, screen)),
      skip: (line) => setSim((s) => leave(s, "skipped", line)),
      go: (step, screen) =>
        setSim((s) => {
          // What was reached keeps its state as you jump back (story 13).
          const order = STEPS[s.kind];
          const frozen = { ...s.states };
          for (const one of order)
            if (order.indexOf(one) < order.indexOf(s.at) && !frozen[one]) frozen[one] = "done";
          return { ...s, at: step, states: frozen, screens: screen ? { ...s.screens, [step]: screen } : s.screens };
        }),
      patch: (to) => setSim(to),
      around: (where) => setSim((s) => ({ ...s, around: where })),
      say: setSaid,
    };
  }, [sim]);

  const poolName = sim.kind === "docker" ? "dev-box" : sim.kind === "join" ? "team-gpu" : "team-gpu";
  const named = sim.states.pool === "done" || sim.lines.pool !== undefined;
  const tabTitle = named ? `Pool setup: ${poolName}` : "Pool setup";

  const outside = outsideEvent(sim);
  const bar = (
    <ProtoBar
      event={outside && { label: outside.label, run: () => setSim(outside.to) }}
      id={id}
      onPick={pick}
      said={said}
      look={look ?? { appearance: "dark", theme: "purlis" }}
      onLook={onLook ?? (() => undefined)}
      looks={onLook !== undefined}
    />
  );

  if (sim.around)
    return (
      <div className="ps-root">
        <Frame title={AROUND_TAB[sim.around] ?? "steward 2"} alone>
          <main className="ps-around-main">
            <Around id={sim.around} api={api} />
          </main>
        </Frame>
        {bar}
      </div>
    );

  const order = STEPS[sim.kind];
  return (
    <div className="ps-root">
      <Frame title={tabTitle}>
        <div className="ps-tab">
          <div className="ps-tab-head">
            <span className="ps-tab-name">Pool setup</span>
            <span className="ps-tab-sub">
              {named ? `${poolName} · ` : ""}
              Step {order.indexOf(sim.at) + 1} of {order.length}
            </span>
          </div>
          <div className="ps-tab-body">
            <StepList sim={sim} onGo={(step) => api.go(step)} />
            <main className="ps-main">
              <div className="ps-main-inner" key={sim.at}>
                {sim.resumed && (
                  <p className="ps-resumed" role="status">
                    Welcome back. You stopped at Sign-ins, and your choices were kept.
                  </p>
                )}
                <Step api={api} />
              </div>
            </main>
          </div>
        </div>
      </Frame>
      {bar}
    </div>
  );
}

export { Button };
