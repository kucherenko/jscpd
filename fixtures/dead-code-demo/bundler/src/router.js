// Every page is loaded by name. The bundler turns this into one chunk per
// file under ./pages/, so each of them is reachable although no file names it.
export async function openPage(name) {
  const loaded = await import(`./pages/${name}.vue`);
  return loaded.default;
}

export function preloadPage(name) {
  return import(`./pages/${name}.vue`);
}
