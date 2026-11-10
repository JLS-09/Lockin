<script lang="ts">
  // Placeholder until the week view arrives in phase 2.
  import mark from "$lib/assets/lockin-mark.svg?raw";
</script>

<main>
  <div class="logo">
    <span class="mark" aria-hidden="true">{@html mark}</span>
    <h1>Lockin</h1>
  </div>
  <p>Phase 1: the schedule import works from the command line. The week view comes in phase 2.</p>
</main>

<style>
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: #15171c;
    color: #e4e6eb;
    font-family: system-ui, sans-serif;
  }

  main {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .mark :global(svg) {
    display: block;
    width: 56px;
    height: 56px;
  }

  h1 {
    margin: 0;
    font-size: 56px;
    font-weight: 700;
    letter-spacing: -0.04em;
  }

  p {
    margin: 0;
    color: #9aa0aa;
  }
</style>
