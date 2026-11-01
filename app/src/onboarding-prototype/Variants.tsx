// PROTOTYPE, throwaway. Question: what should onboarding look like?
// Three structurally different layouts of the same six screens, in the real window chrome.
// Switch with the floating bar or the left and right arrow keys. Nothing here calls the core.
import { useEffect, useState, type ReactNode } from "react";
import { Check, ChevronLeft, ChevronRight, CloudDownload, FolderOpen, FolderPlus, Laptop, LogIn, Search, Terminal } from "lucide-react";
import { Button } from "../ui/button/Button";
import "./proto.css";

type Screen = "home" | "create" | "get" | "signin" | "git" | "harness";
const SCREENS: Screen[] = ["home", "create", "get", "signin", "git", "harness"];
const NAMES: Record<Screen, string> = { home: "Welcome", create: "Create a project", get: "Get a project", signin: "Sign in", git: "Install git", harness: "Pick your harness" };
const VARIANTS = ["A", "B", "C"] as const;
type V = (typeof VARIANTS)[number];
const LABEL: Record<V, string> = { A: "Launcher (two panes)", B: "One question at a time", C: "Setup rail" };

const REPOS = [
  ["purlis/purlis", "Purlis project", true], ["purlis/purlis-plane", "Purlis project", true],
  ["acme/api-gateway", "Private", false], ["acme/web-storefront", "Private", false], ["aaron/dotfiles", "", false], ["aaron/notes", "", false],
] as const;
const RECENTS = [["acme-platform", "~/work/acme-platform"], ["purlis", "~/IdeaProjects/purlis"], ["charter-notes", "~/oss/charter-notes"]];
const TEMPLATES = ["Fits the repo", "Rust", "TypeScript", "Python", "Go", "Monorepo", "Docs only", "None"];
const HARNESSES = [
  { n: "Claude Code", s: "Ready", on: true }, { n: "Codex", s: "Installed. Asks you to sign in when its chat starts.", on: true },
  { n: "opencode", s: "Not installed", on: false, cmd: "curl -fsSL https://opencode.ai/install | bash" },
];
const ACTIONS: { s: Screen; t: string; d: string; I: typeof FolderPlus; k: string }[] = [
  { s: "create", t: "Create a project", d: "On GitHub, on GitLab or only on this Mac", I: FolderPlus, k: "N" },
  { s: "get", t: "Get a project", d: "Clone your team's from GitHub or GitLab", I: CloudDownload, k: "G" },
  { s: "home", t: "Open a folder…", d: "A project or repo already on this Mac", I: FolderOpen, k: "O" },
];

