### Fixed
- A chat's merge refused because purlis was updated on disk under the running app now says so, "purlis was updated on disk but the running app is older: restart purlis, then run the same line again", and names a restart of purlis (Restart to update, or quit and reopen it), never a new chat, which the same app would start with the same binary; a refusal also ends the app's words with a full stop (#7177).

### Changed
- A forge CLI's JSON answer for a GitLab merge is read as its first value past a notice glab writes before or after it, with no pager in between, as hardening on top of #7003 (#7177).
