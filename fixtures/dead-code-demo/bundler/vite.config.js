import { fileURLToPath, URL } from 'node:url';

// The only place `@` is declared: this project has no tsconfig.json.
export default {
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
};
