import type { Meta, StoryObj } from "@storybook/react";
import { NodeActivityChart } from "../components/NodeActivityChart";
import { createMockNodes } from "./mock-nodes";

const meta: Meta<typeof NodeActivityChart> = {
  title: "Core/NodeActivityChart",
  component: NodeActivityChart,
};

export default meta;
type Story = StoryObj<typeof NodeActivityChart>;

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

export const SingleDay: Story = {
  args: {
    nodes: createMockNodes(20),
  },
};
