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
 *   2. `NO_COLOR` (force off) / `FORCE_COLOR` (force on) environment variables;
 *   3. TTY detection of the output stream;
 *   4. default (no color when the stream is not a TTY).
 *
 * See https://no-color.org and the widely used `FORCE_COLOR` convention.
 */
export function shouldEnableColors(input: ColorResolutionInput): boolean {
  const {colors: override, env = {}, isTTY = false} = input;

  // 1. explicit CLI/config override wins over everything else.
  if (typeof override === 'boolean') {
    return override;
  }

  // 2. environment variables. `NO_COLOR` disables when set to any non-empty
  // value; `FORCE_COLOR` enables unless it is explicitly turned off.
  if (typeof env.NO_COLOR === 'string' && env.NO_COLOR !== '') {
    return false;
  }
  if (typeof env.FORCE_COLOR === 'string' && env.FORCE_COLOR !== '') {
    return env.FORCE_COLOR !== '0' && env.FORCE_COLOR !== 'false';
  }

  // 3./4. fall back to TTY detection.
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
