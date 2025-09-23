import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { GraphQLExecutor } from "../src/lib/graphql";
import { BunSqliteService } from "../src/lib/services/bunSqliteService";

describe("GraphQLExecutor", () => {
  let db: BunSqliteService;
  let executor: GraphQLExecutor;

  beforeEach(async () => {
    // Create fresh database instance for each test
    db = new BunSqliteService();
    await db.init();
    // Use the same manifest as the app
    const manifest = {
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
    executor = new GraphQLExecutor(db, manifest);
    await executor.init();
  });

  afterEach(() => {
    db.close();
  });

  it("should save journal entry with indexed journalDay field", async () => {
    // Simulate the data sent when saving a journal entry
    const journalData = {
      date: "2025-09-23",
      title: "Daily Journal - Monday, September 22, 2025",
      content: "Hello!",
      journalDay: "2025-09-23",
      createdAt: "2025-09-23T01:33:40.321Z",
      updatedAt: "2025-09-23T01:33:46.039Z",
    };

    // Execute the createNode mutation (simulating the GraphQL call)
    const result = await executor.execute(
      `
      mutation CreateJournalEntry($data: String!) {
        createNode(type: "JournalEntry", data: $data)
      }
    `,
      {
        data: JSON.stringify(journalData),
      },
    );

    // The mutation should succeed
    expect(result.errors).toBeUndefined();
    expect(result.data?.createNode).toBeDefined();

    const nodeId = result.data.createNode;

    // Verify the node was created
    const nodeTable = db.getTable("node");
    expect(nodeTable.length).toBe(1);
    expect(nodeTable[0].id).toBe(nodeId);
    expect(nodeTable[0].type).toBe("JournalEntry");

    // Parse extraFields to verify journalDay is NOT there
    const extraFields = JSON.parse(nodeTable[0].extraFields || "{}");
    expect(extraFields.journalDay).toBeUndefined();
    expect(extraFields.date).toBe("2025-09-23");
    expect(extraFields.title).toBe(
      "Daily Journal - Monday, September 22, 2025",
    );
    expect(extraFields.content).toBe("Hello!");
    expect(extraFields.createdAt).toBe("2025-09-23T01:33:40.321Z");
    expect(extraFields.updatedAt).toBe("2025-09-23T01:33:46.039Z");

    // Verify journalDay is stored in the indexed table
    const journalDayTable = db.getTable("field_journalDay");
    expect(journalDayTable.length).toBe(1);
    expect(journalDayTable[0].node_id).toBe(nodeId);
    expect(journalDayTable[0].value).toBe("2025-09-23");

    // Verify the field is registered as indexed
    const panoramaTables = db.getTable("_panorama_tables");
    const journalDayRegistration = panoramaTables.find(
      (row) => row.field_name === "journalDay",
    );
    expect(journalDayRegistration).toBeDefined();
    expect(journalDayRegistration?.is_indexed).toBe(1);
    expect(journalDayRegistration?.table_name).toBe("field_journalDay");
  });

  it("should query journal entries by journalDay", async () => {
    // First create a journal entry
    const journalData = {
      date: "2025-09-23",
      title: "Test Entry",
      content: "Test content",
      journalDay: "2025-09-23",
      createdAt: "2025-09-23T01:33:40.321Z",
      updatedAt: "2025-09-23T01:33:46.039Z",
    };

    const createResult = await executor.execute(
      `
      mutation CreateJournalEntry($data: String!) {
        createNode(type: "JournalEntry", data: $data)
      }
    `,
      {
        data: JSON.stringify(journalData),
      },
    );

    const nodeId = createResult.data.createNode;

    // Now query by journalDay
    const queryResult = await executor.execute(
      `
      query GetJournalEntries($journalDay: String!) {
        nodes(type: "JournalEntry", journalDay: $journalDay) {
          id
          extraFields
          journalDay
        }
      }
    `,
      {
        journalDay: "2025-09-23",
      },
    );

    expect(queryResult.errors).toBeUndefined();
    expect(queryResult.data?.nodes).toBeDefined();
    expect(queryResult.data.nodes.length).toBe(1);
    expect(queryResult.data.nodes[0].id).toBe(nodeId);

    // Verify journalDay is a separate field, not in extraFields
    expect(queryResult.data.nodes[0].journalDay).toBe("2025-09-23");
    const extraFields = JSON.parse(
      queryResult.data.nodes[0].extraFields || "{}",
    );
    expect(extraFields.journalDay).toBeUndefined();
    expect(extraFields.date).toBe("2025-09-23");
    expect(extraFields.title).toBe("Test Entry");
    expect(extraFields.content).toBe("Test content");
  });

  it("should return journalDay as separate field when querying individual node", async () => {
    // First create a journal entry
    const journalData = {
      date: "2025-09-23",
      title: "Individual Test Entry",
      content: "Individual test content",
      journalDay: "2025-09-23",
      createdAt: "2025-09-23T01:33:40.321Z",
      updatedAt: "2025-09-23T01:33:46.039Z",
    };

    const createResult = await executor.execute(
      `
      mutation CreateJournalEntry($data: String!) {
        createNode(type: "JournalEntry", data: $data)
      }
    `,
      {
        data: JSON.stringify(journalData),
      },
    );

    const nodeId = createResult.data.createNode;

    // Verify SQLite table structure
    const nodeTable = db.getTable("node");
    expect(nodeTable.length).toBe(1);
    expect(nodeTable[0].id).toBe(nodeId);
    expect(nodeTable[0].type).toBe("JournalEntry");
    const storedExtraFields = JSON.parse(nodeTable[0].extraFields || "{}");
    expect(storedExtraFields.journalDay).toBeUndefined(); // journalDay should not be in extraFields
    expect(storedExtraFields.date).toBe("2025-09-23");
    expect(storedExtraFields.title).toBe("Individual Test Entry");
    expect(storedExtraFields.content).toBe("Individual test content");
    expect(storedExtraFields.createdAt).toBe("2025-09-23T01:33:40.321Z");
    expect(storedExtraFields.updatedAt).toBe("2025-09-23T01:33:46.039Z");

    // Verify indexed field table
    const journalDayTable = db.getTable("field_journalDay");
    expect(journalDayTable.length).toBe(1);
    expect(journalDayTable[0].node_id).toBe(nodeId);
    expect(journalDayTable[0].value).toBe("2025-09-23");

    // Verify _panorama_tables has the indexed field registered
    const panoramaTables = db.getTable("_panorama_tables");
    const journalDayRegistration = panoramaTables.find(
      (row) => row.field_name === "journalDay",
    );
    expect(journalDayRegistration).toBeDefined();
    expect(journalDayRegistration?.table_name).toBe("field_journalDay");
    expect(journalDayRegistration?.is_indexed).toBe(1);

    // Verify _panorama_node_field_map has mappings for extraFields
    const fieldMapTable = db.getTable("_panorama_node_field_map");
    const expectedFields = [
      "date",
      "title",
      "content",
      "createdAt",
      "updatedAt",
    ];
    expectedFields.forEach((field) => {
      const mapping = fieldMapTable.find(
        (row) => row.node_id === nodeId && row.field_name === field,
      );
      expect(mapping).toBeDefined();
    });
    // journalDay should NOT be in field map since it's indexed
    const journalDayMapping = fieldMapTable.find(
      (row) => row.node_id === nodeId && row.field_name === "journalDay",
    );
    expect(journalDayMapping).toBeUndefined();

    // Query the individual node
    const queryResult = await executor.execute(
      `
      query GetNode($id: String!) {
        node(id: $id) {
          id
          type
          extraFields
          journalDay
          created_at
          updated_at
        }
      }
    `,
      {
        id: nodeId,
      },
    );

    expect(queryResult.errors).toBeUndefined();
    expect(queryResult.data?.node).toBeDefined();
    expect(queryResult.data.node.id).toBe(nodeId);
    expect(queryResult.data.node.type).toBe("JournalEntry");

    // Verify journalDay is a separate field, not in extraFields
    expect(queryResult.data.node.journalDay).toBe("2025-09-23");
    const extraFields = JSON.parse(queryResult.data.node.extraFields || "{}");
    expect(extraFields.journalDay).toBeUndefined();
    expect(extraFields.date).toBe("2025-09-23");
    expect(extraFields.title).toBe("Individual Test Entry");
    expect(extraFields.content).toBe("Individual test content");
  });
});
