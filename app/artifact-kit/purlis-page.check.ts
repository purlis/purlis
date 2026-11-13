// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `purlis-page.d.ts` lets a page write, and what it refuses: checked by `npm run
// typecheck`, never run. A page that types against the generated file asks only as purlis
// answers, in the catalogue's shapes.

purlis.port.postMessage({ id: 1, read: "data.json" });
purlis.port.postMessage({ id: 2, query: "purlis.todo_list@1", params: { done: true } });
purlis.port.postMessage({ id: 3, query: "purlis.session_record_list@1" });
purlis.port.postMessage({ id: 4, query: "purlis.session_record_read@1", params: { file: "x.md" } });
purlis.port.postMessage({
  id: 10,
  query: "purlis.forge_milestones@1",
  params: { repo: "purlis", state: "all" },
});
purlis.port.postMessage({ id: 5, offers: [{ does: "todo.done", subject: "fix-login" }] });

// @ts-expect-error A query the catalogue does not have.
purlis.port.postMessage({ id: 6, query: "purlis.no_such@1" });
// @ts-expect-error A query asked without its version.
purlis.port.postMessage({ id: 7, query: "purlis.todo_list" });
// @ts-expect-error A parameter of the wrong type.
purlis.port.postMessage({ id: 8, query: "purlis.todo_list@1", params: { done: "yes" } });
// @ts-expect-error A parameter the query does not take.
purlis.port.postMessage({ id: 9, query: "purlis.change_status@1", params: { repo: "x" } });

purlis.port.onmessage = (event) => {
  const answer = event.data;
  if (answer.error !== undefined) {
    // @ts-expect-error An error kind purlis never answers.
    const never: PurlisPage.ErrorKind = "gone";
    const said: string | undefined = answer.error.message;
    void [answer.error.kind satisfies PurlisPage.ErrorKind, never, said];
    return;
  }
  const at: string = answer.at;
  void at;
};

const todos: PurlisPage.Queries["purlis.todo_list@1"]["data"] = {
  done: false,
  todos: [{ slug: "fix-login", title: "Fix login", opened: "" }],
};
const record: PurlisPage.Queries["purlis.session_record_read@1"]["data"] = null;
const api: 1 = purlis.api;

const fixtures: PurlisPage.Fixtures = {
  data: { "data.json": { rows: 2 } },
  queries: {
    "purlis.todo_list@1": { data: todos, cursor: "c1" },
    "purlis.change_status@1": { error: { kind: "needs-extension" } },
    "purlis.forge_milestones@1": {
      error: { kind: "failed", message: "purlis could not reach GitHub." },
    },
    // @ts-expect-error A fixture in a shape the query never answers.
    "purlis.session_record_list@1": { data: { records: "none" } },
  },
};
void [record, api, fixtures];
