import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import Calendar from "./Calendar";
import "./global.scss";
import "bootstrap/dist/css/bootstrap.css";
import "bootstrap-icons/font/bootstrap-icons.css";

const queryClient = new QueryClient();

export default function App() {
  return (
    <>
      <QueryClientProvider client={queryClient}>
        <div className="container">
          <div className="header">
            <div>
              <input
                type="text"
                className="searchBar"
                placeholder="Search panorama..."
              />
            </div>
          </div>

          <div className="main">
            <Calendar />
          </div>
        </div>
      </QueryClientProvider>
    </>
  );
}
