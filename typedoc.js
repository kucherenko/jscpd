module.exports = {
  mode: 'modules',
  tsconfig: './packages/tsconfig.json',
  out: 'docs',
  exclude: [
    '**/node_modules/**',
    '**/*.test.ts',
    '**/__tests__/**',
    '**/publish-util.ts'
  ],
  name: 'jscpd - copy/paste detector',
  excludePrivate: false,
  readme: 'README.md',
  theme: 'default'
};
