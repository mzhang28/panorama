import type { StorybookConfig } from "@storybook/react-vite";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { mergeConfig } from "vite";

function getAbsolutePath(value: string) {
  return dirname(fileURLToPath(import.meta.resolve(`${value}/package.json`)));
}

const config: StorybookConfig = {
  stories: [
    // All plugin UI packages + frontend components
    "../../crates/panorama-app-*/ui/src/**/*.stories.@(js|jsx|mjs|ts|tsx)",
    "../../crates/panorama-app-*/ui/src/**/*.mdx",
    "../src/**/*.stories.@(js|jsx|mjs|ts|tsx)",
    "../src/**/*.mdx",
  ],
  addons: [
    getAbsolutePath("@chromatic-com/storybook"),
    getAbsolutePath("@storybook/addon-vitest"),
    getAbsolutePath("@storybook/addon-a11y"),
    getAbsolutePath("@storybook/addon-docs"),
    getAbsolutePath("@storybook/addon-mcp"),
  ],
  framework: getAbsolutePath("@storybook/react-vite"),
  async viteFinal(config) {
    return mergeConfig(config, {
      resolve: {
        alias: {
          // Mirror the dev Vite config aliases so plugin UI imports resolve
          "panorama-plugin-journal-ui": resolve(
            "../crates/panorama-app-journal/ui/src/App.tsx",
          ),
          "panorama-plugin-dashboards-ui": resolve(
            "../crates/panorama-app-dashboards/ui/src/App.tsx",
          ),
          "panorama-plugin-coding-ui": resolve(
            "../crates/panorama-app-coding/ui/src/App.tsx",
          ),
          "panorama-plugin-trips-ui": resolve(
            "../crates/panorama-app-trips/ui/src/App.tsx",
          ),
          "panorama-plugin-restaurants-ui": resolve(
            "../crates/panorama-app-restaurants/ui/src/App.tsx",
          ),
          "panorama-plugin-music-ui": resolve(
            "../crates/panorama-app-music/ui/src/App.tsx",
          ),
          "panorama-plugin-files-ui": resolve(
            "../crates/panorama-app-files/ui/src/App.tsx",
          ),
        },
      },
    });
  },
};
export default config;