function Text({ label, ph, help, value }: { label: string; ph?: string; help?: string; value?: string }) {
  return <label className="p-field"><span className="p-label">{label}</span><input className="p-input" placeholder={ph} defaultValue={value} />{help && <span className="p-help">{help}</span>}</label>;
}
function Where({ value, set, cards }: { value: string; set: (v: string) => void; cards?: boolean }) {
  const opts = [["mac", "Only this Mac", "No account needed", Laptop], ["github", "GitHub", "A new repo on your account", CloudDownload], ["gitlab", "GitLab", "A new project on your account", CloudDownload]] as const;
  return <div role="radiogroup" aria-label="Where it lives" className={cards ? "p-where stacked" : "p-where"}>
    {opts.map(([v, t, d, I]) => <button key={v} type="button" role="radio" aria-checked={value === v} className="p-where-opt" onClick={() => set(v)}><I aria-hidden /><span className="p-where-t">{t}</span><span className="p-where-d">{d}</span></button>)}
  </div>;
}
function Template() {
  return <label className="p-field"><span className="p-label">Template</span><select className="p-input">{TEMPLATES.map((t) => <option key={t}>{t}</option>)}</select><span className="p-help">Personas, a review checklist and the commands purlis asks about. Nothing is written into your repo.</span></label>;
}
function CreateFields({ where }: { where: string }) {
  if (where === "mac") return <><Text label="Repo" ph="/path/to/repo" help="purlis opens it in a workspace of its own, on this Mac. Nothing is written into your repo." /><Template /></>;
  return <><Text label="Name" ph="my-project" help="Letters, digits, dots, dashes and underscores." />
    <div className="p-pair"><label className="p-field"><span className="p-label">Owner</span><select className="p-input"><option>aaron</option><option>acme</option></select></label>
    <label className="p-field"><span className="p-label">Visibility</span><select className="p-input"><option>Private</option><option>Public</option></select></label></div><Template />
    <div className="p-dest"><span>Goes into <code>~/Projects/my-project</code></span><Button variant="ghost">Change…</Button></div></>;
}
function GetFields() {
  return <>
    <div className="p-search"><Search aria-hidden /><input className="p-input bare" placeholder="Search your repos, or paste a URL" aria-label="Search your repos, or paste a URL" /></div>
    <ul className="p-rows" role="listbox" aria-label="Your repos">
      {REPOS.map(([n, tag, isP], i) => <li key={n} role="option" aria-selected={i === 0} className="p-row" data-sel={i === 0}><span className="p-row-t">{n}</span>{tag && <span className={isP ? "p-tag accent" : "p-tag"}>{tag}</span>}</li>)}
    </ul>
    <div className="p-dest"><span>Goes into <code>~/Projects/purlis</code></span><Button variant="ghost">Change…</Button></div>
    <p className="p-foot">6 repos on github.com as aaron.</p>
  </>;
}
function SignInFields() {
  return <>
    <div className="p-stack"><Button variant="primary">Sign in with GitHub</Button><Button variant="primary">Sign in with GitLab</Button></div>
    <p className="p-body">purlis keeps the sign-in in your system keychain and never lets it reach a chat.</p>
    <Button variant="ghost">Sign in with a token…</Button>
    <p className="p-foot">For a forge your team hosts. Already use gh or glab? purlis works with those too.</p>
  </>;
}
function GitFields() {
  return <>
    <p className="p-body">purlis works with git, which comes with Apple's command line developer tools. Install opens Apple's installer, and purlis carries on where you were.</p>
    <div className="p-actions-inline"><Button variant="primary">Install</Button><Button>Check again</Button></div>
  </>;
}
function HarnessFields() {
  return <>
    <ul className="p-rows flat" aria-label="Harnesses">
      {HARNESSES.map((h) => <li key={h.n} className="p-hrow"><span className="p-hmark"><Terminal aria-hidden /></span>
        <span className="p-hbody"><span className="p-row-t">{h.n}</span><span className="p-help">{h.s}</span>{h.cmd && <code className="p-cmd">{h.cmd}</code>}</span>
        {h.on ? <span className="p-ok"><Check aria-hidden />Installed</span> : <Button>Install</Button>}</li>)}
    </ul>
    <p className="p-foot">No account? opencode can run on a model served on this Mac by Ollama or LM Studio.</p>
  </>;
}
function fields(s: Screen, where: string, setWhere: (v: string) => void, cards?: boolean): ReactNode {
  if (s === "create") return <><Where value={where} set={setWhere} cards={cards} /><CreateFields where={where} /></>;
  if (s === "get") return <GetFields />;
  if (s === "signin") return <SignInFields />;
  if (s === "git") return <GitFields />;
  if (s === "harness") return <HarnessFields />;
  return null;
}
const LEDE: Partial<Record<Screen, string>> = {
  create: "A project is where purlis keeps your workspaces, personas and memory.",
  get: "Pick one of your repos. purlis clones it and opens it.",
  signin: "Only needed to create or clone on your forge. You can skip it.",
  git: "One thing is missing before purlis can get this project.",
  harness: "A chat runs a coding harness. Your project is ready; pick the one you use.",
};
const PRIMARY: Partial<Record<Screen, string>> = { create: "Create project", get: "Get project", harness: "Start a chat" };
const SECOND: Partial<Record<Screen, string>> = { signin: "Skip for now", git: "Not now", harness: "Skip for now" };

