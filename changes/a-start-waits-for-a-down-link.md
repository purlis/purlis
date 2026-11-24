### Changed

- A chat start on a pool whose runner link is down now waits up to 30 seconds for the link to come back, for every kind of pool, instead of being refused at once on a Docker pool. After 30 seconds it ends with the reason, and the runner's notice offers Try now and Check pool.
- A start on a runner now stops after 3 minutes and says which step it was stuck on.
