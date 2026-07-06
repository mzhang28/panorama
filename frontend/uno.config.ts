import { defineConfig } from "unocss";
import presetUno from "@unocss/preset-uno";
import presetWebFonts from "@unocss/preset-web-fonts";

export default defineConfig({
  presets: [
    presetUno(),
    presetWebFonts({
      provider: "bunny",
      fonts: {
        sans: "Inter:400,500,600,700",
        mono: "JetBrains Mono:400,500",
      },
    }),
  ],
  theme: {
    colors: {
      bg: "var(--bg)",
      "bg-card": "var(--bg-card)",
      "bg-hover": "var(--bg-hover)",
      "bg-active": "var(--bg-active)",
      "bg-input": "var(--bg-input)",
      text: "var(--text)",
      "text-muted": "var(--text-muted)",
      "text-dim": "var(--text-dim)",
      border: "var(--border)",
      "border-bright": "var(--border-bright)",
      accent: "var(--accent)",
      "accent-hover": "var(--accent-hover)",
      "accent-text": "var(--accent-text)",
      "accent-subtle": "var(--accent-subtle)",
      danger: "var(--danger)",
      "danger-hover": "var(--danger-hover)",
      "danger-subtle": "var(--danger-subtle)",
      success: "var(--success)",
      "success-subtle": "var(--success-subtle)",
      warning: "var(--warning)",
      "warning-subtle": "var(--warning-subtle)",
    },
  },
});
