// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A branch's files, and one file of a branch** (RC-5, FM-2): the light editor's two view tabs.
 *
 * - **Files · \<branch\>** is the branch as a tree (`BranchTree`, the explorer's own file rows)
 *   beside a read-only preview of the file picked, with a divider between them that can be
 *   dragged or moved from the keyboard. Where it was left is the tab's (`tabs.splits`), and the
 *   record keeps it for a tab open at a quit (`reopen::View::split`). One tab for a whole
 *   branch, so reading through it does not leave a tab per file behind.
 *   A jump to a file at a line (a search hit, FM-8; `fileJump.ts`) lands here: the preview picks
 *   the file and brings the line into view, and the tree opens the folders above the file and
 *   scrolls to its row (#1137).
 * - **\<file\> · \<branch\>** is one file in a tab of its own: what the preview's *Open in a
 *   tab of its own* opens, and what a jump from a diff, a record or the knowledge graph opens.
 *
 * **The preview** (#1103 F4) draws code in the light editor, an image as an image, markdown
 * rendered (with its source a press away), and for a binary file or one past 2 MiB a sentence
 * with its size instead of its contents. Both tabs only read. The core names the folder from the
 * branch, refuses a path that leaves it, and decides what a file is by its bytes.
 *
 * **Show what changed** (FM-11) is beside a file the branch changed against its base, and only
 * there, in both tabs (#1189): it opens the file's comparison in a view tab of its own
 * (`PieceDiff.tsx`).
 *
 * **Copy path** and **Reveal** (#1143) are beside the file in both, as a tree row's menu has
 * them (FM-10): the core places the file, as it does for the row.
 *
 * **Open in your editor** (RC-20, ADR 0081 §3) is beside the file in both: the file and the
 * line the cursor is on go to the editor chosen in Settings. The window sends the
 * branch, the path, the line and which editor; the core checks the path as it checks a read,
 * and builds the URL or the program's arguments itself.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import Markdown, { type Components } from "react-markdown";
import { Group, Panel, type Layout } from "react-resizable-panels";
import { Separator } from "../ui/panel/Separator";
import { FileText, LoaderCircle, Pause, Play, Users } from "lucide-react";
import { EmptyState } from "../ui/empty-state/EmptyState";
import { ExternalLink } from "../ReleaseNotes";
import {
  commands,
  type ChangeMark,
  type OwnFileView,
  type PieceFile,
  type PlaneId,
} from "../bindings";
import {
  pieceDiffTitle,
  pieceDiffView,
  isPlaneRoot,
  isWorkspaceFiles,
  pieceFileTitle,
  pieceFileView,
  type Place,
} from "../pieceViews";
import { REVEAL_SAID, isPage, pinToWorkspaceRow, type Does, type Offer } from "../actions";
import { ArtifactFrame } from "../ArtifactTab";
import { Badge } from "../ui/badge/Badge";
import type { ViewRef } from "../tabs";
import { BranchTree } from "./BranchTree";
import { LightEditor } from "./LightEditor";
import { settleJump, usePendingJump } from "../fileJump";
import { onAMac } from "../mac";
import { modifierSaid } from "../PaneLinkSurfaces";
import { outsideLanding } from "../paneLinkFiles";
import { DragHandle, PickAChat, StartAChatHere, type Referenced } from "../references";
import { useBranchMoved } from "./branchMoved";
import { readAt } from "./lastRead";
import { Said, ToYourEditor } from "./ToYourEditor";
import {
  MOST_PIXELS,
  SVG,
  animated,
  bytesOf,
  isSvg,
  openFrames,
  svgSide,
  useInSight,
  usePlayback,
  useSvgDraws,
  type Frames,
} from "./imagePreview";
import { motionReduced } from "../theme/motion";
import { Button, ToggleButton } from "../ui/button/Button";
import { editorToolbar, ToolbarFile } from "../ui/tabs/Tabs";

export { ToYourEditor };

/** A size, as a person reads one. */
export function sized(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KiB`;
  return `${Math.round(bytes / (1024 * 1024))} MiB`;
}

/**
 * One file, as the tabs draw it: still being read, refused with the core's sentence, or what
 * the core found it to be. One union, so every state is drawn in one place (#988).
 */
export type FileRead = { kind: "reading" } | { kind: "refused"; why: string } | PieceFile;

export const READING: FileRead = { kind: "reading" };

/** What a file read answers: the core's `PieceFile`, or why it refused. */
type FileAnswer = { status: "ok"; data: PieceFile } | { status: "error"; error: string };

/**
 * **One file, read from the core each time its path or `from` changes**: the one path every file
 * preview reads a file through, a branch's tabs (#988) and a skill's Files tab (#5183). `ask`
 * reads `path` from whatever `from` names. `undefined` while no file is picked.
 */
export function useFileRead(
  path: string | undefined,
  ask: (path: string) => Promise<FileAnswer>,
  from: readonly unknown[],
): FileRead | undefined {
  const [read, setRead] = useState<{ path: string; read: FileRead }>();
  useEffect(() => {
    if (path === undefined) return;
    let gone = false;
    const told = (read: FileRead) => {
      if (!gone) setRead({ path, read });
    };
    void ask(path)
      .then((answer) =>
        told(answer.status === "error" ? { kind: "refused", why: answer.error } : answer.data),
      )
      .catch((err: unknown) => told({ kind: "refused", why: String(err) }));
    return () => {
      gone = true;
    };
    // The read is the caller's, asked again on what it names.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...from, path]);
  if (path === undefined) return undefined;
  return read?.path === path ? read.read : READING;
}

/** What a workspace's own file says of itself beside its bytes (#5375): whether purlis keeps
 *  it, and the view of its own a kept one has. */
export type Kept = { path: string; managed: boolean; view: OwnFileView | null };

/** One file of the branch: {@link useFileRead} on the branch's folder, the plane root's, or a
 *  workspace's own (#5375), which says to `onKept` whether purlis keeps it. */
function useFile(
  plane: PlaneId,
  cut: Place,
  path: string | undefined,
  onKept?: (kept: Kept) => void,
): FileRead | undefined {
  return useFileRead(
    path,
    (path) =>
      // The plane root's file is the window's own read (issue 1916), which names no branch.
      isPlaneRoot({ workspace: cut.workspace, repo: cut.repo, piece: cut.piece })
        ? commands.planeFile(plane, path)
        : isWorkspaceFiles(cut)
          ? commands.workspaceFile(plane, cut.workspace, path).then((said) => {
              if (said.status === "error") return said;
              onKept?.({ path, managed: said.data.managed, view: said.data.view });
              return { status: "ok" as const, data: said.data.file };
            })
          : commands.pieceFile(plane, cut.workspace, cut.repo, cut.piece, path),
    [plane, cut.workspace, cut.repo, cut.piece],
  );
}

/** What each own view of a kept file is called, and what opens it (#5375). */
const OWN_VIEWS: Record<OwnFileView, { said: string; does: (workspace: string) => Does }> = {
  memory: { said: "Open Memory", does: () => ({ verb: "showSideView", view: "memory" }) },
  todos: { said: "Open Todos", does: () => ({ verb: "showSideView", view: "todos" }) },
  sessions: { said: "Open Sessions", does: () => ({ verb: "showSideView", view: "sessions" }) },
  vision: { said: "Edit the vision", does: (workspace) => ({ verb: "editVision", workspace }) },
};

/**
 * **A file purlis keeps in a workspace** (#5375): the *managed* mark, said in words too, and
 * the way to its own view where it has one. It opens read-only, as every file here does; the
 * mark says that it is purlis's to change.
 */
export function KeptBy({
  workspace,
  kept,
  onPress,
}: {
  workspace: string;
  kept: Kept;
  onPress?: (offer: Offer) => void;
}) {
  if (!kept.managed) return null;
  const own = kept.view === null ? undefined : OWN_VIEWS[kept.view];
  return (
    <span className="kept-by ui-text-meta">
      <Badge>managed</Badge> Kept by purlis, so it opens read-only.
      {own !== undefined && onPress !== undefined && (
        <>
          {" "}
          <Button
            variant="link"
            size="sm"
            onPress={() =>
              onPress({
                id: `workspace-file.own-view:${workspace}`,
                title: own.said,
                available: true,
                reason: "",
                does: own.does(workspace),
              })
            }
          >
            {own.said}
          </Button>
        </>
      )}
    </span>
  );
}

/**
 * **Whether the branch changed the file at `path`** against its base, committed or not (FM-11):
 * its mark in `branch_status`, the explorer's own markers (FM-4), or `undefined` when it did not
 * or the status could not be read. Read when the tab opens, again when another file is picked, and
 * again when the window hears this branch moved (#1189, as the comparison tab does), so *Show
 * what changed* comes and goes with the branch. **One read at a time**: a pick or a move during
 * a read asks for one more after it, never one each.
 */
function useChanged(plane: PlaneId, cut: Place, path: string | undefined): ChangeMark | undefined {
  const [marks, setMarks] = useState<ReadonlyMap<string, ChangeMark>>();
  const flight = useRef<{ again: boolean } | undefined>(undefined);
  const read = useRef<() => void>(() => {});
  useEffect(() => {
    read.current = () => {
      if (flight.current !== undefined) {
        flight.current.again = true;
        return;
      }
      const now = { again: false };
      flight.current = now;
      const told = (got: ReadonlyMap<string, ChangeMark>) => {
        flight.current = undefined;
        setMarks(got);
        if (now.again) read.current();
      };
      void commands
        .branchStatus(plane, cut.workspace, cut.repo, cut.piece)
        .then((said) =>
          told(
            said.status === "error"
              ? new Map()
              : new Map(said.data.changes.map((one) => [one.path, one.mark])),
          ),
        )
        .catch(() => told(new Map()));
    };
  });
  // Nothing of the plane root is compared (issue 1916): it has no base, so it is never asked;
  // nor a workspace's own files (#5375), which are no branch.
  const compared = !isPlaneRoot(cut) && !isWorkspaceFiles(cut);
  useEffect(() => {
    if (path !== undefined && compared) read.current();
  }, [plane, cut.workspace, cut.repo, cut.piece, path, compared]);
  useBranchMoved(plane, cut, () => {
    if (path !== undefined && compared) read.current();
  });
  if (path === undefined) return undefined;
  const mark = marks?.get(path);
  // A deleted file has nothing to pick; one that was, a moment ago, is not offered either.
  return mark === "deleted" ? undefined : mark;
}

/**
 * The line the cursor is on in the file at `path`: `start` (else 1) until it moves, and back to
 * that when another file is drawn.
 *
 * Each move is also the file's line last read (`lastRead.ts`, #1143): where a tree row's *Open
 * in your editor* opens it. A jump's line arrives as a move too, since it puts the cursor there.
 */
function useCursorLine(plane: PlaneId, place: Place, path: string | undefined, start?: number) {
  const [moved, setMoved] = useState<{ path: string | undefined; start?: number; line: number }>();
  const line =
    moved !== undefined && moved.path === path && moved.start === start ? moved.line : (start ?? 1);
  return {
    line,
    moved: (line: number) => {
      setMoved({ path, start, line });
      if (path !== undefined) readAt(plane, place, path, line);
    },
  };
}

/**
 * **Copy path and Reveal** (#1143): the file's path in the branch copied, or the file shown in
 * the platform's file manager — the core's `copy_branch_path` and `reveal_branch_path`, the same
 * commands a tree row's menu runs (FM-10). The core places the file and refuses what it will not
 * place; the sentence it answers is said under the buttons, as *Open in your editor*'s is.
 */
function CopyAndReveal({ plane, cut, path }: { plane: PlaneId; cut: Place; path: string }) {
  // What was said, for the file it was said about: another file says nothing until it is tried.
  const [said, setSaid] = useState<{ about: string; sentence: string }>();
  const sentence = said?.about === path ? said.sentence : undefined;
  const answered = (
    asked: Promise<{ status: "ok" } | { status: "error"; error: string }>,
    done?: string,
  ) => {
    setSaid(undefined);
    void asked
      .then((answer) => {
        const told = answer.status === "error" ? answer.error : done;
        if (told !== undefined) setSaid({ about: path, sentence: told });
      })
      .catch((err: unknown) => setSaid({ about: path, sentence: String(err) }));
  };
  return (
    <>
      <Button
        size="sm"
        onPress={() =>
          answered(
            commands.copyBranchPath(plane, cut.workspace, cut.repo, cut.piece, path, false),
            `Copied the path of ${path}.`,
          )
        }
      >
        Copy path
      </Button>
      <Button
        size="sm"
        onPress={() =>
          answered(commands.revealBranchPath(plane, cut.workspace, cut.repo, cut.piece, path))
        }
      >
        {REVEAL_SAID}
      </Button>
      {sentence !== undefined && <Said>{sentence}</Said>}
    </>
  );
}

/** The lines selected in the file at `path`, forgotten when another file is drawn. */
function useSelectedLines(path: string | undefined) {
  const [held, setHeld] = useState<{ path?: string; lines?: Lines }>({});
  const lines = held.path === path ? held.lines : undefined;
  return { lines, selected: (lines: Lines | undefined) => setHeld({ path, lines }) };
}

type Lines = { first: number; last: number };

/**
 * **The file into a chat** (FM-9): a handle to drag onto a chat, and *Ask a chat about this* and
 * *Add to a chat's context*, each naming the file — or the lines selected in it.
 */
function ToAChat({
  plane,
  cut,
  path,
  lines,
}: {
  plane: PlaneId;
  cut: Place;
  path: string;
  lines?: Lines;
}) {
  const referenced: Referenced = { plane, ...cut, path, folder: false, lines };
  return (
    <>
      <DragHandle referenced={referenced} />
      <PickAChat referenced={referenced} how="ask" />
      <PickAChat referenced={referenced} how="add" />
      <StartAChatHere referenced={referenced} />
    </>
  );
}

/** Whether a file is markdown, by its name: what the preview renders rather than lists. */
function isMarkdown(path: string): boolean {
  return /\.(md|markdown)$/i.test(path);
}

/**
 * Markdown's headings one level under the preview's own, its links opened in the browser only
 * when they are http or https (`ReleaseNotes`), and an image said by its words: the window
 * loads nothing a file names (the CSP has no source for it), so a picture is never fetched.
 */
const MARKDOWN: Components = {
  h1: "h3",
  h2: "h4",
  h3: "h5",
  a: ({ href, children }) => <ExternalLink href={href}>{children}</ExternalLink>,
  img: ({ alt }) => (
    <span className="piece-markdown-image">{`[image${alt ? `: ${alt}` : ""}]`}</span>
  ),
};

/**
 * An image, drawn on a canvas from its bytes (`imagePreview.ts`).
 *
 * **Decoded here, never loaded from a URL**: the window's CSP gives images no source but the
 * app's own (`tauri.conf.json`), and a `data:` or `blob:` source would be one more way for
 * whatever the window shows to fetch. `createImageBitmap` decodes bytes it is handed and loads
 * nothing.
 *
 * **An animated image plays** (#1132) on the same canvas, frame by frame from the same bytes,
 * where the webview has WebCodecs' `ImageDecoder`, with *Pause* beside it. It plays only while
 * it can be seen: a tab put behind another, or the window hidden, stops it where it is. Under
 * reduced motion it shows its first frame and *Play*. Where the webview has no decoder for
 * frames, its first frame is said to be one.
 */
function ImagePreview({
  name,
  mime,
  bytes,
}: {
  name: string;
  mime: string;
  bytes: Uint8Array<ArrayBuffer>;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [drawn, setDrawn] = useState<{ of: Uint8Array; size?: string; trouble?: string }>();
  const moving = useMemo(() => animated(bytes, mime), [bytes, mime]);
  const [opened, setOpened] = useState<{ of: Uint8Array; frames?: Frames }>();
  const [paused, setPaused] = useState(motionReduced);
  const inSight = useInSight(canvas);
  useEffect(() => {
    let gone = false;
    const say = (got: { size?: string; trouble?: string }) => {
      if (!gone) setDrawn({ of: bytes, ...got });
    };
    void (async () => {
      try {
        const bitmap = await createImageBitmap(new Blob([bytes], { type: mime }));
        const to = canvas.current;
        if (gone || to === null) {
          bitmap.close();
          return;
        }
        if (bitmap.width * bitmap.height > MOST_PIXELS) {
          bitmap.close();
          say({ trouble: huge(name, bitmap) });
          return;
        }
        to.width = bitmap.width;
        to.height = bitmap.height;
        to.getContext("2d")?.drawImage(bitmap, 0, 0);
        say({ size: `${bitmap.width} × ${bitmap.height}` });
        bitmap.close();
      } catch (err) {
        say({ trouble: `purlis could not draw ${name}: ${String(err)}` });
      }
    })();
    return () => {
      gone = true;
    };
  }, [bytes, mime, name]);
  useEffect(() => {
    if (!moving) return;
    let gone = false;
    let held: Frames | undefined;
    void openFrames(bytes, mime).then((frames) => {
      if (gone) frames?.close();
      else {
        held = frames;
        setOpened({ of: bytes, frames });
      }
    });
    return () => {
      gone = true;
      held?.close();
    };
  }, [bytes, mime, moving]);
  const now = drawn?.of === bytes ? drawn : undefined;
  const open = opened?.of === bytes ? opened : undefined;
  const frames = now?.size === undefined ? undefined : open?.frames;
  usePlayback(
    frames,
    (image) => canvas.current?.getContext("2d")?.drawImage(image, 0, 0),
    !paused && inSight,
  );
  const still =
    moving && open !== undefined && open.frames === undefined
      ? " · its first frame: this window does not play animation"
      : "";
  return (
    <figure className="piece-image">
      <canvas ref={canvas} role="img" aria-label={name} />
      <figcaption>
        {now?.trouble ?? (now?.size === undefined ? mime : `${now.size} · ${mime}${still}`)}
        {frames !== undefined && (
          <Button
            size="sm"
            variant="ghost"
            onPress={() => setPaused(!paused)}
            className="piece-image-play"
          >
            {paused ? <Play /> : <Pause />}
            {paused ? "Play" : "Pause"}
          </Button>
        )}
      </figcaption>
    </figure>
  );
}

/** What the preview says of an image past what it draws. */
function huge(name: string, side: { width: number; height: number }): string {
  return `${name} is ${side.width} × ${side.height} pixels, past what the preview draws (40 megapixels)`;
}

/** An image the core sent as base64. */
function BytesPreview({ name, mime, base64 }: { name: string; mime: string; base64: string }) {
  const bytes = useMemo(() => bytesOf(base64), [base64]);
  return <ImagePreview name={name} mime={mime} bytes={bytes} />;
}

/**
 * **An SVG drawn as an image** (#1132): its text encoded back to bytes and decoded as an
 * `image/svg+xml`, so it runs no script and loads nothing it names, as any image does. Never put
 * into the page as markup. One that declares a canvas past what the preview draws is said by its
 * size and never decoded, and so is one whose size cannot be measured before decoding.
 */
function SvgPreview({ name, text }: { name: string; text: string }) {
  const bytes = useMemo(() => new TextEncoder().encode(text), [text]);
  const side = svgSide(text);
  if (side === "unread")
    return (
      <EmptyState
        mark={FileText}
        headline={`${name} sets its size in a way the preview cannot measure, so it is not drawn`}
        body="Its text is under Source."
        variant="panel"
      />
    );
  if (side !== undefined && side.width * side.height > MOST_PIXELS)
    return (
      <EmptyState
        mark={FileText}
        headline={huge(name, side)}
        body="Its text is under Source."
        variant="panel"
      />
    );
  return <ImagePreview name={name} mime={SVG} bytes={bytes} />;
}

/** A file as the preview draws it, or the sentence that says why it does not: the Files tab's
 *  preview here, and a skill's Files tab's (`SkillFiles.tsx`). */
export function Shown({
  path,
  read,
  line,
  column,
  landing,
  onLine,
  onLines,
  source = false,
  pageAt,
}: {
  path: string;
  read: FileRead;
  line?: number;
  /** The column on `line` the cursor lands at, from 1. */
  column?: number;
  /** The jump that brought the file here, so a jump again lands again (`LightEditor`). */
  landing?: number;
  onLine?: (line: number) => void;
  onLines?: (lines: Lines | undefined) => void;
  /** Markdown's source rather than the rendering. */
  source?: boolean;
  /** Where the file is, for an `.html` page rendered privately (#5953). None: its text. */
  pageAt?: { plane: PlaneId; cut: Place };
}) {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const svgDraws = useSvgDraws();
  switch (read.kind) {
    case "reading":
      return <EmptyState mark={LoaderCircle} headline={`Reading ${path}…`} variant="panel" />;
    case "refused":
      return <EmptyState headline={read.why} variant="panel" testid="piece-file-trouble" />;
    case "text":
      if (svgDraws === undefined && isSvg(path) && !source)
        return <EmptyState mark={LoaderCircle} headline={`Reading ${path}…`} variant="panel" />;
      if (svgDraws === true && isSvg(path) && !source)
        return <SvgPreview key={path} name={name} text={read.text} />;
      if (pageAt !== undefined && isPage(path) && !source)
        return <PagePreview key={path} plane={pageAt.plane} cut={pageAt.cut} path={path} />;
      return isMarkdown(path) && !source ? (
        <article className="piece-markdown release-notes" data-testid="piece-markdown">
          <Markdown skipHtml components={MARKDOWN}>
            {read.text}
          </Markdown>
        </article>
      ) : (
        <LightEditor
          path={path}
          text={read.text}
          line={line}
          column={column}
          landing={landing}
          onLine={onLine}
          onLines={onLines}
        />
      );
    case "image":
      return <BytesPreview key={path} name={name} mime={read.mime} base64={read.base64} />;
    case "huge-image":
      return (
        <EmptyState
          mark={FileText}
          headline={huge(name, read)}
          body="Open it in your editor."
          variant="panel"
        />
      );
    case "binary":
      return (
        <EmptyState
          mark={FileText}
          headline={`${name} is a binary file (${sized(read.bytes)})`}
          body="The preview draws text and images only."
          variant="panel"
        />
      );
    case "too-large":
      return (
        <EmptyState
          mark={FileText}
          headline={`${name} is ${sized(read.bytes)}, past what the preview draws (2 MiB)`}
          body="Open it in your editor."
          variant="panel"
        />
      );
  }
}

/**
 * One file of a branch, in a tab of its own, brought to `line` when one is given. *Show what
 * changed* is beside it where the branch changed it (#1189), as in the Files tab's preview.
 */
export function PieceFileTab({
  plane,
  cut,
  path,
  line,
  onOpenView,
  onPress,
}: {
  plane: PlaneId;
  cut: Place;
  path: string;
  line?: number;
  /** Opens a view tab: the file's comparison. No *Show what changed* without it. */
  onOpenView?: (view: ViewRef, title: string) => void;
  /** Carries out an offer: a kept file's way to its own view (#5375). No link without it. */
  onPress?: (offer: Offer) => void;
}) {
  const [kept, setKept] = useState<Kept>();
  const read = useFile(plane, cut, path, setKept) ?? READING;
  const changed = useChanged(plane, cut, path);
  const landed = useRootLanding(plane, cut, path);
  const lands = landed?.line ?? line;
  const at = useCursorLine(plane, cut, path, lands);
  const selection = useSelectedLines(path);
  // A jump to a line (a diff, a record) lands in the text at that line; a file opened to be read
  // is rendered.
  const [source, setSource] = useState(lands !== undefined);
  const [sourceFor, setSourceFor] = useState(landed?.at);
  if (landed !== undefined && landed.at !== sourceFor) {
    setSourceFor(landed.at);
    setSource(true);
  }
  // What acts on a branch's file — your editor, its path, a chat's context — names a branch,
  // which the plane root is not (issue 1916): there the file is read, and that is all.
  const ofABranch = !isPlaneRoot(cut);
  return (
    <div className="piece-file">
      <header {...editorToolbar("piece-files-head")}>
        {/* The tab is the file's name: the path alone, quiet (#3147). */}
        <ToolbarFile path={path} named={false} />
        <span className="piece-files-actions">
          <SourceToggle path={path} read={read} source={source} onSource={setSource} pages />
          {ofABranch && <ToYourEditor plane={plane} cut={cut} path={path} line={at.line} />}
          {ofABranch && <CopyAndReveal plane={plane} cut={cut} path={path} />}
          {changed !== undefined && onOpenView !== undefined && (
            <ShowWhatChanged cut={cut} path={path} onOpenView={onOpenView} />
          )}
          {ofABranch && <ToAChat plane={plane} cut={cut} path={path} lines={selection.lines} />}
          <PinPageButton cut={cut} path={path} read={read} onPress={onPress} />
        </span>
        {kept !== undefined && kept.path === path && (
          <KeptBy workspace={cut.workspace} kept={kept} onPress={onPress} />
        )}
      </header>
      <Shown
        path={path}
        read={read}
        line={lands}
        column={landed?.column}
        landing={landed?.at}
        onLine={at.moved}
        onLines={selection.selected}
        source={source}
        pageAt={{ plane, cut }}
      />
    </div>
  );
}

/**
 * **A jump into the project's root lands in the file's own tab** (#6356): the root has no
 * branch's file tab to land in (`PlaneView.tsx`), so a pane's link to a file there, or any jump
 * to one at a line, opens this tab, which takes the line and column from the pending jump as
 * the Files tab does. Only the root's: a branch's jump is the Files tab's to settle.
 */
function useRootLanding(plane: PlaneId, cut: Place, path: string) {
  const jump = usePendingJump(plane, cut);
  const root = isPlaneRoot(cut);
  const [landed, setLanded] = useState<{ at: number; line?: number; column?: number }>();
  if (root && jump !== undefined && jump.path === path && jump.at !== landed?.at)
    setLanded({ at: jump.at, line: jump.line, column: jump.column });
  useEffect(() => {
    if (landed !== undefined) settleJump(landed.at);
  }, [landed]);
  return landed;
}

/**
 * **A file outside the project, in a tab of its own** (#6216): one a pane's link opened after its
 * ask was answered, read only, at the line and column the link named. The core reads it only for
 * the chat whose pane opened it, within the hour, and while it is still the file it was then
 * (`pane_link_file`); otherwise the tab says to open it from its link again.
 */
export function OutsideFileTab({
  plane,
  session,
  path,
}: {
  plane: PlaneId;
  session: number | null;
  path: string;
}) {
  const [landing] = useState(() => outsideLanding(path));
  const read =
    useFileRead(
      path,
      (path) =>
        commands.paneLinkFile({ plane, session }, path).then((said) => {
          if (said.status === "error") return said;
          if (said.data.read === "not-opened")
            return { status: "error" as const, error: notOpenedSaid(onAMac()) };
          return { status: "ok" as const, data: said.data.file };
        }),
      [plane, session],
    ) ?? READING;
  return (
    <div className="piece-file">
      <header {...editorToolbar("piece-files-head")}>
        <ToolbarFile path={path} named={false} />
      </header>
      <Shown
        path={path}
        read={read}
        line={landing.line}
        column={landing.column}
        source={landing.line !== undefined}
      />
    </div>
  );
}

/** What a file outside the project's tab says when the core will not read it for this chat. */
export function notOpenedSaid(mac: boolean): string {
  return `purlis shows a file outside the project only after you open it from its link: ${modifierSaid(mac)} it in the chat again.`;
}

/** *Show what changed*: the file's comparison against the branch's base, in a view tab. */
function ShowWhatChanged({
  cut,
  path,
  onOpenView,
}: {
  cut: Place;
  path: string;
  onOpenView: (view: ViewRef, title: string) => void;
}) {
  return (
    <Button
      size="sm"
      onPress={() => onOpenView(pieceDiffView(cut, path), pieceDiffTitle(cut, path))}
    >
      Show what changed
    </Button>
  );
}

/**
 * *Source*, beside a markdown file, or an SVG the window draws: its text in the light editor
 * rather than rendered.
 */
export function SourceToggle({
  path,
  read,
  source,
  onSource,
  pages = false,
}: {
  path: string;
  read: FileRead;
  source: boolean;
  onSource: (source: boolean) => void;
  /** Whether an `.html` page is rendered here, so its text is a press away too (#5953). */
  pages?: boolean;
}) {
  const svgDraws = useSvgDraws();
  if (
    read.kind !== "text" ||
    !(isMarkdown(path) || (pages && isPage(path)) || (isSvg(path) && svgDraws === true))
  )
    return null;
  return (
    <ToggleButton size="sm" isSelected={source} onPress={() => onSource(!source)}>
      Source
    </ToggleButton>
  );
}

/** The tree's panel, as the divider's layout names it. */
const TREE = "piece-files-tree";
/** The tree's share of the tab when it was never moved, and the shares it is held between. */
const SPLIT = { start: 30, least: 10, most: 70 };
/** Where the operator last left a file tab's divider in this window: where a branch's tab that
 *  has never been moved starts, so a second branch opens at the width the first was given. */
let lastSplit: number | undefined;

/**
 * A branch's files: the tree, and the file picked beside it in the preview, with a divider
 * between them that is dragged or moved with the arrow keys (`react-resizable-panels`, as the
 * window's own regions are). Where it is left is told to the tab (`onSplit`).
 */
export function PieceFilesTab({
  plane,
  cut,
  onOpenView,
  split,
  onSplit,
  onPress,
}: {
  plane: PlaneId;
  cut: Place;
  onOpenView: (view: ViewRef, title: string) => void;
  /** A row of the tree had its menu used (FM-10): carried out as the window carries any row. */
  onPress?: (offer: Offer) => void;
  /** Where the tab's divider was left: the tree's share in percent. */
  split?: number;
  /** The operator moved the divider. */
  onSplit?: (split: number) => void;
}) {
  const [picked, setPicked] = useState<string>();
  const [source, setSource] = useState<{ path: string; on: boolean }>();
  // **A jump lands here** (FM-8): the file it names is picked and its line brought into view.
  // Taken while drawing, the way React keeps what an earlier render saw, so the preview never
  // draws the old file first; settled after, so a tab opened later does not land on it again.
  const jump = usePendingJump(plane, cut);
  const [landed, setLanded] = useState<{
    at: number;
    path: string;
    line?: number;
    column?: number;
  }>();
  if (jump !== undefined && jump.at !== landed?.at) {
    setLanded({ at: jump.at, path: jump.path, line: jump.line, column: jump.column });
    setPicked(jump.path);
  }
  useEffect(() => {
    if (landed !== undefined) settleJump(landed.at);
  }, [landed]);
  const line = landed !== undefined && landed.path === picked ? landed.line : undefined;
  const column = landed !== undefined && landed.path === picked ? landed.column : undefined;
  const at = useCursorLine(plane, cut, picked, line);
  const selection = useSelectedLines(picked);
  const read = useFile(plane, cut, picked);
  const changed = useChanged(plane, cut, picked);
  // **Read once**: a panel's size is a constraint, and a constraint that changes re-registers
  // the panel (`RegionFrame.tsx`). The divider is where the operator's hand put it already.
  const [started] = useState(() =>
    Math.min(SPLIT.most, Math.max(SPLIT.least, Math.round(split ?? lastSplit ?? SPLIT.start))),
  );
  const settled = (layout: Layout, meta: { isUserInteraction: boolean }) => {
    // The layout is reported on mount too; only a drag or a key is the operator's.
    const tree = layout[TREE];
    if (!meta.isUserInteraction || typeof tree !== "number") return;
    lastSplit = Math.round(tree);
    onSplit?.(lastSplit);
  };
  // A line jumped to is in the text, so a markdown file shows its source, as its own tab does.
  const showsSource =
    source !== undefined && source.path === picked ? source.on : line !== undefined;

  return (
    <Group className="piece-files" orientation="horizontal" onLayoutChanged={settled}>
      <Panel
        id={TREE}
        defaultSize={`${started}%`}
        minSize={`${SPLIT.least}%`}
        maxSize={`${SPLIT.most}%`}
      >
        <BranchTree
          plane={plane}
          place={cut}
          picked={picked}
          reveal={landed?.at}
          onPick={setPicked}
          onPress={onPress}
        />
      </Panel>
      <Separator className="piece-files-split" aria-label="Resize the file tree" />
      <Panel id="piece-files-preview" minSize={`${100 - SPLIT.most}%`}>
        <section className="piece-files-shown" aria-label={picked ?? "No file picked"}>
          {picked === undefined || read === undefined ? (
            <EmptyState
              mark={FileText}
              headline="Pick a file to read it"
              body="Arrows move through the tree, Right opens a folder, and Enter shows a file here."
              variant="panel"
            />
          ) : (
            <>
              <header {...editorToolbar("piece-files-head")}>
                <ToolbarFile path={picked} />
                <span className="piece-files-actions">
                  <SourceToggle
                    path={picked}
                    read={read}
                    source={showsSource}
                    onSource={(on) => setSource({ path: picked, on })}
                    pages
                  />
                  <ToYourEditor plane={plane} cut={cut} path={picked} line={at.line} />
                  <CopyAndReveal plane={plane} cut={cut} path={picked} />
                  <Button
                    size="sm"
                    onPress={() =>
                      onOpenView(pieceFileView(cut, picked), pieceFileTitle(cut, picked))
                    }
                  >
                    Open in a tab of its own
                  </Button>
                  {changed !== undefined && (
                    <ShowWhatChanged cut={cut} path={picked} onOpenView={onOpenView} />
                  )}
                  <ToAChat plane={plane} cut={cut} path={picked} lines={selection.lines} />
                  <PinPageButton cut={cut} path={picked} read={read} onPress={onPress} />
                </span>
              </header>
              <Shown
                pageAt={{ plane, cut }}
                path={picked}
                read={read}
                line={line}
                column={column}
                landing={landed !== undefined && landed.path === picked ? landed.at : undefined}
                onLine={at.moved}
                onLines={selection.selected}
                source={showsSource}
              />
            </>
          )}
        </section>
      </Panel>
    </Group>
  );
}

/**
 * **An `.html` page, rendered privately** (#5953, spec story 15): the page alone, in a native
 * webview of its own with no grant, no network and no script, as a pinned artifact is
 * (`ArtifactTab`). Nothing is pinned by showing it; its text is under Source.
 */
function PagePreview({ plane, cut, path }: { plane: PlaneId; cut: Place; path: string }) {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const page = { workspace: cut.workspace, repo: cut.repo, piece: cut.piece, path };
  return (
    <div className="piece-page" data-testid="piece-page">
      <ArtifactFrame
        name={name}
        opens={`${plane}\u0000${cut.workspace}\u0000${cut.repo}\u0000${cut.piece ?? ""}\u0000${path}`}
        open={(first, theme) => commands.artifactPreviewOpen(plane, page, first, theme)}
      />
    </div>
  );
}

/**
 * **Pin to workspace…, beside an `.html` page** (#5953): the row its menu carries
 * (`pinToWorkspaceRow`), as a button. People, not a pin, mark it: it is the team's pin, never a
 * tab's own.
 */
function PinPageButton({
  cut,
  path,
  read,
  onPress,
}: {
  cut: Place;
  path: string;
  read: FileRead;
  onPress?: (offer: Offer) => void;
}) {
  if (onPress === undefined || read.kind !== "text" || !isPage(path)) return null;
  const row = pinToWorkspaceRow({ ...cut, path });
  return (
    <Button
      size="sm"
      isDisabled={!row.available}
      tooltip={row.available ? undefined : row.reason}
      onPress={() => onPress(row)}
      data-pin-to-workspace=""
    >
      <Users aria-hidden="true" />
      {row.title}
    </Button>
  );
}