/** A: one window, doors always in sight; a door fills the right pane. */
function A({ s, go, returning }: { s: Screen; go: (s: Screen) => void; returning: boolean }) {
  const [where, setWhere] = useState("mac");
  return <div className="a">
    <aside className="a-left">
      <div className="a-brand"><img src="/src-tauri-icon.png" alt="" onError={(e) => (e.currentTarget.style.display = "none")} /><h1>purlis</h1></div>
      <p className="a-lede">Your coding agents, in one window.</p>
      <nav className="a-doors" aria-label="Ways to start">
        {ACTIONS.map(({ s: t, t: title, d, I, k }) => <button key={title} type="button" className="a-door" aria-current={s === t && t !== "home" ? "page" : undefined} onClick={() => go(t)}>
          <I aria-hidden /><span className="a-door-b"><span className="a-door-t">{title}</span><span className="a-door-d">{d}</span></span><kbd>⌘{k}</kbd></button>)}
      </nav>
      <div className="a-end"><button type="button" className="a-door small" onClick={() => go("signin")} aria-current={s === "signin" ? "page" : undefined}><LogIn aria-hidden /><span className="a-door-t">Sign in to GitHub or GitLab</span></button></div>
    </aside>
    <main className="a-right">
      {s === "home" ? (returning ? <>
        <h2 className="a-h">Recent projects</h2>
        <ul className="p-rows flat">{RECENTS.map(([n, p]) => <li key={n} className="p-row tall"><span className="p-row-t">{n}</span><code className="p-path">{p}</code></li>)}</ul>
      </> : <div className="a-first">
        <h2 className="a-h big">Start with a project</h2>
        <p className="p-body">A project is where purlis keeps your workspaces, personas and memory. Create one, get your team's, or open one you already have.</p>
        <ol className="a-next"><li><Check aria-hidden /><span><b>purlis is ready.</b> Nothing else to install for it.</span></li><li><span className="dot" /><span><b>Pick a project.</b> It takes a minute.</span></li><li><span className="dot" /><span><b>Start a chat</b> on Claude Code, Codex or opencode.</span></li></ol>
      </div>) : <div className="a-page">
        <header><h2 className="a-h big">{NAMES[s]}</h2>{LEDE[s] && <p className="p-body">{LEDE[s]}</p>}</header>
        <div className="p-form">{fields(s, where, setWhere)}</div>
        <footer className="a-foot">{SECOND[s] && <Button variant="ghost" onPress={() => go("home")}>{SECOND[s]}</Button>}<span className="grow" />{PRIMARY[s] && <Button variant="primary">{PRIMARY[s]}</Button>}</footer>
      </div>}
    </main>
  </div>;
}

/** B: one centred question, a thin progress line, fixed Back and Continue. */
function B({ s, go, returning }: { s: Screen; go: (s: Screen) => void; returning: boolean }) {
  const [where, setWhere] = useState("mac");
  const [stage, setStage] = useState(0);
  const at = { home: 0, signin: 1, create: 1, get: 1, git: 2, harness: 3 }[s];
  return <div className="b">
    <div className="b-progress" role="progressbar" aria-valuemin={0} aria-valuemax={3} aria-valuenow={at}><span style={{ width: `${((at + 1) / 4) * 100}%` }} /></div>
    <div className="b-scroll"><div className="b-col">
      {s === "home" ? <>
        <h1 className="b-h">{returning ? "Welcome back" : "Welcome to purlis"}</h1>
        <p className="p-body center">{returning ? "Open a recent project, or start another." : "Your coding agents, in one window. Start with a project."}</p>
        {returning && <ul className="p-rows b-rec">{RECENTS.map(([n, p]) => <li key={n} className="p-row tall"><span className="p-row-t">{n}</span><code className="p-path">{p}</code></li>)}</ul>}
        <div className="b-doors">{ACTIONS.map(({ s: t, t: title, d, I }) => <button key={title} type="button" className="b-door" onClick={() => go(t)}><I aria-hidden /><span className="a-door-b"><span className="a-door-t">{title}</span><span className="a-door-d">{d}</span></span><ChevronRight aria-hidden /></button>)}</div>
        <p className="center"><Button variant="ghost" onPress={() => go("signin")} icon={<LogIn />}>Sign in to GitHub or GitLab</Button></p>
      </> : s === "create" ? (stage === 0 ? <>
        <h1 className="b-h">Where does it live?</h1><p className="p-body center">You can publish a Mac-only project later.</p><Where value={where} set={setWhere} cards />
      </> : <><h1 className="b-h">About this project</h1><div className="p-form"><CreateFields where={where} /></div></>) : <>
        <h1 className="b-h">{NAMES[s]}</h1>{LEDE[s] && <p className="p-body center">{LEDE[s]}</p>}<div className="p-form">{fields(s, where, setWhere, true)}</div>
      </>}
    </div></div>
    {s !== "home" && <footer className="b-foot"><Button variant="ghost" icon={<ChevronLeft />} onPress={() => (s === "create" && stage === 1 ? setStage(0) : go("home"))}>Back</Button><span className="grow" />
      {SECOND[s] && <Button variant="ghost">{SECOND[s]}</Button>}
      {s === "create" ? <Button variant="primary" onPress={() => stage === 0 && setStage(1)}>{stage === 0 ? "Continue" : "Create project"}</Button> : PRIMARY[s] && <Button variant="primary">{PRIMARY[s]}</Button>}</footer>}
  </div>;
}

