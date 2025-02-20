import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import Calendar from "./apps/Calendar";
import "./global.css";
import "bootstrap/dist/css/bootstrap.css";
import "bootstrap-icons/font/bootstrap-icons.css";
import TabContainer from "./components/TabContainer";
import type { PropsWithChildren } from "react";

const queryClient = new QueryClient();

export default function App() {
  return (
    <Wrappers>
      <div className="flex flex-col grow">
        <TabContainer />
      </div>
    </Wrappers>
  );
}

function Wrappers({ children }: PropsWithChildren<{}>) {
  return (
    <QueryClientProvider client={queryClient}>
      {children ?? null}
    </QueryClientProvider>
  );
}
