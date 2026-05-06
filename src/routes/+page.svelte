<script lang="ts">
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  let query = $state("");
  let searchInput: HTMLInputElement;

  function focusSearchInput() {
    requestAnimationFrame(() => searchInput?.focus());
  }

  onMount(() => {
    focusSearchInput();

    if (!isTauri()) {
      return;
    }

    let disposed = false;
    let unlisten: UnlistenFn | undefined;

    void listen("launcher:shown", focusSearchInput)
      .then((unlistenListener) => {
        if (disposed) {
          unlistenListener();
          return;
        }

        unlisten = unlistenListener;
      })
      .catch((error) => {
        console.error("failed to listen for launcher focus event", error);
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      query = "";

      if (isTauri()) {
        void invoke("close_launcher").catch((error) => {
          console.error("failed to close launcher", error);
        });
        return;
      }

      searchInput?.blur();
    }
  }
</script>

<main class="launcher-shell">
  <section class="spotlight-bar" aria-label="Rat Search">
    <span class="search-icon" aria-hidden="true"></span>
    <input
      bind:this={searchInput}
      bind:value={query}
      class="search-input"
      type="search"
      placeholder="Spotlight Search"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      onkeydown={handleKeydown}
    />
  </section>
</main>

<style>
  :root {
    font-family:
      -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", Ubuntu, sans-serif;
    font-size: 16px;
    line-height: 1.4;
    font-weight: 400;
    color: rgba(18, 18, 20, 0.94);
    background: transparent;
    font-synthesis: none;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    -webkit-text-size-adjust: 100%;
  }

  :global(html),
  :global(body),
  :global(body > div) {
    width: 100%;
    height: 100%;
    max-width: 100vw;
    max-height: 100vh;
    margin: 0;
    overflow: hidden !important;
    border-radius: 21px;
    background: transparent;
    -ms-overflow-style: none;
    overscroll-behavior: none;
    touch-action: none;
  }

  :global(*) {
    box-sizing: border-box;
    -ms-overflow-style: none;
    scrollbar-width: none;
  }

  :global(*::-webkit-scrollbar) {
    width: 0;
    height: 0;
    display: none;
    background: transparent;
  }

  :global(*::-webkit-scrollbar-track),
  :global(*::-webkit-scrollbar-thumb),
  :global(*::-webkit-scrollbar-corner) {
    background: transparent;
  }

  .launcher-shell {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    max-width: 100vw;
    max-height: 100vh;
    display: grid;
    place-items: center;
    padding: 4px;
    box-sizing: border-box;
    overflow: hidden !important;
    border-radius: 21px;
    background: transparent;
    -ms-overflow-style: none;
    overscroll-behavior: none;
    touch-action: none;
  }

  .spotlight-bar {
    width: 100%;
    height: 100%;
    max-width: 100%;
    max-height: 100%;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 24px;
    overflow: hidden !important;
    border: 1px solid rgba(255, 255, 255, 0.64);
    border-radius: 17px;
    background: rgba(246, 247, 248, 0.95);
    box-shadow:
      0 3px 8px rgba(0, 0, 0, 0.12),
      0 1px 3px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.7);
    -ms-overflow-style: none;
    overscroll-behavior: none;
    backdrop-filter: blur(28px) saturate(1.45);
    -webkit-backdrop-filter: blur(28px) saturate(1.45);
  }

  .search-icon {
    width: 25px;
    height: 25px;
    position: relative;
    flex: 0 0 auto;
    box-sizing: border-box;
    border: 2.6px solid rgba(58, 60, 67, 0.46);
    border-radius: 50%;
  }

  .search-icon::after {
    content: "";
    position: absolute;
    width: 10px;
    height: 2.6px;
    right: -7px;
    bottom: 0;
    border-radius: 999px;
    background: rgba(58, 60, 67, 0.46);
    transform: rotate(45deg);
    transform-origin: left center;
  }

  .search-input {
    min-width: 0;
    width: 100%;
    height: 100%;
    max-width: 100%;
    max-height: 100%;
    border: 0;
    outline: 0;
    padding: 0;
    overflow: hidden !important;
    appearance: none;
    color: rgba(18, 18, 20, 0.94);
    background: transparent;
    font: inherit;
    font-size: 28px;
    font-weight: 400;
    line-height: 1;
    caret-color: rgba(12, 113, 238, 0.95);
    -ms-overflow-style: none;
    overscroll-behavior: none;
  }

  .search-input::placeholder {
    color: rgba(58, 60, 67, 0.45);
    opacity: 1;
  }

  .search-input::-webkit-search-decoration,
  .search-input::-webkit-search-cancel-button,
  .search-input::-webkit-search-results-button,
  .search-input::-webkit-search-results-decoration {
    display: none;
  }

  @supports not ((backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: blur(1px))) {
    .spotlight-bar {
      background: rgba(247, 248, 250, 0.98);
    }
  }

  @media (prefers-color-scheme: dark) {
    :root {
      color: rgba(246, 247, 249, 0.96);
    }

    .spotlight-bar {
      border-color: rgba(255, 255, 255, 0.18);
      background: rgba(39, 40, 44, 0.95);
      box-shadow:
        0 3px 9px rgba(0, 0, 0, 0.32),
        0 1px 3px rgba(0, 0, 0, 0.24),
        inset 0 1px 0 rgba(255, 255, 255, 0.15);
    }

    .search-icon {
      border-color: rgba(246, 247, 249, 0.5);
    }

    .search-icon::after {
      background: rgba(246, 247, 249, 0.5);
    }

    .search-input {
      color: rgba(246, 247, 249, 0.96);
    }

    .search-input::placeholder {
      color: rgba(246, 247, 249, 0.45);
    }
  }
</style>
