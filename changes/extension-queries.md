### Added

- **An extension can answer an artifact's queries.** Under the new `queries` capability
  (protocol 7), an extension declares queries with their parameters, scope and freshness, and a
  page reads them as `<extension-id>.<name>@1`. purlis answers from the extension's facts file
  while the answer there is fresh, otherwise asks its program, and asks again on its freshness
  while a page subscribes. A teammate without the extension sees the page with that part empty,
  and the bar says which extension it needs. Not on Windows (ADR 0122, #5971).
