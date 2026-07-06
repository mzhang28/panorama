import type { Meta, StoryObj } from "@storybook/react";
import { NodeTableCondensed } from "../components/NodeTableCondensed";
import { createMockNodes } from "./mock-nodes";

const meta: Meta<typeof NodeTableCondensed> = {
  title: "Core/NodeTableCondensed",
  component: NodeTableCondensed,
};

export default meta;
type Story = StoryObj<typeof NodeTableCondensed>;

export const Default: Story = {
  args: {
    nodes: createMockNodes(25),
  },
};

export const Empty: Story = {
  args: {
    nodes: [],
  },
};

export const ManyNodes: Story = {
  args: {
    nodes: createMockNodes(100),
  },
};
