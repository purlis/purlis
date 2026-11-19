### Fixed

- The merge hold no longer holds editing a request's title or description through the API (`gh api -X PATCH repos/<o>/<r>/pulls/<n> --input body.json`, `glab api -X PUT projects/<id>/merge_requests/<n> …`). It still holds an edit that sets the target branch (`base`, `target_branch`) or a merge setting, and one whose body purlis cannot read (a file it cannot open, stdin, a path the shell fills in).
