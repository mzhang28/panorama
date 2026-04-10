import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { db } from './db';
import * as schema from './schema';
import { pino } from 'pino';
import { sql } from 'drizzle-orm';
import { weightTrackerManifest } from './apps/weight-tracker';
import { loadConfig } from '@app-config/main';
import yaml from 'js-yaml';
import fs from 'fs/promises';
import path from 'path';

const logger = pino();
const app = new Hono();

app.use('*', cors());

// Helper for PromQL time parsing
function parsePromQLTimeRange(timeStr: string): number {
  if (!timeStr || timeStr === 'all') return 0;
  const match = timeStr.match(/^(\d+)([hdmw])$/);
  if (!match) return 0;
  const val = parseInt(match[1]);
  const unit = match[2];
  const now = Date.now();
  switch (unit) {
    case 'm': return now - val * 60 * 1000;
    case 'h': return now - val * 60 * 60 * 1000;
    case 'd': return now - val * 24 * 60 * 60 * 1000;
    case 'w': return now - val * 7 * 24 * 60 * 60 * 1000;
    default: return 0;
  }
}

// Registry of apps
const appRegistry = [weightTrackerManifest];

// Bootstrap function
async function bootstrap() {
  logger.info('Bootstrapping system...');
  
  for (const manifest of appRegistry) {
    // Register app in core table
    await db.insert(schema.apps).values({
      id: manifest.id,
      name: manifest.name,
      version: manifest.version,
      manifest: JSON.stringify(manifest),
    }).onConflictDoUpdate({
      target: schema.apps.id,
      set: {
        name: manifest.name,
        version: manifest.version,
        manifest: JSON.stringify(manifest),
      }
    });

    for (const table of manifest.tables) {
      logger.info({ appId: manifest.id, tableName: table.name }, 'Checking third-party table integrity');
      
      // Upsert into app_tables
      await db.insert(schema.appTables).values({
        appId: manifest.id,
        tableName: table.name,
        schemaJson: JSON.stringify(table),
      });

      // Simple dynamic DDL for SQLite
      let ddl = `CREATE TABLE IF NOT EXISTS ${table.name} (`;
      const colDefs = table.columns.map(col => {
        let def = `${col.name} ${col.type.toUpperCase()}`;
        if (col.notNull) def += ' NOT NULL';
        if (col.references) def += ` REFERENCES ${col.references}`;
        return def;
      });
      ddl += colDefs.join(', ') + ')';
      
      await db.run(sql.raw(ddl));
    }
  }
}

// PromQL-like Query Engine
async function queryEngine(query: string, timeRange?: string) {
  const cutoff = parsePromQLTimeRange(timeRange || 'all');
  
  for (const manifest of appRegistry) {
    for (const table of manifest.tables) {
      const col = table.columns.find(c => c.name === query);
      if (col) {
        const rows = await db.all(sql.raw(`
          SELECT t.${query} as value, n.created_at as timestamp 
          FROM ${table.name} t
          JOIN nodes n ON t.node_id = n.id
          WHERE n.created_at >= ${cutoff}
          ORDER BY n.created_at ASC
        `));
        return rows;
      }
    }
  }
  return [];
}

// Config Loading
let cachedConfig: any = null;
async function getConfig(forceReload = true) {
  if (cachedConfig && !forceReload) return cachedConfig;
  try {
    const filepath = await getConfigFilePath();
    logger.info({ filepath, env: process.env.APP_CONFIG_ENV }, 'Loading config from disk');
    const content = await fs.readFile(filepath, 'utf8');
    const config: any = yaml.load(content);
    cachedConfig = config;
    return config;
  } catch (err) {
    logger.error({ err }, 'Failed to load config from disk');
    return { tabs: [] };
  }
}

// API Routes
app.get('/api/config', async (c) => {
  const config = await getConfig(true);
  return c.json(config);
});

app.get('/api/apps', (c) => {
  return c.json(appRegistry);
});

app.get('/api/query', async (c) => {
  const q = c.req.query('q');
  const t = c.req.query('t');
  if (!q) return c.json([]);
  const results = await queryEngine(q, t);
  return c.json(results);
});

async function getConfigFilePath() {
  const env = process.env.APP_CONFIG_ENV || '';
  const filename = env ? `.app-config.${env}.yml` : '.app-config.yml';
  const localPath = path.resolve(process.cwd(), filename);
  return localPath;
}

app.put('/api/config/widget/:tabId/:widgetId', async (c) => {
  const tabId = c.req.param('tabId');
  const widgetId = c.req.param('widgetId');
  const body = await c.req.json();
  const filepath = await getConfigFilePath();
  
  try {
    const content = await fs.readFile(filepath, 'utf8');
    const config: any = yaml.load(content);
    
    const tab = config.tabs.find((t: any) => t.id === tabId);
    if (!tab) return c.json({ error: 'Tab not found' }, 404);
    
    const widget = tab.widgets.find((w: any) => w.id === widgetId);
    if (!widget) return c.json({ error: 'Widget not found' }, 404);
    
    Object.assign(widget, body);
    
    await fs.writeFile(filepath, yaml.dump(config), 'utf8');
    cachedConfig = null;
    return c.json({ success: true });
  } catch (err) {
    logger.error({ err }, 'Failed to update config file');
    return c.json({ error: 'Failed to update config file' }, 500);
  }
});

app.post('/api/config/widget/:tabId', async (c) => {
  const tabId = c.req.param('tabId');
  const body = await c.req.json();
  const filepath = await getConfigFilePath();
  
  try {
    const content = await fs.readFile(filepath, 'utf8');
    const config: any = yaml.load(content);
    
    const tab = config.tabs.find((t: any) => t.id === tabId);
    if (!tab) return c.json({ error: 'Tab not found' }, 404);
    
    const newWidget = {
      id: `widget-${Date.now()}`,
      ...body,
      grid: body.grid || { w: 3, h: 1 }
    };
    
    tab.widgets.push(newWidget);
    
    await fs.writeFile(filepath, yaml.dump(config), 'utf8');
    cachedConfig = null;
    return c.json(newWidget);
  } catch (err) {
    logger.error({ err }, 'Failed to add widget to config');
    return c.json({ error: 'Failed to add widget' }, 500);
  }
});

app.post('/api/apps/weight-tracker/entry', async (c) => {
  const body = await c.req.json();
  const nodeId = crypto.randomUUID();
  
  await db.transaction(async (tx) => {
    await tx.insert(schema.nodes).values({ id: nodeId, type: 'weight_entry' });
    await tx.run(sql`
      INSERT INTO app_weight_entries (node_id, weight_kg)
      VALUES (${nodeId}, ${body.weight})
    `);
  });
  
  return c.json({ success: true, nodeId });
});

app.get('/', (c) => c.text('Panorama Backend Running'));

(async () => {
  await bootstrap();
  logger.info('System Ready.');
})();

export default {
  port: 3001,
  fetch: app.fetch,
};
