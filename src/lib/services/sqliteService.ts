/** This is an abstraction layer for interacting with SQLite databases */

export abstract class SqliteService {
  abstract init(): Promise<void>;

  abstract query(sql: string, params?: any[]): Promise<any[]>;

  abstract exec(sql: string, params?: any[]): Promise<void>;
}
