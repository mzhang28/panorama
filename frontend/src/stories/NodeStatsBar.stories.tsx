import type { Meta, StoryObj } from "@storybook/react";
import { NodeStatsBar } from "../components/NodeStatsBar";
import { createMockNodes } from "./mock-nodes";

const meta: Meta<typeof NodeStatsBar> = {
  title: "Core/NodeStatsBar",
  component: NodeStatsBar,
};

export default meta;
type Story = StoryObj<typeof NodeStatsBar>;

export const Default: Story = {
  args: {
    nodes: createMockNodes(50),
  },
};

export const Empty: Story = {
  args: {
    nodes: [],
  },
};

export const SingleNode: Story = {
  args: {
    nodes: createMockNodes(1),
  },
};
