<script lang="ts">
</script>

<div class="sidebar">
  <div class="top">
    <button class="item">Home</button>
    <button class="item">Mail</button>
    <button class="item">Recipes</button>
  </div>

  <hr />

  <div class="bottom">
    <button class="item">Settings</button>
  </div>
</div>

<style lang="scss" scoped>
  .sidebar {
    display: flex;
    flex-direction: column;

    .top,
    .bottom {
      padding: 0.75rem;

      display: flex;
      flex-direction: column;
      gap: 0.75rem;

      .item {
        cursor: pointer;
        font-family: inherit;
        background: transparent;
        border: 1px solid gray;
        border-radius: 0.5rem;
        aspect-ratio: 1;

        padding: 0.5rem;
      }
    }

    .top {
      flex-grow: 1;
    }
  }
</style>
