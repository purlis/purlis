### Fixed

- A chat's `git push` is no longer refused with "the remote could not be asked which commits it already has" when the chat reaches its forge through purlis's forge route. The pre-push scan now asks purlis for the remote's branches and tags, and purlis asks the forge from outside the chat's sandbox, on the chat's own route and only for a repository recorded for that chat. Where purlis cannot answer, the scan reads the push as it always has. A push the app makes that fails now says why even when git said nothing.
