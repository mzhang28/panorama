import NodeContainer from "./components/NodeContainer";
import Sidebar from "./components/Sidebar";
import styles from "./App.module.scss";
import { createSignal } from "solid-js";

function App() {
  const [getFocusedNodeId, setFocusedNodeId] = createSignal<string | null>(
    null,
  );

  const focusedNodeId = getFocusedNodeId();

  return (
    <div class={styles.container}>
      <Sidebar />
      {focusedNodeId !== null && <NodeContainer id={focusedNodeId} />}
    </div>
  );
}

export default App;
