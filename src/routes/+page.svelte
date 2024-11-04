<script>
  import "../global.scss";
  import Sidebar from "../components/composite/sidebar/Sidebar.svelte";
</script>

<div class="container">
  <Sidebar />
  <main class="container">main shit</main>
</div>

<style lang="scss" scoped>
  .container {
    width: 100%;
    height: 100%;

    display: flex;
    flex-direction: row;
    align-items: stretch;
  }
</style>
