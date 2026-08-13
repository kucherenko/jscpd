// Shared with the jscpd CLI via @jscpd/finder: registers verbose/progress
// subscribers and skips the progress announcer for reporters that print
// every clone themselves (ai, consoleFull) — previously this copy lacked
// that exclusion, so such reporters logged every clone twice.
export {registerSubscribers} from '@jscpd/finder';
