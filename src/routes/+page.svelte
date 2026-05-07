<script lang="ts">
  import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type SearchSource = "applications" | "files" | "folders";
  type SearchAction = "launch_app" | "open_path" | "reveal_path" | "copy_path";
  type PathAction = Exclude<SearchAction, "launch_app">;
  type ShortcutAction = Extract<SearchAction, "reveal_path" | "copy_path">;

  type ApplicationMetadata = {
    kind: "application";
    app_id: string;
    terminal: boolean;
  };

  type FileMetadata = {
    kind: "file";
    extension: string | null;
    modified_time_ms: number | null;
  };

  type FolderMetadata = {
    kind: "folder";
  };

  type SearchMetadata = ApplicationMetadata | FileMetadata | FolderMetadata;

  type SearchResult = {
    id: string;
    title: string;
    subtitle: string | null;
    icon: string | null;
    source: SearchSource;
    action: SearchAction;
    path: string | null;
    score: number;
    metadata: SearchMetadata | null;
  };

  let query = $state("");
  let isExpanded = $state(false);
  let isCollapsing = $state(false);
  let results = $state<SearchResult[]>([]);
  let selectedIndex = $state(-1);
  let searchError = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let isRunningAction = $state(false);
  let searchInput: HTMLInputElement;
  let searchRequestId = 0;
  let requestedExpandedState: boolean | null = null;
  let collapseTimer: ReturnType<typeof setTimeout> | null = null;

  const COLLAPSE_TRANSITION_MS = 120;

  $effect(() => {
    void loadResults(query);
  });

  function focusSearchInput() {
    requestAnimationFrame(() => searchInput?.focus());
  }

  function prefersReducedMotion() {
    return (
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    );
  }

  function clearResultState() {
    results = [];
    selectedIndex = -1;
    searchError = null;
    actionError = null;
  }

  function clearCollapseTimer() {
    if (collapseTimer) {
      clearTimeout(collapseTimer);
      collapseTimer = null;
    }
  }

  function collapseToCompactNow() {
    clearCollapseTimer();
    clearResultState();
    isExpanded = false;
    isCollapsing = false;

    if (isTauri()) {
      void setNativeExpanded(false);
    }
  }

  function cancelPendingCollapse() {
    clearCollapseTimer();
    isCollapsing = false;
  }

  function startCollapseToCompact() {
    if (!isExpanded || prefersReducedMotion()) {
      collapseToCompactNow();
      return;
    }

    clearCollapseTimer();
    searchError = null;
    actionError = null;
    isCollapsing = true;

    collapseTimer = setTimeout(() => {
      collapseToCompactNow();
    }, COLLAPSE_TRANSITION_MS);
  }

  async function loadResults(searchQuery: string) {
    const requestId = ++searchRequestId;
    const trimmedQuery = searchQuery.trim();

    if (trimmedQuery.length === 0) {
      startCollapseToCompact();
      return;
    }

    cancelPendingCollapse();
    isExpanded = true;

    if (!isTauri()) {
      results = [];
      selectedIndex = -1;
      searchError = null;
      actionError = null;
      return;
    }

    try {
      await setNativeExpanded(true);

      const nextResults = await invoke<SearchResult[]>("search", {
        query: trimmedQuery,
        limit: 0,
      });

      if (requestId !== searchRequestId || query.trim().length === 0) {
        return;
      }

      results = nextResults;
      selectedIndex = nextResults.length > 0 ? 0 : -1;
      searchError = null;
      actionError = null;
    } catch (error) {
      if (requestId !== searchRequestId) {
        return;
      }

      console.error("failed to search apps", error);
      results = [];
      selectedIndex = -1;
      searchError = "Search unavailable";
      actionError = null;
    }
  }

  async function setNativeExpanded(expanded: boolean) {
    if (requestedExpandedState === expanded) {
      return;
    }

    requestedExpandedState = expanded;

    try {
      await invoke("set_launcher_expanded", { expanded });
    } catch (error) {
      console.error("failed to resize launcher", error);
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

  function symbolicIconClass(icon: string | null) {
    switch (icon) {
      case "app":
        return "symbolic-icon symbolic-app";
      case "folder":
        return "symbolic-icon symbolic-folder";
      case "file":
        return "symbolic-icon symbolic-file";
      case "file-pdf":
        return "symbolic-icon symbolic-file symbolic-file-pdf";
      case "file-text":
        return "symbolic-icon symbolic-file symbolic-file-text";
      case "file-image":
        return "symbolic-icon symbolic-file symbolic-file-image";
      case "file-video":
        return "symbolic-icon symbolic-file symbolic-file-video";
      case "file-audio":
        return "symbolic-icon symbolic-file symbolic-file-audio";
      case "file-archive":
        return "symbolic-icon symbolic-file symbolic-file-archive";
      case "file-document":
        return "symbolic-icon symbolic-file symbolic-file-document";
      default:
        return null;
    }
  }

  function sourceLabel(source: SearchSource) {
    switch (source) {
      case "applications":
        return "App";
      case "files":
        return "File";
      case "folders":
        return "Folder";
    }
  }

  function displaySubtitle(result: SearchResult) {
    if (result.source === "applications") {
      return result.subtitle ?? "";
    }

    if (result.source === "folders") {
      return joinSubtitleParts([result.subtitle, "Folder"]);
    }

    const metadata = result.metadata?.kind === "file" ? result.metadata : null;
    return joinSubtitleParts([
      result.subtitle,
      extensionLabel(metadata?.extension ?? null),
      modifiedLabel(metadata?.modified_time_ms ?? null),
    ]);
  }

  function joinSubtitleParts(parts: Array<string | null | undefined>) {
    return parts.map((part) => part?.trim()).filter(Boolean).join(" - ");
  }

  function extensionLabel(extension: string | null) {
    const value = extension?.trim();
    return value ? value.toUpperCase() : "File";
  }

  function modifiedLabel(modifiedTimeMs: number | null) {
    if (modifiedTimeMs === null || !Number.isFinite(modifiedTimeMs)) {
      return null;
    }

    const elapsedMs = Date.now() - modifiedTimeMs;
    const dayMs = 24 * 60 * 60 * 1000;

    if (elapsedMs < dayMs) {
      return "Modified recently";
    }

    const days = Math.max(1, Math.floor(elapsedMs / dayMs));

    if (days === 1) {
      return "Modified yesterday";
    }

    return `Modified ${days} days ago`;
  }

  function iconFallback(title: string) {
    return title.trim().charAt(0).toUpperCase() || "?";
  }

  function moveSelection(delta: number) {
    if (results.length === 0) {
      selectedIndex = -1;
      return;
    }

    const nextIndex = selectedIndex < 0 ? 0 : selectedIndex + delta;
    selectedIndex = ((nextIndex % results.length) + results.length) % results.length;
  }

  function selectedResult() {
    if (selectedIndex < 0 || selectedIndex >= results.length) {
      return null;
    }

    return results[selectedIndex];
  }

  function isFileSystemResult(result: SearchResult) {
    return result.source === "files" || result.source === "folders";
  }

  function isPathAction(action: SearchAction): action is PathAction {
    return action !== "launch_app";
  }

  function selectedResultCanRunShortcut(action: ShortcutAction) {
    const selected = selectedResult();

    return Boolean(
      selected &&
        isFileSystemResult(selected) &&
        selected.path &&
        !isRunningAction &&
        !isCollapsing &&
        (action === "reveal_path" || action === "copy_path"),
    );
  }

  function searchInputHasSelection() {
    return (
      searchInput?.selectionStart !== null &&
      searchInput?.selectionEnd !== null &&
      searchInput?.selectionStart !== searchInput?.selectionEnd
    );
  }

  function actionFailureMessage(action: SearchAction) {
    if (action === "launch_app") {
      return "Could not launch app";
    }

    if (action === "copy_path") {
      return "Could not complete action";
    }

    return "Could not open item";
  }

  async function runSelectedResultAction(actionOverride?: ShortcutAction) {
    const selected = selectedResult();

    if (!selected || isRunningAction || isCollapsing || !isTauri()) {
      return;
    }

    if (actionOverride && !isFileSystemResult(selected)) {
      return;
    }

    const action = actionOverride ?? selected.action;

    isRunningAction = true;
    actionError = null;

    try {
      if (action === "launch_app") {
        await invoke("launch_app", { appId: selected.id });
      } else if (isPathAction(action)) {
        if (!selected.path || !isFileSystemResult(selected)) {
          actionError = actionFailureMessage(action);
          focusSearchInput();
          return;
        }

        await invoke(action, { path: selected.path });
      }

      query = "";
      results = [];
      selectedIndex = -1;
      searchError = null;
      actionError = null;
      isExpanded = false;
    } catch (error) {
      console.error("failed to run selected action", error);
      actionError = actionFailureMessage(action);
      focusSearchInput();
    } finally {
      isRunningAction = false;
    }
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
      clearCollapseTimer();
      unlisten?.();
    };
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      query = "";
      actionError = null;

      if (isTauri()) {
        void invoke("close_launcher").catch((error) => {
          console.error("failed to close launcher", error);
        });
        return;
      }

      searchInput?.blur();
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      actionError = null;
      moveSelection(1);
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      actionError = null;
      moveSelection(-1);
      return;
    }

    if (event.key === "Tab") {
      event.preventDefault();
      actionError = null;
      moveSelection(event.shiftKey ? -1 : 1);
      return;
    }

    if (event.ctrlKey && event.key === "Enter") {
      if (selectedResultCanRunShortcut("reveal_path")) {
        event.preventDefault();
        actionError = null;
        void runSelectedResultAction("reveal_path");
      }

      return;
    }

    if (event.ctrlKey && event.key.toLowerCase() === "c") {
      if (!searchInputHasSelection() && selectedResultCanRunShortcut("copy_path")) {
        event.preventDefault();
        actionError = null;
        void runSelectedResultAction("copy_path");
      }

      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      void runSelectedResultAction();
    }
  }
