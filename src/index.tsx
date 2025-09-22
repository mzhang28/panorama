import { createRoot } from "react-dom/client";
import App from "./App";
const root = document.getElementById("root");
if (!root) throw new Error("Could not find root.");
createRoot(root).render(<App />);
