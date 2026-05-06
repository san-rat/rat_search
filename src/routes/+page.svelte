<script lang="ts">
  import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type AppSearchResult = {
    app_id: string;
    title: string;
    subtitle: string | null;
    icon: string | null;
    terminal: boolean;
  };

  const RESULT_LIMIT = 8;

  let query = $state("");
  let results = $state<AppSearchResult[]>([]);
  let selectedIndex = $state(-1);
  let searchError = $state<string | null>(null);
  let searchInput: HTMLInputElement;
  let searchRequestId = 0;

  $effect(() => {
    void loadResults(query);
  });

  function focusSearchInput() {
    requestAnimationFrame(() => searchInput?.focus());
  }

  async function loadResults(searchQuery: string) {
    const requestId = ++searchRequestId;

    if (!isTauri()) {
      results = [];
      selectedIndex = -1;
      searchError = null;
      return;
    }

    try {
      const nextResults = await invoke<AppSearchResult[]>("search_apps", {
        query: searchQuery,
        limit: RESULT_LIMIT,
      });

      if (requestId !== searchRequestId) {
        return;
      }

      results = nextResults;
      selectedIndex = nextResults.length > 0 ? 0 : -1;
      searchError = null;
    } catch (error) {
      if (requestId !== searchRequestId) {
        return;
      }

      console.error("failed to search apps", error);
      results = [];
      selectedIndex = -1;
      searchError = "Search unavailable";
    }
  }

  function iconImageSrc(icon: string | null) {
    const value = icon?.trim();

    if (!value) {
      return null;
    }

    if (value.startsWith("file://")) {
      return value;
    }

    if (value.startsWith("/") && isTauri()) {
      return convertFileSrc(value);
    }

    if (value.includes("/") && /\.(png|jpe?g|svg|webp|gif|xpm)$/i.test(value)) {
      return value;
    }

    return null;
  }

  function iconFallback(title: string) {
    return title.trim().charAt(0).toUpperCase() || "?";
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
  <section class="command-palette" aria-label="Rat Search">
    <div class="spotlight-bar">
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
    </div>

    {#if results.length > 0}
      <ul class="results-list" aria-label="Search results">
        {#each results as result, index (result.app_id)}
          {@const imageSrc = iconImageSrc(result.icon)}
          <li class:selected={index === selectedIndex} class="result-row">
            <span class="app-icon" aria-hidden="true">
              {#if imageSrc}
                <img src={imageSrc} alt="" />
              {:else}
                <span>{iconFallback(result.title)}</span>
              {/if}
            </span>
            <span class="result-copy">
              <span class="result-title">{result.title}</span>
              {#if result.subtitle}
                <span class="result-subtitle">{result.subtitle}</span>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
    {:else if query.trim().length > 0}
      <div class="empty-state">{searchError ?? "No results"}</div>
    {/if}
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

  .command-palette {
    width: 100%;
    height: 100%;
    max-width: 100%;
    max-height: 100%;
    display: grid;
    grid-template-rows: 68px minmax(0, 1fr);
    gap: 6px;
    padding: 4px;
    overflow: hidden !important;
    border: 1px solid rgba(255, 255, 255, 0.64);
    border-radius: 17px;
    background: rgba(246, 247, 248, 0.95);
    box-shadow:
      0 8px 22px rgba(0, 0, 0, 0.14),
      0 2px 8px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.7);
    -ms-overflow-style: none;
    overscroll-behavior: none;
    backdrop-filter: blur(28px) saturate(1.45);
    -webkit-backdrop-filter: blur(28px) saturate(1.45);
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
    border-radius: 13px;
    background: transparent;
    -ms-overflow-style: none;
    overscroll-behavior: none;
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

  .results-list {
    min-height: 0;
    width: 100%;
    height: 100%;
    display: grid;
    grid-auto-rows: 42px;
    align-content: start;
    gap: 4px;
    margin: 0;
    padding: 4px;
    overflow: hidden !important;
    list-style: none;
    -ms-overflow-style: none;
    overscroll-behavior: none;
  }

  .result-row {
    width: 100%;
    height: 42px;
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    padding: 0 12px 0 10px;
    overflow: hidden !important;
    border-radius: 10px;
    color: rgba(18, 18, 20, 0.9);
  }

  .result-row.selected {
    background: rgba(12, 113, 238, 0.14);
  }

  .app-icon {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    overflow: hidden;
    border-radius: 8px;
    background: rgba(58, 60, 67, 0.12);
    color: rgba(58, 60, 67, 0.76);
    font-size: 13px;
    font-weight: 650;
  }

  .app-icon img {
    width: 24px;
    height: 24px;
    display: block;
    object-fit: contain;
  }

  .result-copy {
    min-width: 0;
    display: grid;
    gap: 1px;
    overflow: hidden;
  }

  .result-title,
  .result-subtitle {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result-title {
    font-size: 14px;
    font-weight: 600;
    line-height: 1.15;
  }

  .result-subtitle {
    color: rgba(58, 60, 67, 0.6);
    font-size: 12px;
    line-height: 1.1;
  }

  .empty-state {
    min-height: 0;
    display: grid;
    place-items: center;
    overflow: hidden !important;
    color: rgba(58, 60, 67, 0.56);
    font-size: 14px;
  }

  .search-input::-webkit-search-decoration,
  .search-input::-webkit-search-cancel-button,
  .search-input::-webkit-search-results-button,
  .search-input::-webkit-search-results-decoration {
    display: none;
  }

  @supports not ((backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: blur(1px))) {
    .command-palette {
      background: rgba(247, 248, 250, 0.98);
    }
  }

  @media (prefers-color-scheme: dark) {
    :root {
      color: rgba(246, 247, 249, 0.96);
    }

    .command-palette {
      border-color: rgba(255, 255, 255, 0.18);
      background: rgba(39, 40, 44, 0.95);
      box-shadow:
        0 8px 24px rgba(0, 0, 0, 0.36),
        0 2px 8px rgba(0, 0, 0, 0.28),
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

    .result-row {
      color: rgba(246, 247, 249, 0.94);
    }

    .result-row.selected {
      background: rgba(82, 155, 255, 0.24);
    }

    .app-icon {
      background: rgba(246, 247, 249, 0.12);
      color: rgba(246, 247, 249, 0.8);
    }

    .result-subtitle,
    .empty-state {
      color: rgba(246, 247, 249, 0.56);
    }
  }
</style>
