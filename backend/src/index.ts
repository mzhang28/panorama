import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { db } from './db';
import * as schema from './schema';
import { pino } from 'pino';
import { sql } from 'drizzle-orm';
import { weightTrackerManifest } from './apps/weight-tracker';
import { loadConfig } from '@app-config/main';

const logger = pino();
const app = new Hono();

app.use('*', cors());

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
      
      // In SQLite, adding columns is also okay if they don't exist, but for a WIP, we just ensure the table exists
      // SQLite Database instance is available on db.$client if using Bun
      // But we can use sql.raw
      await db.run(sql.raw(ddl));
    }
  }
}

// PromQL-like Query Engine
async function queryEngine(query: string) {
  // Simple: match the query string to a column name in any third-party table
  for (const manifest of appRegistry) {
    for (const table of manifest.tables) {
      const col = table.columns.find(c => c.name === query);
      if (col) {
        // Build join query. Joins app table with nodes for potential graph features
        // SELECT val, timestamp FROM app_table JOIN nodes ON app_table.node_id = nodes.id
        const results = await db.run(sql.raw(`
          SELECT ${query} as value, timestamp 
          FROM ${table.name} 
          ORDER BY timestamp ASC
        `));
        // Database.run in Bun returns { results, ... } or just rows?
        // Actually Database.all is for rows.
        // Let's use db.all instead of db.run for results.
        const rows = await db.all(sql.raw(`
          SELECT ${query} as value, timestamp 
          FROM ${table.name} 
          ORDER BY timestamp ASC
        `));
        return rows;
      }
    }
  }
  return [];
}

// Config Loading
let cachedConfig: any = null;
async function getConfig() {
  if (cachedConfig) return cachedConfig;
  try {
    const config = await loadConfig();
    cachedConfig = config;
    return config;
  } catch (err) {
    logger.error({ err }, 'Failed to load app-config');
    return { tabs: [] };
  }
}

// API Routes
app.get('/api/config', async (c) => {
  const config = await getConfig();
  return c.json(config);
});

app.get('/api/query', async (c) => {
  const q = c.req.query('q');
  if (!q) return c.json([]);
  const results = await queryEngine(q);
  return c.json(results);
});

app.post('/api/apps/weight-tracker/entry', async (c) => {
  const body = await c.req.json();
  const nodeId = crypto.randomUUID();
  
  await db.transaction(async (tx) => {
    await tx.insert(schema.nodes).values({ id: nodeId, type: 'weight_entry' });
    await tx.run(sql`
      INSERT INTO app_weight_entries (node_id, weight_kg, timestamp)
      VALUES (${nodeId}, ${body.weight}, ${Date.now()})
    `);
  });
  
  return c.json({ success: true, nodeId });
});

app.get('/', (c) => c.text('Panorama Backend Running'));

// Bootstrap and run
(async () => {
  await bootstrap();
  logger.info('System Ready.');
})();

export default {
  port: 3001,
  fetch: app.fetch,
};
