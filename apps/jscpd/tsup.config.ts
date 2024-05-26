import { defineConfig } from 'tsup'

export default defineConfig({
  entry: ['bin/jscpd.ts'],
  splitting: false,
  sourcemap: false,
  clean: true,
})
