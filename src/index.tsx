import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import "./global.css";

// Listen for messages from service worker
navigator.serviceWorker?.addEventListener('message', (event) => {
  if (event.data?.type === 'console-log') {
    console.log('[SW]', ...event.data.args);
  } else if (event.data?.type === 'init-complete') {
    console.log('[SW] Initialization complete');
  }
});

const root = document.getElementById("root");
if (!root) throw new Error("Could not find root.");
createRoot(root).render(
  <BrowserRouter>
    <App />
  </BrowserRouter>,
);
