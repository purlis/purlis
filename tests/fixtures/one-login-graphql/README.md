# GraphQL documents the forge CLIs send

What `gh` and `glab` ask the forge's GraphQL API for, one document per command, replayed by
`crates/purlis-core/tests/it/a_chats_forge_proxy_answers_the_graphql_gh_and_glab_send.rs` against
the proxy's per-type field allowlist (ADR 0128 D3e, review item M2). Every document must be
answered.

**Provenance.** These are transcribed from the query builders in `gh`'s source
(`api/queries_pr.go`, `api/queries_issue.go`, `api/queries_repo.go`, `api/query_builder.go`, the
release commands) as shipped in the 2.103 and 2.105 lines the spike pinned, and from the documents
`glab api graphql` callers write. They were **not** captured off a wire, because the sandbox the
slice was written in cannot bind a socket. The real-clients test of the same slice runs the real
tools through the listener on a CI runner and fails if the proxy refuses what a tool sends; with
`PURLIS_CAPTURE_GRAPHQL=<dir>` set it also writes every document it saw, which is how these are
re-captured when a `gh` bump changes a query.
