// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **A mock of `purlis.port`, for a page developed outside purlis** (`purlis artifact mock`,
// ADR 0122 §7, #5975). Load it before the page's own script:
//
//   <script src="purlis-page-mock.js"></script>
//
// and serve the folder from any local web server. It answers as purlis does, in the same
// envelope, `{ id, data | error: { kind }, at, cursor? }`, under the page's own declaration
// (`<script type="application/json" id="purlis">`): anything undeclared is `not-approved`.
//
// - **A data file** is answered from `purlis-fixtures.json`'s `data`, else fetched from the
//   page's folder, else `null`, as purlis answers a file not generated on this machine.
// - **A query** is answered from `purlis-fixtures.json`'s `queries`, keyed `<id>@<N>`, each an
//   envelope without `at`: `{ "data": …, "cursor": "…" }` or
//   `{ "error": { "kind": "…", "message": "…" } }`, the message optional.
//   With no fixture it fails, and says so in the console.
// - **Offers** are checked as purlis checks them and written to the console; nothing is done.
//
// It does not know purlis's catalogue: `purlis artifact check <folder>` says whether each
// query the page declares is one purlis answers. **In purlis it does nothing**: purlis's own
// port is there first.
(() => {
  // purlis defines its port as the window's own property; `window.purlis` alone may be the
  // declaration block itself, which a browser names on the window by its id.
  if (Object.hasOwn(window, "purlis")) return;

  const API = 1;
  const FIXTURES = "purlis-fixtures.json";
  const KINDS = ["needs-extension", "not-approved", "timeout", "too-large", "refused", "failed"];
  const MOST_OFFERS = 8;

  const at = () => new Date().toISOString().replace(/\.\d+Z$/, "Z");
  const said = (why) => console.warn("purlis mock: " + why);
  const own = (object, key) =>
    object !== null && typeof object === "object" && Object.hasOwn(object, key);

  // A data file's name as purlis reads one: data.json, or a file under data/, every segment plain.
  const isDataName = (name) =>
    typeof name === "string" &&
    (name === "data.json" || (name.startsWith("data/") && name.endsWith(".json"))) &&
    !/[\\%\u0000-\u001f\u007f]/.test(name) &&
    name.split("/").every((segment) => segment !== "" && !segment.startsWith("."));

  /** The page's declaration as purlis reads it, or null when it declares nothing. */
  const declaration = () => {
    const block = document.getElementById("purlis");
    if (block === null) return null;
    let doc;
    try {
      doc = JSON.parse(block.textContent);
    } catch {
      doc = null;
    }
    if (doc === null || typeof doc !== "object" || Array.isArray(doc)) {
      said('its <script id="purlis"> block is not a JSON object, so the page reads nothing');
      return null;
    }
    if (!Number.isSafeInteger(doc.api) || doc.api < 1 || doc.api > API) {
      said(`it declares no "api" this purlis answers (1 to ${API}), so the page reads nothing`);
      return null;
    }
    const data = doc.data ?? [];
    if (!Array.isArray(data) || !data.every(isDataName)) {
      said("only data.json or a file under data/ is a data file, so the page reads nothing");
      return null;
    }
    const queries = doc.queries ?? {};
    if (typeof queries !== "object" || queries === null || Array.isArray(queries)) {
      said('its "queries" is not an object of id@N to parameter names, so the page reads nothing');
      return null;
    }
    for (const [key, params] of Object.entries(queries)) {
      const version = /^.+@([1-9][0-9]*)$/.exec(key);
      if (version === null || Number(version[1]) > API) {
        said(
          `it declares ${key}, and this purlis answers id@1 to id@${API}, so the page reads nothing`,
        );
        return null;
      }
      if (!Array.isArray(params) || !params.every((p) => typeof p === "string")) {
        said(`its parameters of ${key} are not a list of names, so the page reads nothing`);
        return null;
      }
    }
    return { data, queries };
  };

  let fixtures = null;
  const loadFixtures = () =>
    (fixtures ??= fetch(FIXTURES)
      .then((r) => (r.ok ? r.json() : {}))
      .catch((e) => {
        said(`${FIXTURES} is not JSON (${e}), so nothing is answered from it`);
        return {};
      })
      .then((f) => (f !== null && typeof f === "object" ? f : {})));

  const read = async (declared, name) => {
    if (!declared.data.includes(name)) return { error: { kind: "not-approved" } };
    const from = await loadFixtures();
    if (own(from.data, name)) return { data: from.data[name] };
    let response;
    try {
      response = await fetch(name);
    } catch {
      return { error: { kind: "failed" } };
    }
    if (response.status === 404) return { data: null };
    if (!response.ok) return { error: { kind: "failed" } };
    try {
      return { data: await response.json() };
    } catch {
      return { error: { kind: "failed" } };
    }
  };

  const query = async (declared, key, params) => {
    if (!own(declared.queries, key)) return { error: { kind: "not-approved" } };
    if (params !== undefined && params !== null) {
      if (typeof params !== "object" || Array.isArray(params))
        return { error: { kind: "refused" } };
      if (!Object.keys(params).every((p) => declared.queries[key].includes(p)))
        return { error: { kind: "not-approved" } };
    }
    const from = await loadFixtures();
    if (!own(from.queries, key)) {
      said(`no fixture for ${key}: add one to ${FIXTURES}'s "queries"`);
      return { error: { kind: "failed" } };
    }
    const fixture = from.queries[key];
    if (own(fixture, "error")) {
      const kind = fixture.error?.kind;
      const error = { kind: KINDS.includes(kind) ? kind : "failed" };
      if (typeof fixture.error?.message === "string") error.message = fixture.error.message;
      return { error };
    }
    const answer = { data: own(fixture, "data") ? fixture.data : null };
    if (typeof fixture?.cursor === "string") answer.cursor = fixture.cursor;
    return answer;
  };

  // An offer as purlis takes one: a row's dotted lowercase verb and one line naming its subject.
  const isOffer = (offer) =>
    offer !== null &&
    typeof offer === "object" &&
    Object.keys(offer).length === 2 &&
    typeof offer.does === "string" &&
    typeof offer.subject === "string" &&
    offer.does.length <= 64 &&
    offer.does.includes(".") &&
    offer.does.split(".").every((word) => /^[a-z][a-z0-9-]*$/.test(word)) &&
    offer.subject !== "" &&
    [...offer.subject].length <= 256 &&
    !/[\u0000-\u001f\u007f-\u009f]/.test(offer.subject);

  const offer = (offers) => {
    const seen = new Set(offers.map((o) => (isOffer(o) ? o.does + ":" + o.subject : null)));
    if (offers.length > MOST_OFFERS || !offers.every(isOffer) || seen.size !== offers.length)
      return { error: { kind: "refused" } };
    console.info(
      "purlis mock: the page offers",
      offers.map((o) => o.does + ":" + o.subject),
    );
    return { data: { offered: offers.length } };
  };

  const channel = new MessageChannel();
  const port = channel.port1;
  port.onmessage = async (event) => {
    const asked = event.data;
    const id = asked !== null && typeof asked === "object" ? asked.id : undefined;
    const answer = (envelope) =>
      port.postMessage({ id: Number.isSafeInteger(id) ? id : null, ...envelope, at: at() });
    if (
      !Number.isSafeInteger(id) ||
      id < 0 ||
      (typeof asked.read !== "string" &&
        typeof asked.query !== "string" &&
        !Array.isArray(asked.offers))
    ) {
      answer({ error: { kind: "refused" } });
      return;
    }
    if (Array.isArray(asked.offers)) {
      answer(offer(asked.offers));
      return;
    }
    const declared = declaration();
    if (declared === null) {
      answer({ error: { kind: "not-approved" } });
      return;
    }
    answer(
      typeof asked.query === "string"
        ? await query(declared, asked.query, asked.params)
        : await read(declared, asked.read),
    );
  };
  Object.defineProperty(window, "purlis", {
    value: Object.freeze({ api: API, port: channel.port2 }),
  });
})();
