import { defineConfig } from "vitest/config";
import tailwindcss from "tailwindcss";

export default defineConfig({
  css: {
    postcss: {
      plugins: [tailwindcss()],
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/setup.ts"],
  },
});
