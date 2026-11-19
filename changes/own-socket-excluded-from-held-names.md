### Fixed

- **A wrapped chat serving a local port can make its own server socket again, under the full profile.** The deny on purlis's socket names and folders now leaves out the exact path of each socket the chat serves on, instead of relying on rule order; every other name, `other.sock` beside it and the folder itself stay held (round 7 of #7105).
