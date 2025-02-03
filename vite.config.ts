import { defineConfig } from "vite";
import pluginReact from "@vitejs/plugin-react";

export default defineConfig({
	plugins: [pluginReact()],
	server: {
		proxy: {
			"/api": {
				target: "http://localhost:3000",
				rewrite: (path) => path.replace(/^\/api/, ""),
			},
		},
	},
});
