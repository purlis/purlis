### Fixed

- A login no longer refuses every sign-in when a page held still has frames from other sites that run in processes of their own. Each frame is now watched through the connection that owns it, and those frames are asked to pause too. A frame that closes while this happens counts as stopped.
