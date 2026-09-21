import { readdir } from 'node:fs/promises';

export async function mountScreens(root) {
  const found = await readdir(root, { recursive: true });
  const screens = found.filter((name) => name.endsWith('.screen.js'));
  for (const name of screens) {
    const screen = await import(new URL(name, root).href);
    screen.default.mount(name.replace(/\.screen\.js$/, ''));
  }
  return screens.length;
}
