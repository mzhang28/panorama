import styles from "./Sidebar.module.scss";

export default function Sidebar() {
  return (
    <div class={styles.sidebar}>
      <h1>Panorama</h1>

      <h3>Bookmarked Nodes</h3>

      <h3>Settings</h3>
    </div>
  );
}
