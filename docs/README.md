# Panorama Documentation

This directory contains the source for the **Panorama documentation site**, built with [Astro Starlight](https://starlight.astro.build/).

## Quick Start

```bash
cd docs/
bun install
bun run dev
```

Opens at `http://localhost:4321`.

## Structure

```
docs/
├── src/content/docs/       # Markdown/MDX content pages
│   ├── user/               # User-facing guides
│   ├── developer/core/     # Core platform architecture
│   ├── developer/apps/     # Custom app development
│   └── pql/                # Panorama Query Language
├── astro.config.mjs        # Starlight sidebar & config
└── public/                 # Static assets
```

## Building

```bash
bun run build      # Output to docs/dist/
bun run preview    # Preview the built site
```
