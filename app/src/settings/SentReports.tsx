// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type SentReport } from "../bindings";
import { CodeBlock } from "../ui/code/CodeBlock";
import { Dialog } from "../ui/dialog/Dialog";
import { EmptyState } from "../ui/empty-state/EmptyState";
import { List, ListRow } from "../ui/list/List";
import { RowDescription, RowLabel } from "../ui/list/Row";
import type { RowIds } from "./components";

/**
 * **The sent list** (#5689, spec #5683, Q13): every report this machine sent in the last 90
 * days, newest first, each opening exactly the bytes that left. The core keeps it
 * (`<config>/reports-sent.json`, `purlis_core::report::SentList`); the window only reads
 * it. It is the local viewer ruling O9 asks for: a real viewer, not a summary.
 */
export function SentReports({ ids }: { ids: RowIds }) {
  const sent = useSentReports();
  const [open, setOpen] = useState<SentReport | undefined>(undefined);
  return (
    <div id={ids.id} role="group" aria-labelledby={ids.labelledBy}>
      {sent.status === "error" ? (
        <EmptyState
          variant="panel"
          headline="The reports this machine sent could not be read."
          body={sent.error}
        />
      ) : sent.reports.length === 0 ? (
        <EmptyState variant="panel" headline="No report has been sent from this machine." />
      ) : (
        <List
          aria-label="Reports sent from this machine"
          onAction={(key) => setOpen(sent.reports[Number(key)])}
        >
          {sent.reports.map((one, at) => (
            <ListRow key={at} id={at} textValue={one.title}>
              <RowLabel>{one.title}</RowLabel>
              <RowDescription>
                {one.to} · {when(one.at)}
              </RowDescription>
            </ListRow>
          ))}
        </List>
      )}
      {open !== undefined && (
        <Dialog
          title={open.title}
          description={`Exactly what was sent to ${open.to}, ${when(open.at)}.`}
          size="wide"
          onClose={() => setOpen(undefined)}
          cancel={{ children: "Close" }}
        >
          <CodeBlock label="What was sent" text={open.bytes} />
        </Dialog>
      )}
    </div>
  );
}

type Read =
  | { status: "ok"; reports: readonly SentReport[] }
  | { status: "error"; error: string; reports: readonly SentReport[] };

/** The sent list, read from the core when the row is drawn. Empty until it answers. */
function useSentReports(): Read {
  const [read, setRead] = useState<Read>({ status: "ok", reports: [] });
  useEffect(() => {
    let live = true;
    void commands.reportsSent().then((said) => {
      if (!live) return;
      setRead(
        said.status === "ok"
          ? // Only the core's own shape is a list: anything else reads as nothing sent.
            { status: "ok", reports: Array.isArray(said.data) ? said.data : [] }
          : { status: "error", error: said.error, reports: [] },
      );
    });
    return () => {
      live = false;
    };
  }, []);
  return read;
}

/** When a report left, in this machine's own words for a date and a time. */
function when(at: string): string {
  const date = new Date(at);
  return Number.isNaN(date.getTime())
    ? at
    : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}
