// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	integrations: [
		starlight({
			title: 'Panorama Documentation',
			social: [],
			sidebar: [
				{
					label: 'User Guide',
					items: [
						{ label: 'Installation & Setup', slug: 'user/installation' },
						{ label: 'Core Concepts', slug: 'user/concepts' },
						{ label: 'Installing Apps', slug: 'user/installing-apps' },
					],
				},
				{
					label: 'Core Platform Development',
					items: [
						{ label: 'Architecture Overview', slug: 'developer/core/architecture' },
						{ label: 'Reactor & Hook Subsystem', slug: 'developer/core/reactor-system' },
					],
				},
				{
					label: 'Custom App Development',
					items: [
						{ label: 'Getting Started', slug: 'developer/apps/getting-started' },
						{ label: 'Plugin API Reference', slug: 'developer/apps/plugin-api' },
						{ label: 'Building & Packaging', slug: 'developer/apps/building-packaging' },
						{ label: 'Example Reference Implementations', slug: 'developer/apps/examples' },
					],
				},
				{
					label: 'Panorama Query Language (PQL)',
					items: [
						{ label: 'PQL Data Model', slug: 'pql/data-model' },
						{ label: 'Syntax & Usage Guide', slug: 'pql/syntax-guide' },
						{ label: 'Execution & Compiler Engine', slug: 'pql/execution-engine' },
					],
				},
			],
		}),
	],
});
