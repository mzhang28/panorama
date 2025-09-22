import { graphql, GraphQLSchema, GraphQLObjectType, GraphQLString, GraphQLList, GraphQLNonNull, GraphQLInputObjectType, GraphQLBoolean, GraphQLInt, GraphQLFloat } from 'graphql';
import { WASqliteService } from '../sw/sqlite';

export class GraphQLExecutor {
  private db: WASqliteService;
  private schema: GraphQLSchema;

  constructor(db: WASqliteService) {
    this.db = db;
    this.schema = this.buildSchema();
  }

  private buildSchema(): GraphQLSchema {
    // Define Node type
    const NodeType = new GraphQLObjectType({
      name: 'Node',
      fields: {
        id: { type: new GraphQLNonNull(GraphQLString) },
        type: { type: GraphQLString },
        extraFields: { type: GraphQLString },
        created_at: { type: GraphQLString },
        updated_at: { type: GraphQLString }
      }
    });

    const QueryType = new GraphQLObjectType({
      name: 'Query',
      fields: {
        nodes: {
          type: new GraphQLList(NodeType),
          args: {
            type: { type: GraphQLString },
            limit: { type: GraphQLInt },
            offset: { type: GraphQLInt }
          },
          resolve: async (parent, args) => {
            return this.resolveNodes(args);
          }
        },
        node: {
          type: NodeType,
          args: {
            id: { type: new GraphQLNonNull(GraphQLString) }
          },
          resolve: async (parent, args) => {
            return this.resolveNode(args.id);
          }
        }
      }
    });

    const MutationType = new GraphQLObjectType({
      name: 'Mutation',
      fields: {
        createNode: {
          type: GraphQLString,
          args: {
            type: { type: new GraphQLNonNull(GraphQLString) },
            data: { type: GraphQLString } // JSON string
          },
          resolve: async (parent, args) => {
            return this.createNode(args.type, args.data);
          }
        },
        updateNode: {
          type: GraphQLString,
          args: {
            id: { type: new GraphQLNonNull(GraphQLString) },
            data: { type: GraphQLString }
          },
          resolve: async (parent, args) => {
            return this.updateNode(args.id, args.data);
          }
        },
        deleteNode: {
          type: GraphQLString,
          args: {
            id: { type: new GraphQLNonNull(GraphQLString) }
          },
          resolve: async (parent, args) => {
            return this.deleteNode(args.id);
          }
        }
      }
    });

    return new GraphQLSchema({
      query: QueryType,
      mutation: MutationType
    });
  }

  async execute(query: string, variables?: any): Promise<any> {
    return graphql({
      schema: this.schema,
      source: query,
      variableValues: variables
    });
  }

  private async resolveNodes(args: any): Promise<any[]> {
    let sql = 'SELECT * FROM node';
    const params: any[] = [];

    if (args.type) {
      sql += ' WHERE type = ?';
      params.push(args.type);
    }

    if (args.limit) {
      sql += ' LIMIT ?';
      params.push(args.limit);
    }

    if (args.offset) {
      sql += ' OFFSET ?';
      params.push(args.offset);
    }

    const results = await this.db.query(sql, params);
    return results.map(row => ({
      ...row,
      extraFields: row.extraFields || null
    }));
  }

  private async resolveNode(id: string): Promise<any> {
    const results = await this.db.query('SELECT * FROM node WHERE id = ?', [id]);
    if (results.length === 0) return null;

    const node = results[0];
    return {
      ...node,
      extraFields: node.extraFields || null
    };
  }

  private async createNode(type: string, data: string): Promise<string> {
    const id = Math.random().toString(36).substring(2, 15);
    const parsedData = data ? JSON.parse(data) : {};

    // Check for dynamic fields
    await this.handleDynamicFields(parsedData);

    const extraFields = JSON.stringify(parsedData);
    const now = new Date().toISOString();

    await this.db.exec(
      'INSERT INTO node (id, type, extraFields, created_at, updated_at) VALUES (?, ?, ?, ?, ?)',
      [id, type, extraFields, now, now]
    );

    return id;
  }

  private async updateNode(id: string, data: string): Promise<string> {
    const parsedData = data ? JSON.parse(data) : {};

    // Check for dynamic fields
    await this.handleDynamicFields(parsedData);

    const extraFields = JSON.stringify(parsedData);
    const now = new Date().toISOString();

    await this.db.exec(
      'UPDATE node SET extraFields = ?, updated_at = ? WHERE id = ?',
      [extraFields, now, id]
    );

    return id;
  }

  private async deleteNode(id: string): Promise<string> {
    await this.db.exec('DELETE FROM node WHERE id = ?', [id]);
    return id;
  }

  private async handleDynamicFields(data: any): Promise<void> {
    for (const [key, value] of Object.entries(data)) {
      // Check if field exists in _panorama_tables
      const existing = await this.db.query(
        'SELECT * FROM _panorama_tables WHERE field_name = ?',
        [key]
      );

      if (existing.length === 0) {
        // Field doesn't exist, decide based on value type/complexity
        const shouldIndex = this.shouldIndexField(value);
        if (shouldIndex) {
          await this.createDynamicTable(key);
        }
        // If not indexed, it stays in extraFields
      }
    }
  }

  private shouldIndexField(value: any): boolean {
    // Simple heuristic: index if it's a primitive or small array
    if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
      return true;
    }
    if (Array.isArray(value) && value.length < 10) {
      return true;
    }
    return false;
  }

  private async createDynamicTable(fieldName: string): Promise<void> {
    const tableName = `field_${fieldName.replace(/[^a-zA-Z0-9]/g, '_')}`;

    // Create table
    await this.db.exec(`
      CREATE TABLE IF NOT EXISTS ${tableName} (
        node_id TEXT PRIMARY KEY REFERENCES node(id) ON DELETE CASCADE,
        value TEXT
      )
    `);

    // Register in _panorama_tables
    await this.db.exec(
      'INSERT INTO _panorama_tables (field_name, table_name, is_indexed) VALUES (?, ?, ?)',
      [fieldName, tableName, 1]
    );
  }
}