import { sqliteTable, text, integer, primaryKey } from 'drizzle-orm/sqlite-core';
import { sql } from 'drizzle-orm';

export const nodes = sqliteTable('nodes', {
  id: text('id').primaryKey(),
  type: text('type').notNull().default('generic'),
  createdAt: integer('created_at', { mode: 'timestamp' }).notNull().default(sql`CURRENT_TIMESTAMP`),
  updatedAt: integer('updated_at', { mode: 'timestamp' }).notNull().default(sql`CURRENT_TIMESTAMP`),
});

export const edges = sqliteTable('edges', {
  sourceId: text('source_id').notNull().references(() => nodes.id),
  targetId: text('target_id').notNull().references(() => nodes.id),
  type: text('type').notNull().default('related'),
  createdAt: integer('created_at', { mode: 'timestamp' }).notNull().default(new Date()),
}, (t) => ({
  pk: primaryKey({ columns: [t.sourceId, t.targetId, t.type] }),
}));

export const apps = sqliteTable('apps', {
  id: text('id').primaryKey(),
  name: text('name').notNull(),
  version: text('version').notNull(),
  manifest: text('manifest').notNull(), // JSON string
  createdAt: integer('created_at', { mode: 'timestamp' }).notNull().default(new Date()),
});

export const appTables = sqliteTable('app_tables', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  appId: text('app_id').notNull().references(() => apps.id),
  tableName: text('table_name').notNull(),
  schemaJson: text('schema_json').notNull(),
});
