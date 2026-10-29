import { defineConfig } from "vitest/config";

// The frontend's pure helpers are plain functions with no DOM, so a node environment and the
// default resolver are enough.
export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
