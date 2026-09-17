// Vite's own spelling of the same idea: every file the pattern matches is
// bundled, so every locale is live.
const catalogs = import.meta.glob('./locales/*.js', { eager: true });

export function translate(key, locale = 'en') {
  const catalog = catalogs[`./locales/${locale}.js`];
  return catalog?.messages[key] ?? key;
}
