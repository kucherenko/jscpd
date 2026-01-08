import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["bin/jscpd-server.ts", "src/index.ts"],
  splitting: true,
  sourcemap: true,
  clean: true,
  format: ["esm", "cjs"],
});
