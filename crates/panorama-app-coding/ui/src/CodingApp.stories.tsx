import type { Meta, StoryObj } from "@storybook/react";
import CodingApp from "./App";

const meta: Meta<typeof CodingApp> = {
  title: "Coding Activity/App",
  component: CodingApp,
  parameters: {
    // No server needed — component renders its UI statically
    mockData: true,
  },
};

export default meta;
type Story = StoryObj<typeof CodingApp>;

export const Default: Story = {
  args: {
    pluginId: "io.mzhang.panorama.coding",
  },
};
