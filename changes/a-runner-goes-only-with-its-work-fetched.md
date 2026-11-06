### Fixed

- **A runner goes only with its work fetched.** A draining runner stays until every project that ran chats on it has fetched from it, and a project whose window is closed fetches once it opens. If the fetch can't finish the runner stays, and the log says which project it waits on.
- **A fresh runner's check no longer holds up other chats' branches coming back.** The check runs outside the one-at-a-time fetch, the background sweep asks a runner whose check took minutes far less often, and Fetch again still asks at once. A check that took long looks again before the runner goes. Opening a task on a very large kept runner now says what to do if its check is too slow, and a failed release after a Discard no longer says "Its work is all here" (#5393).
