### Fixed

- The window answers faster with many chats open. Finished turns no longer parse the whole 30-day event log on every ask: they read the last day's files and keep each parsed until it changes. The project's sandbox state and the asks waiting are each read once per burst of plane changes instead of once per ask.
