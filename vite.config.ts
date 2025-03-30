import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tsconfigPaths from "vite-tsconfig-paths";
import dotenv from "dotenv";

dotenv.config();

export default defineConfig({
  plugins: [react(), tsconfigPaths()],
  server: {
    port: process.env.PORT ? Number.parseInt(process.env.PORT) : 6560,
    proxy: {
      "/api": {
        target: "http://localhost:6561",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
  },
});
