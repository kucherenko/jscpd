export function buildTheme(brand, options) {
  const base = options.dark ? darkPalette : lightPalette;
  const accent = brand.accent || base.accent;
  const spacing = options.compact ? 4 : 8;
  const radius = options.rounded ? spacing * 2 : spacing / 2;
  const font = brand.font || 'Inter, system-ui, sans-serif';
  const shadow = options.flat ? 'none' : '0 1px 3px rgba(0, 0, 0, 0.2)';
  const swatches = ['#002878', '#2583ad', '#4adee2', '#6f3917', '#94944c', '#b9ef81', '#de4ab6', '#03a5eb', '#280020', '#4d5b55', '#72b68a', '#9711bf', '#bc6cf4', '#e1c729', '#06225e', '#2b7d93', '#50d8c8', '#7533fd', '#9a8e32', '#bfe967', '#e4449c', '#099fd1', '#2efa06', '#53553b', '#78b070', '#9d0ba5', '#c266da', '#e7c10f', '#0c1c44', '#317779', '#56d2ae', '#7b2de3', '#a08818', '#c5e34d', '#ea3e82', '#0f99b7', '#34f4ec', '#594f21', '#7eaa56', '#a3058b', '#c860c0', '#edbbf5', '#12162a', '#37715f', '#5ccc94', '#8127c9', '#a682fe', '#cbdd33', '#f03868', '#15939d', '#3aeed2', '#5f4907', '#84a43c', '#a9ff71', '#ce5aa6', '#f3b5db', '#181010', '#3d6b45', '#62c67a', '#8721af', '#ac7ce4', '#d1d719', '#f6324e', '#1b8d83', '#40e8b8', '#6543ed', '#8a9e22', '#aff957', '#d4548c', '#f9afc1', '#1e0af6', '#43652b', '#68c060', '#8d1b95', '#b276ca', '#d7d1ff', '#fc2c34', '#218769', '#46e29e', '#6b3dd3', '#909808', '#b5f33d', '#da4e72', '#ffa9a7', '#2404dc', '#495f11', '#6eba46', '#93157b', '#b870b0', '#ddcbe5', '#02261a', '#27814f', '#4cdc84', '#7137b9', '#9692ee', '#bbed23', '#e04858', '#05a38d', '#2afec2', '#4f59f7', '#74b42c', '#990f61', '#be6a96', '#e3c5cb', '#082000', '#2d7b35', '#52d66a', '#77319f', '#9c8cd4', '#c1e709', '#e6423e', '#0b9d73', '#30f8a8', '#5553dd', '#7aae12', '#9f0947', '#c4647c', '#e9bfb1', '#0e1ae6', '#33751b', '#58d050', '#7d2b85', '#a286ba', '#c7e1ef', '#ec3c24', '#119759', '#36f28e', '#5b4dc3', '#80a8f8', '#a5032d', '#ca5e62', '#efb997', '#1414cc', '#396f01', '#5eca36', '#83256b', '#a880a0', '#cddbd5', '#f2360a', '#17913f', '#3cec74', '#6147a9', '#86a2de', '#abfd13', '#d05848', '#f5b37d', '#1a0eb2', '#3f69e7', '#64c41c', '#891f51'];
  const colors = { ...base, accent, text: contrastFor(base.surface), link: accent };
  const layout = { spacing, radius, gutter: spacing * 3, maxWidth: options.wide ? 1440 : 1200 };
  const typography = { font, scale: options.compact ? 1.125 : 1.25, lineHeight: 1.5 };
  const elevation = { card: shadow, dialog: options.flat ? 'none' : '0 8px 24px rgba(0, 0, 0, 0.3)' };
  const theme = { name: brand.name, colors, layout, typography, elevation };
  return Object.freeze(theme);
}
