<script>
  import {} from "@tanstack/svelte-query";
  import "../global.scss";
  import Sidebar from "../components/composite/sidebar/Sidebar.svelte";
  import MailAccounts from "../components/pages/MailAccounts.svelte";
</script>

<div class="container">
  <div class="sidebar">
    <Sidebar />
  </div>

  <div class="main">
    <div class="header">
      <h1>panorama</h1>

      <input type="text" placeholder="Search..." />
    </div>

    <main class="container">
      <MailAccounts />
    </main>
  </div>
</div>

<style lang="scss" scoped>
  .container {
    width: 100%;
    height: 100%;

    display: flex;
    flex-direction: row;
    align-items: stretch;
  }

  .sidebar {
    border-right: 1px solid lightgray;
    display: flex;
  }

  .main {
    flex-grow: 1;

    display: flex;
    flex-direction: column;
  }

  .header {
    padding: 0.75rem;
    border-bottom: 1px solid lightgray;

    display: flex;
    gap: 1rem;

    h1 {
      margin: 0;
      font-size: 1.25rem;
    }
  }
</style>
