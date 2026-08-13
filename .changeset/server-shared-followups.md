---
"@jscpd/finder": minor
"jscpd": patch
"jscpd-server": patch
---

jscpd-server hardening follow-ups: server log colors now respect NO_COLOR/FORCE_COLOR and TTY detection via the shared `configureColors`; the consoleFull/ai double-print exclusion is shared with the CLI through `@jscpd/finder`'s new `registerSubscribers` (the server's copy lacked it); the REST Origin/Host guard now covers all `/api` routes by default, with `/api/health` exempt for load-balancer probes. `@jscpd/finder` newly exports `shouldEnableColors`, `configureColors`, and `registerSubscribers`.
