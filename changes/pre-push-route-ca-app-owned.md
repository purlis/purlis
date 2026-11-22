### Fixed

- The pre-push secret scan asks the remote through the chat's own route again: it trusts the app's copy of the chat's certificate in the chat's wire folder (the one file its sandbox lets it read), and only where neither that file nor any folder above it can be written or swapped by a chat, so a chat cannot answer the scan's check of the remote with an authority of its own and have its commits skip the scan.
