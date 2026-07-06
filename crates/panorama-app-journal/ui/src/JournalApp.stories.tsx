import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  BlockView,
  JournalApp,
  NewBlockForm,
  PropertiesBlock,
} from "./JournalApp";

// ── Mock data helpers ──────────────────────────────────────────────────────────

function makeBlockNode(overrides: Record<string, unknown> = {}) {
  return {
    id: "block-001",
    fields: {
      "system:node_title": { type: "string", value: "My Journal Page" },
      "journal:content": {
        type: "string",
        value:
          "This is the page content with a [[link]] to another page and some **bold text**.",
      },
      "journal:journal_day": { type: "string", value: "2026-07-06" },
      "journal:deleted": { type: "boolean", value: false },
      "journal:tags": {
        type: "json",
        value: '["journal", "daily", "personal"]',
      },
      "journal:properties": {
        type: "json",
        value: '{"status": "draft", "pinned": true}',
      },
      ...overrides,
    },
    created_at: "2026-07-06T12:00:00Z",
    updated_at: "2026-07-06T14:30:00Z",
  };
}

// ── Shared noop callbacks ──────────────────────────────────────────────────────

const noop = () => {};
const noopAsync = async (body: unknown) => ({
  id: "new-001",
  n: { id: "new-001" },
});

// ── PropertiesBlock ────────────────────────────────────────────────────────────

const propertiesMeta: Meta<typeof PropertiesBlock> = {
  title: "Journal/PropertiesBlock",
  component: PropertiesBlock,
};

export default propertiesMeta;
type PropsStory = StoryObj<typeof PropertiesBlock>;

export const Properties: PropsStory = {
  args: {
    properties: { status: "draft", pinned: true, author: "Michael" },
  },
};

export const PropertiesEmpty: PropsStory = {
  args: { properties: {} },
};

export const PropertiesManyTypes: PropsStory = {
  args: {
    properties: {
      done: false,
      priority: "high",
      "word-count": 1420,
      reviewed: true,
      tags: "journal, writing",
    },
  },
};

// ── BlockView ──────────────────────────────────────────────────────────────────

const blockViewMeta: Meta<typeof BlockView> = {
  title: "Journal/BlockView",
  component: BlockView,
  args: {
    onEdit: noop,
    onSave: noop,
    onCancel: noop,
    onDelete: noop,
    editingBlockId: null,
  },
};

type BlockStory = StoryObj<typeof BlockView>;

export const BlockRoot: BlockStory = {
  name: "Root block (depth 0)",
  args: { block: makeBlockNode(), depth: 0 },
};

export const BlockChild: BlockStory = {
  name: "Child block (depth 1)",
  args: {
    block: makeBlockNode({
      "system:node_title": { type: "string", value: "" },
      "journal:content": {
        type: "string",
        value: "A nested child block with some content.",
      },
    }),
    depth: 1,
  },
};

export const BlockDeeplyNested: BlockStory = {
  name: "Deeply nested (depth 3)",
  args: {
    block: makeBlockNode({
      "system:node_title": { type: "string", value: "" },
      "journal:content": {
        type: "string",
        value: "A deeply indented block.",
      },
    }),
    depth: 3,
  },
};

export const BlockEditing: BlockStory = {
  name: "Editing mode",
  args: { block: makeBlockNode(), depth: 0, editingBlockId: "block-001" },
};

export const BlockDeleted: BlockStory = {
  name: "Deleted block",
  args: {
    block: makeBlockNode({
      "journal:deleted": { type: "boolean", value: true },
      "journal:content": {
        type: "string",
        value: "This page has been deleted",
      },
    }),
    depth: 0,
  },
};

export const BlockEmpty: BlockStory = {
  name: "Empty content",
  args: {
    block: makeBlockNode({
      "journal:content": { type: "string", value: "" },
    }),
    depth: 0,
  },
};

export const BlockWithLinks: BlockStory = {
  name: "Wiki links",
  args: {
    block: makeBlockNode({
      "journal:content": {
        type: "string",
        value:
          "See also [[Project Alpha]] and [[Meeting Notes]]. Related to [[Sprint 42]].",
      },
    }),
    depth: 0,
  },
};

// ── NewBlockForm ───────────────────────────────────────────────────────────────

const newBlockFormMeta: Meta<typeof NewBlockForm> = {
  title: "Journal/NewBlockForm",
  component: NewBlockForm,
  args: {
    createBlock: noopAsync,
    onCreated: noop,
    isPending: false,
  },
};

type FormStory = StoryObj<typeof NewBlockForm>;

export const NewPageForm: FormStory = {
  name: "New page form",
  args: { parentId: null, pageId: null },
};

export const NewChildBlock: FormStory = {
  name: "New child block form",
  args: { parentId: "block-001", pageId: "block-001" },
};

export const NewBlockPending: FormStory = {
  name: "Pending submission",
  args: { parentId: null, pageId: null, isPending: true },
};

// ── Full Journal App ───────────────────────────────────────────────────────────

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const journalMeta: Meta<typeof JournalApp> = {
  title: "Journal/Full App",
  component: JournalApp,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div style={{ height: "100vh", background: "var(--bg)" }}>
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
  parameters: { mockData: true },
};

export const FullApp: StoryObj<typeof JournalApp> = {
  name: "Full Journal App",
};
