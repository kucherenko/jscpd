// cli-table3 colours its header and borders through its own `@colors/colors`
// dependency, which is a different singleton from the `colors/safe` one every
// jscpd reporter styles with. Disabling colors there has no effect on the
// table, so the styles have to be stripped explicitly instead.
//
// `colors/safe` is pulled in with `require` on purpose: the published bundle
// copies namespace imports into a plain object, which snapshots `enabled` at
// load time and would keep reporting the value from before `configureColors()`
// ran. The module object returned by `require` stays live.
const colors = require('colors/safe');

const DEFAULT_HEAD = ['red'];
const DEFAULT_BORDER = ['grey'];

/**
 * cli-table3 `style` entry matching the current color preference: the library
 * defaults while colors are on, no styling at all once they are off.
 */
export function getTableStyle(): {head: string[]; border: string[]} {
	return colors.enabled
		? {head: [...DEFAULT_HEAD], border: [...DEFAULT_BORDER]}
		: {head: [], border: []};
}
