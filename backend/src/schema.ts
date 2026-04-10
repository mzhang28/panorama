import { sqliteTable, text, integer } from 'drizzle-orm/sqlite-core';

export const greetings = sqliteTable('greetings', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  name: text('name').notNull(),
  message: text('message').notNull(),
  createdAt: integer('created_at', { mode: 'timestamp' }).notNull().default(new Date()),
});
