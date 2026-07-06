import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { federation } from "@module-federation/vite";
import UnoCSS from "unocss/vite";
import path from "path";

const BACKEND_PORT = process.env.VITE_BACKEND_PORT || "3000";
const FRONTEND_PORT = Number(process.env.VITE_PORT) || 5173;

const resolvePlugin = (name: string) =>
  path.resolve(__dirname, `../crates/panorama-app-${name}/ui/src/App.tsx`);

export default defineConfig(({ mode }) => ({
  plugins: [
    UnoCSS(),
    react(),
    ...(mode === "production"
      ? [
          federation({
            name: "panorama_host",
            shared: {
              react: { singleton: true, requiredVersion: "^19.0.0" },
              "react-dom": { singleton: true, requiredVersion: "^19.0.0" },
              "@tanstack/react-query": {
                singleton: true,
                requiredVersion: "^5.60.0",
              },
            },
          }),
        ]
      : []),
  ],
  resolve: {
    alias: {
      "panorama-plugin-journal-ui": resolvePlugin("journal"),
      "panorama-plugin-dashboards-ui": resolvePlugin("dashboards"),
      "panorama-plugin-coding-ui": resolvePlugin("coding"),
      "panorama-plugin-trips-ui": resolvePlugin("trips"),
      "panorama-plugin-restaurants-ui": resolvePlugin("restaurants"),
      "panorama-plugin-music-ui": resolvePlugin("music"),
      "panorama-plugin-files-ui": resolvePlugin("files"),
    },
  },
  server: {
    port: FRONTEND_PORT,
    proxy: {
      "/api": `http://127.0.0.1:${BACKEND_PORT}`,
      "/plugin": `http://127.0.0.1:${BACKEND_PORT}`,
    },
    allowedHosts: ["ephemeral"],
  },
}));
