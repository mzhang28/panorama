import { graphql, GraphQLSchema, GraphQLObjectType, GraphQLString, GraphQLList, GraphQLNonNull, GraphQLInputObjectType, GraphQLBoolean, GraphQLInt, GraphQLFloat } from 'graphql';
import { SqliteService } from './services/sqliteService';

export class GraphQLExecutor {
  private db: SqliteService;
  private schema: GraphQLSchema | null = null;
  private manifest: any = null;

  constructor(db: SqliteService, manifest?: any) {
    this.db = db;
    this.manifest = manifest;
  }

  async init(): Promise<void> {
    await this.loadManifests();
    // Reset schema so it gets rebuilt with new indexed fields
    this.schema = null;
  }

  private async buildSchema(): Promise<GraphQLSchema> {
    // Get indexed fields from database
    const indexedFields = await this.db.query('SELECT field_name FROM _panorama_tables WHERE is_indexed = 1');

    // Define Node type with dynamic indexed fields
    const NodeType = new GraphQLObjectType({
      name: 'Node',
      fields: {
        id: { type: new GraphQLNonNull(GraphQLString) },
        type: { type: GraphQLString },
        extraFields: { type: GraphQLString },
        created_at: { type: GraphQLString },
        updated_at: { type: GraphQLString },
        ...indexedFields.reduce((acc, field) => {
          acc[field.field_name] = { type: GraphQLString }; // For now, all indexed fields are strings
          return acc;
        }, {} as any)
      }
    });

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

  private async getIndexedFields(): Promise<any[]> {
    return await this.db.query('SELECT field_name, table_name FROM _panorama_tables WHERE is_indexed = 1');
  }

  private async getNodesWithFilters(args: any, indexedFields: any[]): Promise<any[]> {
    let sql = 'SELECT n.* FROM node n';
    const params: any[] = [];
    const joins: string[] = [];
    let whereClause = '';

    if (args.type) {
      whereClause += ' n.type = ?';
      params.push(args.type);
    }

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

    if (whereClause) {
      sql += ' WHERE' + whereClause;
    }

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

    return await this.db.query(sql, params);
  }

  private async getIndexedFieldValue(tableName: string, nodeId: string): Promise<string | null> {
    const results = await this.db.query(`SELECT value FROM ${tableName} WHERE node_id = ?`, [nodeId]);
    return results.length > 0 ? results[0].value : null;
  }

  private async resolveNodes(args: any): Promise<any[]> {
    const indexedFields = await this.getIndexedFields();
    const results = await this.getNodesWithFilters(args, indexedFields);
    const nodes = [];

    for (const row of results) {
      const node = { ...row };
      for (const field of indexedFields) {
        const value = await this.getIndexedFieldValue(field.table_name, row.id);
        if (value !== null) {
          node[field.field_name] = value;
        }
      }
      nodes.push(node);
    }

    return nodes;
  }



  private async getNodeById(id: string): Promise<any> {
    const results = await this.db.query('SELECT * FROM node WHERE id = ?', [id]);
    return results.length > 0 ? results[0] : null;
  }

  private async resolveNode(id: string): Promise<any> {
    const node = await this.getNodeById(id);
    if (!node) return null;

    const indexedFields = await this.getIndexedFields();
    for (const field of indexedFields) {
      const value = await this.getIndexedFieldValue(field.table_name, node.id);
      if (value !== null) {
        node[field.field_name] = value;
      }
    }

    return node;
  }

  private generateNodeId(): string {
    return Math.random().toString(36).substring(2, 15);
  }

  private parseNodeData(data: string): any {
    return data ? JSON.parse(data) : {};
  }

  private async insertNode(id: string, type: string, extraFields: string | null): Promise<void> {
    const now = new Date().toISOString();
    await this.db.exec(
      'INSERT INTO node (id, type, extraFields, created_at, updated_at) VALUES (?, ?, ?, ?, ?)',
      [id, type, extraFields, now, now]
    );
  }

  private async getFieldTableName(fieldName: string): Promise<string | null> {
    const tableResult = await this.db.query(
      'SELECT table_name FROM _panorama_tables WHERE field_name = ? AND is_indexed = 1',
      [fieldName]
    );
    return tableResult.length > 0 ? tableResult[0].table_name : null;
  }

  private async ensureFieldTableExists(tableName: string): Promise<void> {
    try {
      await this.db.query(`SELECT 1 FROM ${tableName} LIMIT 1`);
    } catch (error) {
      await this.db.exec(`
        CREATE TABLE IF NOT EXISTS ${tableName} (
          node_id TEXT PRIMARY KEY REFERENCES node(id) ON DELETE CASCADE,
          value TEXT
        )
      `);
    }
  }

  private async insertIndexedField(tableName: string, nodeId: string, value: any): Promise<void> {
    await this.db.exec(
      `INSERT INTO ${tableName} (node_id, value) VALUES (?, ?)`,
      [nodeId, String(value)]
    );
  }

  private async insertExtraFieldMapping(nodeId: string, fieldName: string): Promise<void> {
    await this.db.exec(
      'INSERT INTO _panorama_node_field_map (node_id, field_name) VALUES (?, ?)',
      [nodeId, fieldName]
    );
  }

  private async createNode(type: string, data: string): Promise<string> {
    console.log('createNode called with type:', type, 'data:', data);
    const id = this.generateNodeId();
    const parsedData = this.parseNodeData(data);
    console.log('Parsed data:', parsedData);

    const { indexedFields, extraFieldsData } = await this.processFields(parsedData);
    console.log('After processFields - indexedFields:', indexedFields, 'extraFieldsData:', extraFieldsData);

    const extraFields = Object.keys(extraFieldsData).length > 0 ? JSON.stringify(extraFieldsData) : null;
    await this.insertNode(id, type, extraFields);

    // Insert indexed fields
    console.log('Inserting indexed fields:', indexedFields);
    for (const [fieldName, value] of Object.entries(indexedFields)) {
      console.log(`Inserting indexed field ${fieldName} = ${value}`);
      try {
        const tableName = await this.getFieldTableName(fieldName);
        if (tableName) {
          console.log(`Inserting into table ${tableName}`);
          await this.ensureFieldTableExists(tableName);
          await this.insertIndexedField(tableName, id, value);
          console.log(`Successfully inserted ${fieldName} into ${tableName}`);
        } else {
          console.log(`No table found for indexed field ${fieldName}`);
        }
      } catch (error) {
        console.error(`Failed to insert indexed field ${fieldName}:`, error);
      }
    }

    // Track fields in extraFields
    if (extraFields) {
      for (const fieldName of Object.keys(extraFieldsData)) {
        await this.insertExtraFieldMapping(id, fieldName);
      }
    }

    console.log(`createNode completed for id: ${id}`);
    return id;
  }

  private async getCurrentExtraFields(id: string): Promise<any> {
    const currentNode = await this.db.query('SELECT extraFields FROM node WHERE id = ?', [id]);
    if (currentNode.length > 0 && currentNode[0].extraFields) {
      try {
        return JSON.parse(currentNode[0].extraFields);
      } catch (e) {
        return {};
      }
    }
    return {};
  }

  private async updateNodeRecord(id: string, extraFields: string | null): Promise<void> {
    const now = new Date().toISOString();
    await this.db.exec(
      'UPDATE node SET extraFields = ?, updated_at = ? WHERE id = ?',
      [extraFields, now, id]
    );
  }

  private async updateIndexedField(tableName: string, nodeId: string, value: any): Promise<void> {
    await this.db.exec(
      `INSERT OR REPLACE INTO ${tableName} (node_id, value) VALUES (?, ?)`,
      [nodeId, String(value)]
    );
  }

  private async clearExtraFieldMappings(nodeId: string): Promise<void> {
    await this.db.exec('DELETE FROM _panorama_node_field_map WHERE node_id = ?', [nodeId]);
  }

  private async updateNode(id: string, data: string): Promise<string> {
    const parsedData = this.parseNodeData(data);
    const { indexedFields, extraFieldsData } = await this.processFields(parsedData);

    const currentExtraFields = await this.getCurrentExtraFields(id);
    const mergedExtraFields = { ...currentExtraFields, ...extraFieldsData };
    const extraFields = Object.keys(mergedExtraFields).length > 0 ? JSON.stringify(mergedExtraFields) : null;

    await this.updateNodeRecord(id, extraFields);

    // Update indexed fields
    for (const [fieldName, value] of Object.entries(indexedFields)) {
      const tableName = await this.getFieldTableName(fieldName);
      if (tableName) {
        await this.updateIndexedField(tableName, id, value);
      }
    }

    // Update field map for extraFields
    await this.clearExtraFieldMappings(id);
    if (extraFields) {
      for (const fieldName of Object.keys(mergedExtraFields)) {
        await this.insertExtraFieldMapping(id, fieldName);
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

    console.log('processFields called with data:', data);

    for (const [key, value] of Object.entries(data)) {
      console.log(`Processing field: '${key}' (length: ${key.length}) = ${value}`);

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

  private generateTableName(fieldName: string): string {
    return `field_${fieldName.replace(/[^a-zA-Z0-9]/g, '_')}`;
  }

  private async createFieldTable(tableName: string): Promise<void> {
    console.log(`Creating table ${tableName}`);
    await this.db.exec(`
      CREATE TABLE IF NOT EXISTS ${tableName} (
        node_id TEXT PRIMARY KEY REFERENCES node(id) ON DELETE CASCADE,
        value TEXT
      )
    `);
    console.log(`Created table ${tableName}, verifying it exists`);

    // Verify table was created
    try {
      await this.db.query(`SELECT 1 FROM ${tableName} LIMIT 1`);
      console.log(`Table ${tableName} exists and is accessible`);
    } catch (error) {
      console.error(`Table ${tableName} creation failed:`, error);
      throw error;
    }
  }

  private async ensurePanoramaTablesExist(): Promise<void> {
    try {
      await this.db.query('SELECT 1 FROM _panorama_tables LIMIT 1');
      console.log('_panorama_tables table exists');
    } catch (error) {
      console.log('_panorama_tables table does not exist, creating it');
      await this.db.exec(`
        CREATE TABLE IF NOT EXISTS _panorama_tables (
          field_name TEXT PRIMARY KEY,
          table_name TEXT NOT NULL,
          is_indexed BOOLEAN DEFAULT 0
        )
      `);
    }
  }

  private async checkFieldExists(fieldName: string): Promise<any[]> {
    return await this.db.query(
      'SELECT * FROM _panorama_tables WHERE field_name = ?',
      [fieldName]
    );
  }

  private async insertFieldRegistration(fieldName: string, tableName: string): Promise<void> {
    await this.db.exec(
      'INSERT OR IGNORE INTO _panorama_tables (field_name, table_name, is_indexed) VALUES (?, ?, ?)',
      [fieldName, tableName, 1]
    );
  }

  private async updateFieldRegistration(fieldName: string, tableName: string): Promise<void> {
    await this.db.exec(
      'UPDATE _panorama_tables SET table_name = ?, is_indexed = 1 WHERE field_name = ?',
      [tableName, fieldName]
    );
  }

  private async verifyFieldRegistration(fieldName: string): Promise<any[]> {
    return await this.db.query(
      'SELECT * FROM _panorama_tables WHERE field_name = ?',
      [fieldName]
    );
  }

  private async registerFieldInPanoramaTables(fieldName: string, tableName: string): Promise<void> {
    const existing = await this.checkFieldExists(fieldName);
    console.log(`Existing entries for ${fieldName} in _panorama_tables:`, existing.length, existing);

    console.log(`Ensuring ${fieldName} is registered in _panorama_tables with table ${tableName}`);
    try {
      await this.insertFieldRegistration(fieldName, tableName);
      console.log(`INSERT OR IGNORE completed for ${fieldName}`);

      await this.updateFieldRegistration(fieldName, tableName);
      console.log(`UPDATE completed for ${fieldName}`);

      const verify = await this.verifyFieldRegistration(fieldName);
      console.log(`Verification: ${fieldName} in _panorama_tables:`, verify);

      console.log(`Successfully ensured ${fieldName} is registered and indexed`);
    } catch (error) {
      console.error(`Failed to register ${fieldName} in _panorama_tables:`, error);
      throw error;
    }
  }

  private async createDynamicTable(fieldName: string): Promise<void> {
    console.log(`createDynamicTable called for: ${fieldName}`);
    const tableName = this.generateTableName(fieldName);

    await this.createFieldTable(tableName);

    console.log(`Now registering ${fieldName} in _panorama_tables`);
    await this.ensurePanoramaTablesExist();
    await this.registerFieldInPanoramaTables(fieldName, tableName);
  }

  private async loadManifests(): Promise<void> {
    console.log('Loading manifests...');

    if (this.manifest) {
      console.log('Using provided manifest:', this.manifest);
      if (this.manifest?.indexedFields) {
        console.log('Processing indexed fields:', this.manifest.indexedFields);
        for (const field of this.manifest.indexedFields) {
          if (field.indexed) {
            console.log(`Registering indexed field from manifest: ${field.name}`);
            await this.createDynamicTable(field.name);
            console.log(`Successfully registered ${field.name}`);
          }
        }
      } else {
        console.log('No indexed fields found in manifest');
      }
    } else {
      console.log('No manifest provided - no indexed fields to register');
    }

    // Check what indexed fields are registered
    try {
      const indexedFields = await this.db.query('SELECT * FROM _panorama_tables WHERE is_indexed = 1');
      console.log('Currently registered indexed fields:', indexedFields);
    } catch (error) {
      console.error('Failed to check indexed fields:', error);
    }
  }

  private async loadManifest(path: string): Promise<any> {
    try {
      console.log(`Loading manifest from ${path}`);
      // Use Bun's file reading capabilities
      const file = Bun.file(path);
      const manifest = await file.json();
      console.log(`Loaded manifest:`, manifest);
      return manifest;
    } catch (error) {
      console.error(`Failed to load manifest from ${path}:`, error);
      // Fallback to hardcoded manifest
      if (path === '/apps/journal/manifest.json') {
        console.log('Using hardcoded fallback manifest');
        return {
          name: "Journal",
          description: "Personal journaling application",
          version: "1.0.0",
          indexedFields: [
            {
              name: "journalDay",
              description: "Date of the journal entry in YYYY-MM-DD format",
              type: "string",
              indexed: true
            }
          ],
          nodeType: "JournalEntry"
        };
      }
      return null;
    }
  }
}