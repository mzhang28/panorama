import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { NodeExplorerHome } from "../components/NodeExplorerHome";

// Create a QueryClient that returns mock data
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof NodeExplorerHome> = {
  title: "Core/NodeExplorerHome",
  component: NodeExplorerHome,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="p-[var(--space-6)]">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    // This story requires a mock server or MSW — for now, it shows the loading state
    mockData: true,
  },
};

export default meta;
type Story = StoryObj<typeof NodeExplorerHome>;

export const Default: Story = {};
