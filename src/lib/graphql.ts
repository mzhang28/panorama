import { graphql, GraphQLSchema, GraphQLObjectType, GraphQLString, GraphQLList, GraphQLNonNull, GraphQLInputObjectType, GraphQLBoolean, GraphQLInt, GraphQLFloat } from 'graphql';
import { WASqliteService } from '../sw/sqlite';

export class GraphQLExecutor {
  private db: WASqliteService;
  private schema: GraphQLSchema | null = null;

  constructor(db: WASqliteService) {
    this.db = db;
  }

  private async buildSchema(): Promise<GraphQLSchema> {
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

    // Get indexed fields from database
    const indexedFields = await this.db.query('SELECT field_name FROM _panorama_tables WHERE is_indexed = 1');

    // Build query fields with dynamic indexed field arguments
    const nodesArgs: any = {
      type: { type: GraphQLString },
      limit: { type: GraphQLInt },
      offset: { type: GraphQLInt }
    };

    // Add indexed fields as query arguments
    for (const field of indexedFields) {
      nodesArgs[field.field_name] = { type: GraphQLString };
    }

    const queryFields: any = {
      nodes: {
        type: new GraphQLList(NodeType),
        args: nodesArgs,
        resolve: async (_parent: any, args: any) => {
          return this.resolveNodes(args);
        }
      },
      node: {
        type: NodeType,
        args: {
          id: { type: new GraphQLNonNull(GraphQLString) }
        },
        resolve: async (_parent: any, args: any) => {
          return this.resolveNode(args.id);
        }
      }
    };

    const QueryType = new GraphQLObjectType({
      name: 'Query',
      fields: queryFields
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
          resolve: async (_parent, args) => {
            return this.createNode(args.type, args.data);
          }
        },
        updateNode: {
          type: GraphQLString,
          args: {
            id: { type: new GraphQLNonNull(GraphQLString) },
            data: { type: GraphQLString }
          },
          resolve: async (_parent, args) => {
            return this.updateNode(args.id, args.data);
          }
        },
        deleteNode: {
          type: GraphQLString,
          args: {
            id: { type: new GraphQLNonNull(GraphQLString) }
          },
          resolve: async (_parent, args) => {
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
    if (!this.schema) {
      this.schema = await this.buildSchema();
    }

    return graphql({
      schema: this.schema,
      source: query,
      variableValues: variables
    });
  }

  private async resolveNodes(args: any): Promise<any[]> {
    let sql = 'SELECT n.* FROM node n';
    const params: any[] = [];
    const joins: string[] = [];
    let whereClause = '';

    if (args.type) {
      whereClause += ' n.type = ?';
      params.push(args.type);
    }

    // Handle dynamic indexed fields by checking all possible arguments against _panorama_tables
    const indexedFields = await this.db.query('SELECT field_name, table_name FROM _panorama_tables WHERE is_indexed = 1');

    for (const field of indexedFields) {
      const fieldName = field.field_name;
      if (args[fieldName]) {
        const tableName = field.table_name;
        const alias = fieldName.substring(0, 2); // Use first 2 chars as alias
        joins.push(`INNER JOIN ${tableName} ${alias} ON n.id = ${alias}.node_id`);
        whereClause += (whereClause ? ' AND' : '') + ` ${alias}.value = ?`;
        params.push(args[fieldName]);
      }
    }

    // Add WHERE clause if we have conditions
    if (whereClause) {
      sql += ' WHERE' + whereClause;
    }

    // Add joins to the query
    if (joins.length > 0) {
      sql = sql.replace('FROM node n', `FROM node n ${joins.join(' ')}`);
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
    const nodes = [];

    for (const row of results) {
      // Merge indexed fields into extraFields for each node
      const mergedExtraFields = await this.mergeIndexedFields(row.id, row.extraFields);
      nodes.push({
        ...row,
        extraFields: mergedExtraFields
      });
    }

    return nodes;
  }

  private async mergeIndexedFields(nodeId: string, extraFieldsJson: string | null): Promise<string | null> {
    // Parse existing extraFields
    let extraFields: Record<string, any> = {};
    if (extraFieldsJson) {
      try {
        extraFields = JSON.parse(extraFieldsJson);
      } catch (e) {
        // If parsing fails, start with empty object
        extraFields = {};
      }
    }

    // Get all indexed fields for this node
    const indexedFields = await this.db.query('SELECT field_name, table_name FROM _panorama_tables WHERE is_indexed = 1');

    for (const field of indexedFields) {
      const tableName = field.table_name;
      const fieldResults = await this.db.query(
        `SELECT value FROM ${tableName} WHERE node_id = ?`,
        [nodeId]
      );

      if (fieldResults.length > 0) {
        // Try to parse as JSON, otherwise keep as string
        const value = fieldResults[0].value;
        try {
          extraFields[field.field_name] = JSON.parse(value);
        } catch {
          extraFields[field.field_name] = value;
        }
      }
    }

    return Object.keys(extraFields).length > 0 ? JSON.stringify(extraFields) : null;
  }

  private async resolveNode(id: string): Promise<any> {
    const results = await this.db.query('SELECT * FROM node WHERE id = ?', [id]);
    if (results.length === 0) return null;

    const node = results[0];

    // Merge indexed fields into extraFields for response
    const mergedExtraFields = await this.mergeIndexedFields(node.id, node.extraFields);

    return {
      ...node,
      extraFields: mergedExtraFields
    };
  }

  private async createNode(type: string, data: string): Promise<string> {
    const id = Math.random().toString(36).substring(2, 15);
    const parsedData = data ? JSON.parse(data) : {};

    // Check for dynamic fields and separate indexed vs extra fields
    const { indexedFields, extraFieldsData } = await this.processFields(parsedData);

    const extraFields = Object.keys(extraFieldsData).length > 0 ? JSON.stringify(extraFieldsData) : null;
    const now = new Date().toISOString();

    await this.db.exec(
      'INSERT INTO node (id, type, extraFields, created_at, updated_at) VALUES (?, ?, ?, ?, ?)',
      [id, type, extraFields, now, now]
    );

    // Insert indexed fields
    for (const [fieldName, value] of Object.entries(indexedFields)) {
      const tableResult = await this.db.query(
        'SELECT table_name FROM _panorama_tables WHERE field_name = ? AND is_indexed = 1',
        [fieldName]
      );

      if (tableResult.length > 0) {
        const tableName = tableResult[0].table_name;
        await this.db.exec(
          `INSERT INTO ${tableName} (node_id, value) VALUES (?, ?)`,
          [id, String(value)]
        );
      }
    }

    // Track fields in extraFields
    if (extraFields) {
      for (const fieldName of Object.keys(extraFieldsData)) {
        await this.db.exec(
          'INSERT INTO _panorama_node_field_map (node_id, field_name) VALUES (?, ?)',
          [id, fieldName]
        );
      }
    }

    return id;
  }

  private async updateNode(id: string, data: string): Promise<string> {
    const parsedData = data ? JSON.parse(data) : {};

    // Process fields and separate indexed vs extra
    const { indexedFields, extraFieldsData } = await this.processFields(parsedData);

    // Get current extraFields to merge
    const currentNode = await this.db.query('SELECT extraFields FROM node WHERE id = ?', [id]);
    let currentExtraFields = {};
    if (currentNode.length > 0 && currentNode[0].extraFields) {
      try {
        currentExtraFields = JSON.parse(currentNode[0].extraFields);
      } catch (e) {
        // Ignore parse errors
      }
    }

    // Merge extra fields (new data overrides existing)
    const mergedExtraFields = { ...currentExtraFields, ...extraFieldsData };
    const extraFields = Object.keys(mergedExtraFields).length > 0 ? JSON.stringify(mergedExtraFields) : null;

    const now = new Date().toISOString();
    await this.db.exec(
      'UPDATE node SET extraFields = ?, updated_at = ? WHERE id = ?',
      [extraFields, now, id]
    );

    // Update indexed fields
    for (const [fieldName, value] of Object.entries(indexedFields)) {
      const tableResult = await this.db.query(
        'SELECT table_name FROM _panorama_tables WHERE field_name = ? AND is_indexed = 1',
        [fieldName]
      );

      if (tableResult.length > 0) {
        const tableName = tableResult[0].table_name;
        await this.db.exec(
          `INSERT OR REPLACE INTO ${tableName} (node_id, value) VALUES (?, ?)`,
          [id, String(value)]
        );
      }
    }

    // Update field map for extraFields
    // First, remove existing mappings for this node
    await this.db.exec('DELETE FROM _panorama_node_field_map WHERE node_id = ?', [id]);

    // Add new mappings
    if (extraFields) {
      for (const fieldName of Object.keys(mergedExtraFields)) {
        await this.db.exec(
          'INSERT INTO _panorama_node_field_map (node_id, field_name) VALUES (?, ?)',
          [id, fieldName]
        );
      }
    }

    return id;
  }

  private async deleteNode(id: string): Promise<string> {
    await this.db.exec('DELETE FROM node WHERE id = ?', [id]);
    return id;
  }

  private async processFields(data: any): Promise<{ indexedFields: Record<string, any>, extraFieldsData: Record<string, any> }> {
    const indexedFields: Record<string, any> = {};
    const extraFieldsData: Record<string, any> = {};

    for (const [key, value] of Object.entries(data)) {
      // Check if field is indexed
      const existing = await this.db.query(
        'SELECT * FROM _panorama_tables WHERE field_name = ? AND is_indexed = 1',
        [key]
      );

      if (existing.length > 0) {
        // Field is indexed
        indexedFields[key] = value;
      } else {
        // Check if field should be dynamically indexed
        const shouldIndex = this.shouldIndexField(value);
        if (shouldIndex) {
          await this.createDynamicTable(key);
          indexedFields[key] = value;
        } else {
          // Store in extraFields
          extraFieldsData[key] = value;
        }
      }
    }



    return { indexedFields, extraFieldsData };
  }

  private async handleDynamicFields(data: any): Promise<void> {
    // This method is now deprecated - use processFields instead
    await this.processFields(data);
  }

  private shouldIndexField(value: any): boolean {
    // For now, disable dynamic indexing to prevent issues
    // TODO: Re-enable with better logic to avoid conflicts
    return false;

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
    console.log(`createDynamicTable called for: ${fieldName}`);
    const tableName = `field_${fieldName.replace(/[^a-zA-Z0-9]/g, '_')}`;

    // Create table
    await this.db.exec(`
      CREATE TABLE IF NOT EXISTS ${tableName} (
        node_id TEXT PRIMARY KEY REFERENCES node(id) ON DELETE CASCADE,
        value TEXT
      )
    `);

    console.log(`Created table ${tableName}, now checking _panorama_tables`);

    // Check if field is already registered
    const existing = await this.db.query(
      'SELECT * FROM _panorama_tables WHERE field_name = ?',
      [fieldName]
    );

    console.log(`Existing entries for ${fieldName} in _panorama_tables:`, existing.length, existing);

    if (existing.length === 0) {
      console.log(`Inserting ${fieldName} into _panorama_tables`);
      try {
        // Register in _panorama_tables
        await this.db.exec(
          'INSERT INTO _panorama_tables (field_name, table_name, is_indexed) VALUES (?, ?, ?)',
          [fieldName, tableName, 1]
        );
        console.log(`Successfully inserted ${fieldName}`);
      } catch (error) {
        console.error(`Failed to insert ${fieldName} into _panorama_tables:`, error);
        // Check again after failure
        const recheck = await this.db.query(
          'SELECT * FROM _panorama_tables WHERE field_name = ?',
          [fieldName]
        );
        console.log(`After insert failure, entries for ${fieldName}:`, recheck.length, recheck);
        throw error;
      }
    } else {
      console.log(`Updating ${fieldName} to indexed`);
      // Update to ensure it's marked as indexed
      await this.db.exec(
        'UPDATE _panorama_tables SET is_indexed = 1 WHERE field_name = ?',
        [fieldName]
      );
      console.log(`Successfully updated ${fieldName}`);
    }
  }
}