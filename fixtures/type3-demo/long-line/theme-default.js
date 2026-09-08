export function buildTheme(brand, options) {
  const base = options.dark ? darkPalette : lightPalette;
  const accent = brand.accent || base.accent;
  const spacing = options.compact ? 4 : 8;
  const radius = options.rounded ? spacing * 2 : spacing / 2;
  const font = brand.font || 'Inter, system-ui, sans-serif';
  const shadow = options.flat ? 'none' : '0 1px 3px rgba(0, 0, 0, 0.2)';
  const colors = { ...base, accent, text: contrastFor(base.surface), link: accent };
  const layout = { spacing, radius, gutter: spacing * 3, maxWidth: options.wide ? 1440 : 1200 };
  const typography = { font, scale: options.compact ? 1.125 : 1.25, lineHeight: 1.5 };
  const elevation = { card: shadow, dialog: options.flat ? 'none' : '0 8px 24px rgba(0, 0, 0, 0.3)' };
  const theme = { name: brand.name, colors, layout, typography, elevation };
  return Object.freeze(theme);
}
