import NodeContainer from "./components/NodeContainer";
import Sidebar from "./components/Sidebar";
import styles from "./App.module.scss";

function App() {
  return (
    <div class={styles.container}>
      <Sidebar />
      <NodeContainer />
    </div>
  );
}

export default App;
