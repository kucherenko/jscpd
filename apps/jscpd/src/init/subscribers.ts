// Subscriber wiring moved to @jscpd/finder so jscpd-server shares the same
// double-print exclusion; re-exported here to keep the import path stable.
export {registerSubscribers} from '@jscpd/finder';