/** C: a rail of the steps with their state; the step's page beside it, its act pinned below. */
function C({ s, go, returning }: { s: Screen; go: (s: Screen) => void; returning: boolean }) {
  const [where, setWhere] = useState("mac");
  const rail: [Screen, string][] = [["home", "Welcome"], [s === "get" ? "get" : "create", "Your project"], ["git", "Git"], ["harness", "Your harness"]];
  const order = ["home", "signin", "create", "get", "git", "harness"];
  const at = order.indexOf(s);
  return <div className="c">
    <nav className="c-rail" aria-label="Setting up">
      <div className="a-brand small"><h1>purlis</h1></div>
      <ol>{rail.map(([t, n]) => { const i = order.indexOf(t); const cur = t === s || (s === "signin" && t === "create"); const done = i < at && !cur;
        return <li key={n} aria-current={cur ? "step" : undefined} data-done={done}><button type="button" onClick={() => go(t)}><span className="c-dot">{done && <Check aria-hidden />}</span>{n}</button></li>; })}</ol>
      {returning && <div className="c-rec"><h2>Recent</h2>{RECENTS.map(([n]) => <button key={n} type="button">{n}</button>)}</div>}
    </nav>
    <main className="c-main">
      <div className="c-page">
        {s === "home" ? <>
          <h1 className="c-h">Welcome to purlis</h1><p className="p-body">A project is where purlis keeps your workspaces, personas and memory.</p>
          <div className="c-doors">{ACTIONS.map(({ s: t, t: title, d, I, k }) => <button key={title} type="button" className="a-door boxed" onClick={() => go(t)}><I aria-hidden /><span className="a-door-b"><span className="a-door-t">{title}</span><span className="a-door-d">{d}</span></span><kbd>⌘{k}</kbd></button>)}</div>
          <Button variant="ghost" icon={<LogIn />} onPress={() => go("signin")}>Sign in to GitHub or GitLab</Button>
        </> : <><h1 className="c-h">{NAMES[s]}</h1>{LEDE[s] && <p className="p-body">{LEDE[s]}</p>}<div className="p-form">{fields(s, where, setWhere)}</div></>}
      </div>
      {s !== "home" && <footer className="c-foot"><Button variant="ghost" icon={<ChevronLeft />} onPress={() => go("home")}>Back</Button><span className="grow" />{SECOND[s] && <Button variant="ghost">{SECOND[s]}</Button>}{PRIMARY[s] && <Button variant="primary">{PRIMARY[s]}</Button>}</footer>}
    </main>
  </div>;
}

export function Variants() {
  const q = () => new URLSearchParams(location.hash.slice(1));
  const [v, setV] = useState<V>((q().get("v") as V) ?? "A");
  const [s, setS] = useState<Screen>((q().get("s") as Screen) ?? "home");
  const [returning, setR] = useState(q().get("r") === "1");
  useEffect(() => { const p = q(); p.set("v", v); p.set("s", s); p.set("r", returning ? "1" : "0"); history.replaceState(null, "", `#${p}`); }, [v, s, returning]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (/INPUT|TEXTAREA|SELECT/.test((e.target as HTMLElement).tagName)) return;
      const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
      if (d) setV((was) => VARIANTS[(VARIANTS.indexOf(was) + d + 3) % 3]);
    };
    addEventListener("keydown", key); return () => removeEventListener("keydown", key);
  }, []);
  const V_ = { A, B, C }[v];
  return <>
    <V_ key={v} s={s} go={setS} returning={returning} />
    <div className="p-bar" role="toolbar" aria-label="Prototype switcher">
      <button type="button" aria-label="Previous variant" onClick={() => setV(VARIANTS[(VARIANTS.indexOf(v) + 2) % 3])}><ChevronLeft /></button>
      <b>{v}</b><span>{LABEL[v]}</span>
      <button type="button" aria-label="Next variant" onClick={() => setV(VARIANTS[(VARIANTS.indexOf(v) + 1) % 3])}><ChevronRight /></button>
      <select aria-label="Screen" value={s} onChange={(e) => setS(e.target.value as Screen)}>{SCREENS.map((one) => <option key={one} value={one}>{NAMES[one]}</option>)}</select>
      <label><input type="checkbox" checked={returning} onChange={(e) => setR(e.target.checked)} /> Returning</label>
    </div>
  </>;
}
