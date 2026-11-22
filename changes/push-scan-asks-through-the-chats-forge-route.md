### Fixed

- A chat's `git push` is no longer refused with "the remote could not be asked which commits it already has" when the chat reaches its forge through purlis's forge route. The pre-push scan now asks the remote through the chat's own route, and a push the app makes that fails now says why even when git said nothing.
