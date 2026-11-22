### Fixed

- The pre-push secret scan trusts only the app's own certificate file, and only from a folder a chat cannot write, so a chat cannot answer the scan's check of the remote with an authority of its own and have its commits skip the scan.
