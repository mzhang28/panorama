import { createRoot } from "react-dom/client";
import App from "./App";

const root = document.getElementById("root");
if (root === null) throw new Error("no root found");
createRoot(root).render(<App />);
