import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri/target", "src-tauri/gen"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
    rules: {
      // AGENTS.md: no console output outside tests — use src/lib/logger.ts
      "no-console": "error",
    },
  },
  {
    files: ["**/*.test.*"],
    rules: {
      "no-console": "off",
    },
  },
);
