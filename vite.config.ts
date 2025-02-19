import { defineConfig } from "vite";
import pluginReact from "@vitejs/plugin-react";
import tailwindCss from "@tailwindcss/vite";
import path from "node:path";

export default defineConfig({
  plugins: [pluginReact(), tailwindCss()],
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
  },
  resolve: {
    alias: { "@": path.resolve(__dirname, "./frontend") },
  },
});
