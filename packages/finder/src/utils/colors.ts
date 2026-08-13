import * as colors from 'colors/safe';

export interface ColorResolutionInput {
	/**
	 * Explicit override from CLI/config (`--colors` / `--no-colors` /
	 * `colors: true|false`). `undefined` means "auto-detect".
	 */
	colors?: boolean;
	env?: NodeJS.ProcessEnv;
	isTTY?: boolean;
}

/**
 * Decide whether ANSI colors should be emitted, following the precedence:
 *
 *   1. explicit CLI/config override (`--colors` / `--no-colors`);
 *   2. `FORCE_COLOR` (force on, unless `0`/`false`);
 *   3. `NO_COLOR` (force off);
 *   4. TTY detection of the output stream;
 *   5. default (no color when the stream is not a TTY).
 *
 * See https://no-color.org and the widely used `FORCE_COLOR` convention.
 * `FORCE_COLOR` is checked first so that opting back in works inside images
 * that export `NO_COLOR` globally, matching Node core (`tty.getColorDepth`),
 * chalk, and `@colors/colors`.
 */
export function shouldEnableColors(input: ColorResolutionInput): boolean {
	const {colors: override, env = {}, isTTY = false} = input;

	// 1. explicit CLI/config override wins over everything else.
	if (typeof override === 'boolean') {
		return override;
	}

	// 2. `FORCE_COLOR` enables unless it is explicitly turned off. An empty
	// value counts as force-on, as in chalk and `@colors/colors`.
	if (typeof env.FORCE_COLOR === 'string') {
		return env.FORCE_COLOR !== '0' && env.FORCE_COLOR !== 'false';
	}

	// 3. `NO_COLOR` disables when set to any non-empty value.
	if (typeof env.NO_COLOR === 'string' && env.NO_COLOR !== '') {
		return false;
	}

	// 4./5. fall back to TTY detection.
	return isTTY;
}

/**
 * Apply the resolved color preference to the shared `colors` singleton so that
 * every reporter using `colors/safe` respects it. Reads `process.stdout.isTTY`
 * and the environment when no explicit override is provided.
 */
export function configureColors(
	options: {colors?: boolean} = {},
	env: NodeJS.ProcessEnv = process.env,
	isTTY: boolean = Boolean(process.stdout && process.stdout.isTTY),
): boolean {
	const enabled = shouldEnableColors({colors: options.colors, env, isTTY});
	if (enabled) {
		colors.enable();
	} else {
		colors.disable();
	}
	return enabled;
}
