/// <reference types="vite/client" />

// Plugin component loader — dispatches to dev or prod implementation
// based on Vite build mode.
//
//   vite dev                      → workspace imports + HMR
//   vite build --mode development → workspace imports (E2E)
//   vite build                    → Module Federation (production)

import { loadPluginComponent as devLoad } from './plugins/dev'
import { loadPluginComponent as prodLoad } from './plugins/prod'

export const loadPluginComponent =
  import.meta.env.MODE === 'production' ? prodLoad : devLoad
