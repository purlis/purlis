// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The pages in `docs/` link the way GitHub reads them: relative paths to other `.md` files and
// to files anywhere in the repository. On the site, a link to a page in `docs/` goes to that
// page, and a link to anything else goes to the file on the forge at the ref the site was built
// from. A relative link to a path that does not exist fails the build, so the broken-link check
// covers links out of `docs/` too, which starlight-links-validator does not see.
//
// A picture is read the same way: a page shows a story's screenshot baseline by its path in the
// repository, as GitHub shows it, and the site shows the same file from the forge at the built
// ref, so no picture is copied into the site and none can go stale there.
import { existsSync, statSync } from "node:fs";
import { posix, relative, resolve, sep } from "node:path";
import { REPO_URL, pageHref } from "./pages.mjs";

const SCHEME = /^[a-z][a-z0-9+.-]*:/i;

/** The repository path `target`, written in `page`, names, and what follows it (`?…`, `#…`). */
function inRepository(target, { page, repoRoot }) {
  const cut = target.search(/[?#]/);
  const path = cut === -1 ? target : target.slice(0, cut);
  const rest = cut === -1 ? "" : target.slice(cut);
  const resolved = posix
    .normalize(path.startsWith("/") ? path.slice(1) : posix.join(posix.dirname(page), path))
    .replace(/\/$/, "");
  const outside = resolved === ".." || resolved.startsWith("../");
  const onDisk = resolve(repoRoot, resolved);
  if (outside || !existsSync(onDisk)) {
    throw new Error(`${page}: the link to ${target} leads to ${resolved}, which does not exist`);
  }
  return { resolved, rest, directory: statSync(onDisk).isDirectory() };
}

/** Where `target`, written in the repository file `page`, leads on the site. */
export function siteLink(target, { page, repoRoot, ref }) {
  if (SCHEME.test(target) || target.startsWith("#")) return target;
  const { resolved, rest, directory } = inRepository(target, { page, repoRoot });
  if (directory) return `${REPO_URL}/tree/${ref}/${resolved}${rest}`;
  if (resolved.startsWith("docs/") && resolved.endsWith(".md")) {
    return `${pageHref(resolved.slice("docs/".length))}${rest}`;
  }
  return `${REPO_URL}/blob/${ref}/${resolved}${rest}`;
}

/** Where the site shows the picture `target`, written in the repository file `page`, from. */
export function siteImage(target, { page, repoRoot, ref }) {
  if (SCHEME.test(target)) return target;
  const { resolved, rest, directory } = inRepository(target, { page, repoRoot });
  if (directory) throw new Error(`${page}: the picture ${target} leads to ${resolved}, which is not a file`);
  return `${REPO_URL}/raw/${ref}/${resolved}${rest}`;
}

function visit(node, fn) {
  fn(node);
  for (const child of node.children ?? []) visit(child, fn);
}

/**
 * The remark plugin. `contentRoot` is the content collection's directory; a page under it at
 * `docs/…` is the copy of the repository's `docs/…` that `scripts/sync-docs.mjs` made.
 */
export function remarkRepoLinks({ contentRoot, ...where }) {
  return (tree, file) => {
    const page = relative(contentRoot, file.path).split(sep).join("/");
    if (!page.startsWith("docs/")) return;
    visit(tree, (node) => {
      if (node.type === "link" || node.type === "definition") {
        node.url = siteLink(node.url, { page, ...where });
      } else if (node.type === "image") {
        node.url = siteImage(node.url, { page, ...where });
      }
    });
  };
}
