import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import { VitePWA } from "vite-plugin-pwa";
import mkcert from "vite-plugin-mkcert";
import wasm from "vite-plugin-wasm";

export default defineConfig({
  plugins: [
    tailwindcss(),
    mkcert(),
    VitePWA({
      strategies: "injectManifest",
      srcDir: "src",
      filename: "sw/index.ts",
      injectRegister: "auto",
      registerType: "autoUpdate",
      includeAssets: ["**/*.wasm"],
      workbox: { globPatterns: ["**/*.{js,css,html,svg,wasm}"] },
      devOptions: { enabled: true, type: "module" },
      manifest: {
        name: "My Awesome App",
        short_name: "MyApp",
        description: "My Awesome App description",
        theme_color: "#ffffff",
        icons: [
          {
            src: "pwa-192x192.png",
            sizes: "192x192",
            type: "image/png",
          },
          {
            src: "pwa-512x512.png",
            sizes: "512x512",
            type: "image/png",
          },
        ],
      },
    }),
    wasm(),
  ],
});
