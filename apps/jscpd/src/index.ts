export { detectClones } from './detect';

export async function jscpd(argv: string[], exitCallback?: (code: number) => void) {
  // Check if 'server' command is present in argv (skip first 2 elements: node and script path)
  const isServerMode = argv.slice(2).includes('server');

  if (isServerMode) {
    const { runServer } = await import('./server-entry');
    return runServer(argv, exitCallback);
  }

  const { runCli } = await import('./cli-entry');
  return runCli(argv, exitCallback);
}