</script>

<main class="launcher-shell">
  <section class:expanded={isExpanded} class="command-palette" aria-label="Rat Search">
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

    {#if isExpanded}
      <div class:collapsing={isCollapsing} class="results-region">
        {#if results.length > 0}
          <ul class="results-list" aria-label="Search results">
            {#each results as result, index (result.id)}
              {@const imageSrc = iconImageSrc(result.icon)}
              {@const symbolicClass = symbolicIconClass(result.icon)}
              {@const subtitle = displaySubtitle(result)}
              <li class:selected={index === selectedIndex} class="result-row">
                <span class="app-icon" aria-hidden="true">
                  {#if imageSrc}
                    <img src={imageSrc} alt="" />
                  {:else if symbolicClass}
                    <span class={symbolicClass}></span>
                  {:else}
                    <span>{iconFallback(result.title)}</span>
                  {/if}
                </span>
                <span class="result-copy">
                  <span class="result-title">{result.title}</span>
                  <span class="result-subtitle">{subtitle}</span>
                </span>
                <span class="result-source">{sourceLabel(result.source)}</span>
              </li>
            {/each}
          </ul>
        {:else if query.trim().length > 0 || isCollapsing}
          <div class="empty-state">{searchError ?? "No results"}</div>
        {/if}
      </div>
    {/if}

    {#if isExpanded && actionError}
      <div class="status-message" role="status">{actionError}</div>
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
    position: relative;
    width: 100%;
    height: 100%;
    max-width: 100%;
    max-height: 100%;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    gap: 0;
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
    backdrop-filter: blur(18px) saturate(1.35);
    -webkit-backdrop-filter: blur(18px) saturate(1.35);
  }

  .command-palette.expanded {
    grid-template-rows: 60px minmax(0, 1fr);
    gap: 6px;
  }

  .spotlight-bar {
    width: 100%;
    height: 60px;
    max-width: 100%;
    min-height: 60px;
    max-height: 60px;
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
    width: 22px;
    height: 22px;
    position: relative;
    flex: 0 0 auto;
    box-sizing: border-box;
    border: 2.2px solid rgba(58, 60, 67, 0.46);
    border-radius: 50%;
  }

  .search-icon::after {
    content: "";
    position: absolute;
    width: 9px;
    height: 2.2px;
    right: -6px;
    bottom: 1px;
    border-radius: 999px;
    background: rgba(58, 60, 67, 0.46);
    transform: rotate(45deg);
    transform-origin: left center;
  }

  .search-input {
    min-width: 0;
    width: 100%;
    height: 60px;
    max-width: 100%;
    max-height: 60px;
    border: 0;
    outline: 0;
    padding: 0;
    overflow: hidden !important;
    appearance: none;
    color: rgba(18, 18, 20, 0.94);
    background: transparent;
    font: inherit;
    font-size: 24px;
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

  .results-region {
    min-height: 0;
    width: 100%;
    height: 100%;
    overflow: hidden !important;
  }

  .results-region > .results-list,
  .results-region > .empty-state {
    opacity: 1;
    transform: translateY(0);
    transition:
      opacity 120ms ease-out,
      transform 120ms ease-out;
    animation: results-reveal 120ms ease-out;
  }

  .results-region.collapsing > .results-list,
  .results-region.collapsing > .empty-state {
    opacity: 0;
    transform: translateY(-6px);
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
    grid-template-columns: 34px minmax(0, 1fr) 52px;
    align-items: center;
    gap: 12px;
    padding: 0 12px 0 10px;
    overflow: hidden !important;
    border-radius: 10px;
    color: rgba(18, 18, 20, 0.9);
  }

  .result-row.selected {
    background: rgba(12, 113, 238, 0.12);
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

  .symbolic-icon {
    position: relative;
    display: block;
    width: 20px;
    height: 20px;
    color: currentColor;
  }

  .symbolic-app {
    width: 18px;
    height: 18px;
  }

  .symbolic-app::before {
    content: "";
    position: absolute;
    top: 1px;
    left: 1px;
    width: 7px;
    height: 7px;
    border-radius: 2px;
    background: currentColor;
    box-shadow:
      9px 0 0 currentColor,
      0 9px 0 currentColor,
      9px 9px 0 currentColor;
    opacity: 0.78;
  }

  .symbolic-folder {
    width: 22px;
    height: 18px;
  }

  .symbolic-folder::before {
    content: "";
    position: absolute;
    left: 1px;
    top: 5px;
    width: 20px;
    height: 12px;
    border-radius: 3px;
    background: currentColor;
    opacity: 0.72;
  }

  .symbolic-folder::after {
    content: "";
    position: absolute;
    left: 2px;
    top: 2px;
    width: 9px;
    height: 5px;
    border-radius: 3px 3px 0 0;
    background: currentColor;
    opacity: 0.62;
  }

  .symbolic-file {
    width: 18px;
    height: 21px;
  }

  .symbolic-file::before {
    content: "";
    position: absolute;
    inset: 1px 3px 1px 2px;
    border: 1.8px solid currentColor;
    border-radius: 3px;
    opacity: 0.78;
  }

  .symbolic-file::after {
    content: "";
    position: absolute;
    right: 3px;
    top: 1px;
    width: 6px;
    height: 6px;
    border-left: 1.8px solid currentColor;
    border-bottom: 1.8px solid currentColor;
    border-radius: 0 3px 0 2px;
    background: rgba(246, 247, 248, 0.95);
    opacity: 0.78;
  }

  .symbolic-file-pdf::before {
    color: rgba(206, 48, 48, 0.92);
  }

  .symbolic-file-text::before {
    color: rgba(58, 91, 168, 0.9);
  }

  .symbolic-file-image::before {
    color: rgba(33, 139, 91, 0.9);
  }

  .symbolic-file-video::before {
    color: rgba(145, 75, 198, 0.9);
  }

  .symbolic-file-audio::before {
    color: rgba(196, 111, 32, 0.92);
  }

  .symbolic-file-archive::before {
    color: rgba(128, 91, 48, 0.92);
  }

  .symbolic-file-document::before {
    color: rgba(44, 111, 184, 0.9);
  }

  .result-copy {
    min-width: 0;
    display: grid;
    grid-template-rows: 17px 14px;
    gap: 1px;
    overflow: hidden;
  }

  .result-title,
  .result-subtitle,
  .result-source {
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

  .result-source {
    width: 44px;
    justify-self: end;
    padding: 2px 6px;
    border-radius: 6px;
    background: rgba(58, 60, 67, 0.1);
    color: rgba(58, 60, 67, 0.62);
    font-size: 10px;
    font-weight: 700;
    line-height: 1;
    text-align: center;
    text-transform: uppercase;
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

  @keyframes results-reveal {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }

    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .status-message {
    position: absolute;
    right: 16px;
    bottom: 14px;
    max-width: calc(100% - 32px);
    padding: 6px 10px;
    overflow: hidden;
    border-radius: 9px;
    background: rgba(18, 18, 20, 0.78);
    color: rgba(255, 255, 255, 0.92);
    font-size: 12px;
    font-weight: 500;
    line-height: 1.2;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  @media (prefers-reduced-motion: reduce) {
    .results-region > .results-list,
    .results-region > .empty-state,
    .results-region.collapsing > .results-list,
    .results-region.collapsing > .empty-state {
      animation: none;
      transform: none;
      transition: none;
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
      background: rgba(82, 155, 255, 0.2);
    }

    .app-icon {
      background: rgba(246, 247, 249, 0.12);
      color: rgba(246, 247, 249, 0.8);
    }

    .result-source {
      background: rgba(246, 247, 249, 0.12);
      color: rgba(246, 247, 249, 0.58);
    }

    .symbolic-file::after {
      background: rgba(39, 40, 44, 0.95);
    }

    .result-subtitle,
    .empty-state {
      color: rgba(246, 247, 249, 0.56);
    }

    .status-message {
      background: rgba(246, 247, 249, 0.88);
      color: rgba(18, 18, 20, 0.9);
    }
  }
</style>
