export { detectClones } from './detect';

export async function jscpd(argv: string[], exitCallback?: (code: number) => {}) {
  const isServerMode = argv.includes('server');

  if (isServerMode) {
    const { runServer } = await import('./server-entry');
    return runServer(argv, exitCallback);
  }

  const { runCli } = await import('./cli-entry');
  return runCli(argv, exitCallback);
}

