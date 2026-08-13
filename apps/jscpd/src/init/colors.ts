// Color resolution moved to @jscpd/finder so jscpd-server can share it;
// re-exported here to keep this module's import path stable.
export {shouldEnableColors, configureColors} from '@jscpd/finder';
export type {ColorResolutionInput} from '@jscpd/finder';
