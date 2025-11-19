export { detectClones } from './detect';

export async function jscpd(argv: string[], exitCallback?: (code: number) => void) {
  // Find first non-flag argument
  const firstCommand = argv.find(arg => !arg.startsWith('-'));
  const isServerMode = firstCommand === 'server';

  if (isServerMode) {
    const { runServer } = await import('./server-entry');
    return runServer(argv, exitCallback);
  }

  const { runCli } = await import('./cli-entry');
  return runCli(argv, exitCallback);
}

