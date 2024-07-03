import { defineConfig } from 'tsup'

export default defineConfig({
  entry: ['bin/jscpd.ts'],
  splitting: true,
  sourcemap: true,
  clean: true,
  format: ['esm', 'cjs']
})
