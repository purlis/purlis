// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState, type ReactNode, type Ref } from "react";
import { Button } from "../ui/button/Button";
import { HostStatus } from "../ui/badge/Badge";
import { ListFilter, noMatchSaid, useListFilter } from "../ui/list/ListFilter";
import {
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
  type Selection,
} from "../ui/table/Table";
import { TableBar } from "../ui/table/TableBar";
import { agoSaid } from "../ages";
import type { CollectionEntry, HostAdded } from "./groups";

const HOST = { one: "host", many: "hosts" };

/** A host as the file writes it, and the port it reaches: its own, else HTTPS's (R65). */
export function hostAndPort(written: string): { host: string; port: string } {
  const at = written.lastIndexOf(":");
  const before = written.slice(0, at);
  // A port follows a name or a bracketed address; a bare IPv6 address's last part is no port.
  const ported =
    at > 0 &&
    /^\d+$/.test(written.slice(at + 1)) &&
    (!before.includes(":") || before.endsWith("]"));
  return ported ? { host: before, port: written.slice(at + 1) } : { host: written, port: "443" };
}

/** A host's own words, as its file writes them: what the table shows and finds it by. */
export function hostOf(entry: CollectionEntry): string {
  return entry.host ?? entry.label;
}

/**
 * **When, and by what, a host was added** (#6172, R65): "‹when› · ‹by›", by "you" (Settings' Add)
 * or "a chat's block" (its Allow), the when an age (`agoSaid`). A host added before the core kept
 * the record says it was added earlier, never a guess.
 */
export function addedSaid(added: HostAdded | undefined, now: Date): string {
  if (added === undefined) return "Earlier";
  const when = agoSaid(Math.floor(now.getTime() / 1000) - added.at, now);
  return `${when} · ${added.by === "you" ? "you" : "a chat's block"}`;
}

/**
 * **Your hosts, as a table** (#2919, prototype A of `prototype/settings-page`, R65, R67–R74): the
 * table family's, in its rounded frame, which keeps its least width and scrolls sideways. A row
 * per host, the ones waiting for your Confirm first (R68): a check, the host in mono with its
 * port in the quiet words, its status, when and by what it was added (`addedSaid`), and at its
 * end Confirm (only while it waits) and Remove,
 * in columns of their own so they stand in one place on every row (R67). Over it the bar: the
 * filter, by any part of a host, from eight hosts (R69, R164); then Confirm all and Add host, or,
 * while rows are chosen, how many, Confirm and Remove (R70); the head's check chooses every row
 * or none (R71). A filter that leaves none says so in the table (R73).
 */
export function HostsTable({
  label,
  entries,
  busy,
  onConfirm,
  onRemove,
  onConfirmAll,
  onRemoveAll,
  onAdd,
  addRef,
  now = new Date(),
}: {
  /** What the table and its bar are called: the page's name. */
  label: string;
  entries: readonly CollectionEntry[];
  /** A write is on its way: every act waits for it. */
  busy: boolean;
  onConfirm: (entry: CollectionEntry) => void;
  onRemove: (entry: CollectionEntry) => void;
  /** Confirms each of `hosts`, waiting, in turn: `every` waiting host (Confirm all), or the
   *  rows chosen. */
  onConfirmAll: (hosts: readonly string[], every: boolean) => void;
  /** Removes each of `hosts` in turn. */
  onRemoveAll: (hosts: readonly string[]) => void;
  onAdd: () => void;
  /** Add host, where the keyboard comes back to from the form it opens. */
  addRef: Ref<HTMLButtonElement>;
  /** The moment each Added age is told from: now, unless a story holds it still. */
  now?: Date;
}) {
  const { query, setQuery, shown, filtered } = useListFilter(entries, hostOf);
  const [chosen, setChosen] = useState<Selection>(new Set());
  const rows = useMemo(
    () => [
      ...shown.filter((one) => one.status === "waiting"),
      ...shown.filter((one) => one.status !== "waiting"),
    ],
    [shown],
  );
  // What is chosen and still on screen: a host the filter or a write took away is not acted on.
  const picked = rows.filter((one) => chosen === "all" || chosen.has(hostOf(one)));
  const waiting = entries.filter((one) => one.status === "waiting");
  const done = (then: () => void) => {
    setChosen(new Set());
    then();
  };
  let empty: ReactNode = "No host of your own yet. Add host adds one.";
  if (query.trim() !== "") empty = noMatchSaid(HOST, query);
  return (
    <div className="ui-hosts">
      <TableBar
        label={label}
        filter={
          filtered ? (
            <ListFilter noun={HOST} total={entries.length} query={query} onQuery={setQuery} />
          ) : undefined
        }
        chosen={picked.length}
        bulk={
          <>
            <Button
              isDisabled={busy || !picked.some((one) => one.status === "waiting")}
              onPress={() =>
                done(() =>
                  onConfirmAll(picked.filter((one) => one.status === "waiting").map(hostOf), false),
                )
              }
            >
              Confirm
            </Button>
            <Button
              variant="ghost"
              isDisabled={busy}
              onPress={() => done(() => onRemoveAll(picked.map(hostOf)))}
            >
              Remove
            </Button>
          </>
        }
        actions={
          <>
            {waiting.length > 0 && (
              <Button
                variant="primary"
                isDisabled={busy}
                onPress={() => onConfirmAll(waiting.map(hostOf), true)}
              >
                {`Confirm all ${waiting.length} waiting`}
              </Button>
            )}
            <Button ref={addRef} onPress={onAdd}>
              Add host
            </Button>
          </>
        }
      />
      <Table
        aria-label={label}
        selectionMode="multiple"
        selectedKeys={chosen}
        onSelectionChange={setChosen}
      >
        <TableHeader>
          {/* The host is what gives way where the page is narrow, cut with its whole words in the
              cell's tooltip, so the Added column never pushes Remove out of the frame (#6172). */}
          <TableColumn id="host" isRowHeader size="fill" cut>
            Host
          </TableColumn>
          <TableColumn id="status">Status</TableColumn>
          <TableColumn id="added" size="fit">
            Added
          </TableColumn>
          <TableColumn id="confirm" align="end" hideName>
            Confirm
          </TableColumn>
          <TableColumn id="remove" align="end" hideName>
            Remove
          </TableColumn>
        </TableHeader>
        <TableBody empty={empty}>
          {rows.map((one) => {
            const written = hostOf(one);
            const { host, port } = hostAndPort(written);
            return (
              <TableRow key={written} id={written} textValue={written}>
                <TableCell>
                  <code className="ui-hosts-host">
                    {host}
                    <span className="ui-hosts-port">{`:${port}`}</span>
                  </code>
                </TableCell>
                <TableCell>
                  <HostStatus waiting={one.status === "waiting"} />
                </TableCell>
                <TableCell>
                  <span className="ui-hosts-added">{addedSaid(one.added, now)}</span>
                </TableCell>
                <TableCell align="end">
                  {one.status === "waiting" && (
                    <Button
                      size="sm"
                      isDisabled={busy}
                      aria-label={`Confirm ${written}`}
                      onPress={() => onConfirm(one)}
                    >
                      Confirm
                    </Button>
                  )}
                </TableCell>
                <TableCell align="end">
                  <Button
                    variant="ghost"
                    size="sm"
                    isDisabled={busy}
                    aria-label={`Remove ${written}`}
                    onPress={() => onRemove(one)}
                  >
                    Remove
                  </Button>
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </div>
  );
}
