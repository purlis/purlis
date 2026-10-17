### Fixed

- **An Allow for one chat that has closed is no longer recorded in the audit.** A block's Notice
  or Settings' port form allowing something for a chat that closed in the meantime is refused
  before anything is audited, where the audit used to record an Allow that was never kept. An
  Allow for one chat whose audit cannot be written keeps nothing, and allowing for the chat what
  was allowed once is now audited too (#2454).
