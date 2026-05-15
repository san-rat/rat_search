<script lang="ts">
  import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type SearchSource =
    | "applications"
    | "files"
    | "folders"
    | "calculator"
    | "web"
    | "settings"
    | "clipboard"
    | "history";
  type SearchAction =
    | "launch_app"
    | "open_path"
    | "open_in_code"
    | "reveal_path"
    | "copy_path"
    | "copy_text"
    | "open_calculator_app"
    | "open_url"
    | "open_setting"
    | "copy_clipboard_text"
    | "delete_clipboard_item"
    | "reuse_query";
  type PathAction = "open_path" | "open_in_code" | "reveal_path" | "copy_path";
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

  type CalculatorMetadata = {
    kind: "calculator";
    expression: string;
    result: string;
    copy_text: string;
  };

  type WebMetadata = {
    kind: "web";
    shortcut: string;
    query: string;
    url: string;
  };

  type SettingMetadata = {
    kind: "setting";
    setting_id: string;
    panel: string;
    command: string;
  };

  type HistoryMetadata = {
    kind: "history";
    query: string;
    last_used_ms: number;
    use_count: number;
  };

  type ClipboardMetadata = {
    kind: "clipboard";
    item_id: string;
    preview: string;
    copied_at_ms: number;
    last_used_ms: number | null;
    use_count: number;
    text_len: number;
  };

  type SearchMetadata =
    | ApplicationMetadata
    | FileMetadata
    | FolderMetadata
    | CalculatorMetadata
    | WebMetadata
    | SettingMetadata
    | ClipboardMetadata
    | HistoryMetadata;

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

  type ClipboardPrivacyStatus = {
    enabled: boolean;
    entry_count: number;
    retention_days: number;
    max_entries: number;
    max_text_bytes: number;
  };

  let query = $state("");
  let isExpanded = $state(false);
  let isCollapsing = $state(false);
  let isVisuallyCompact = $state(true);
  let hasExpandedThisSession = $state(false);
  let results = $state<SearchResult[]>([]);
  let selectedIndex = $state(-1);
  let searchError = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let isRunningAction = $state(false);
  let isPrivacyPanelOpen = $state(false);
  let clipboardPrivacyStatus = $state<ClipboardPrivacyStatus | null>(null);
  let clipboardPrivacyError = $state<string | null>(null);
  let isClipboardPrivacyBusy = $state(false);
  let hasConfirmedClipboardEnable = $state(false);
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

  async function refreshClipboardPrivacyStatus() {
    if (!isTauri()) {
      return;
    }

    try {
      clipboardPrivacyStatus = await invoke<ClipboardPrivacyStatus>(
        "get_clipboard_privacy_status",
      );
      clipboardPrivacyError = null;
    } catch (error) {
      console.error("failed to load clipboard privacy status", error);
      clipboardPrivacyError = "Clipboard status unavailable";
    }
  }

  function togglePrivacyPanel() {
    isPrivacyPanelOpen = !isPrivacyPanelOpen;

    if (isPrivacyPanelOpen) {
      void refreshClipboardPrivacyStatus();
    }
  }

  async function runClipboardPrivacyCommand(command: string) {
    if (!isTauri() || isClipboardPrivacyBusy) {
      return;
    }

    if (command === "enable_clipboard_history" && !hasConfirmedClipboardEnable) {
      hasConfirmedClipboardEnable = true;
      clipboardPrivacyError = "Clipboard history is local. Select Enable again.";
      focusSearchInput();
      return;
    }

    isClipboardPrivacyBusy = true;
    clipboardPrivacyError = null;

    try {
      await invoke(command);

      if (command === "clear_clipboard_history") {
        results = results.filter((result) => result.source !== "clipboard");
        selectedIndex = results.length > 0 ? Math.min(selectedIndex, results.length - 1) : -1;
      }

      if (command !== "enable_clipboard_history") {
        hasConfirmedClipboardEnable = false;
      }

      await refreshClipboardPrivacyStatus();
      focusSearchInput();
    } catch (error) {
      console.error("failed to update clipboard privacy", error);
      clipboardPrivacyError = "Clipboard action failed";
      focusSearchInput();
    } finally {
      isClipboardPrivacyBusy = false;
    }
  }

  function clearCollapseTimer() {
    if (collapseTimer) {
      clearTimeout(collapseTimer);
      collapseTimer = null;
    }
  }

  function collapseToVisualCompactNow() {
    clearCollapseTimer();
    clearResultState();
    isCollapsing = false;
    isVisuallyCompact = true;
  }

  function cancelPendingCollapse() {
    clearCollapseTimer();
    isCollapsing = false;
  }

  function startCollapseToCompact() {
    if (isVisuallyCompact && !isCollapsing) {
      clearResultState();
      return;
    }

    if (prefersReducedMotion()) {
      collapseToVisualCompactNow();
      return;
    }

    clearCollapseTimer();
    searchError = null;
    actionError = null;
    isCollapsing = true;

    collapseTimer = setTimeout(() => {
      collapseToVisualCompactNow();
    }, COLLAPSE_TRANSITION_MS);
  }

  function useExpandedChrome() {
    return hasExpandedThisSession && !isVisuallyCompact;
  }

  function resetVisualSessionState() {
    clearCollapseTimer();
    clearResultState();
    isExpanded = false;
    isCollapsing = false;
    isVisuallyCompact = true;
    hasExpandedThisSession = false;
  }

  async function resetToNativeCompact() {
    resetVisualSessionState();

    if (isTauri()) {
      await setNativeExpanded(false);
    }
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
    isVisuallyCompact = false;
    hasExpandedThisSession = true;

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
      case "calculator":
        return "symbolic-icon symbolic-calculator";
      case "web":
        return "symbolic-icon symbolic-web";
      case "settings":
        return "symbolic-icon symbolic-settings";
      case "clipboard":
        return "symbolic-icon symbolic-clipboard";
      case "history":
        return "symbolic-icon symbolic-history";
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
      case "calculator":
        return "Calc";
      case "web":
        return "Web";
      case "settings":
        return "Set";
      case "clipboard":
        return "Clip";
      case "history":
        return "Hist";
    }
  }

  function displaySubtitle(result: SearchResult) {
    switch (result.source) {
      case "applications":
        return result.subtitle ?? "";
      case "folders":
        return joinSubtitleParts([result.subtitle, "Folder"]);
      case "files": {
        const metadata = result.metadata?.kind === "file" ? result.metadata : null;
        return joinSubtitleParts([
          result.subtitle,
          extensionLabel(metadata?.extension ?? null),
          modifiedLabel(metadata?.modified_time_ms ?? null),
        ]);
      }
      case "calculator":
        return firstSubtitlePart([calculatorMetadata(result)?.expression, result.subtitle]);
      case "web": {
        const metadata = webMetadata(result);
        return firstSubtitlePart([
          metadata?.query,
          result.subtitle,
          webHostLabel(metadata?.url ?? null),
        ]);
      }
      case "settings":
        return firstSubtitlePart(["System Settings", result.subtitle]);
      case "clipboard": {
        const metadata = clipboardMetadata(result);
        return firstSubtitlePart([
          result.subtitle,
          metadata && metadata.use_count > 0
            ? `Clipboard - used ${metadata.use_count} times`
            : null,
        ]);
      }
      case "history": {
        const metadata = historyMetadata(result);
        return firstSubtitlePart([
          metadata ? `Search history - used ${metadata.use_count} times` : null,
          result.subtitle,
        ]);
      }
    }
  }

  function joinSubtitleParts(parts: Array<string | null | undefined>) {
    return parts.map((part) => part?.trim()).filter(Boolean).join(" - ");
  }

  function firstSubtitlePart(parts: Array<string | null | undefined>) {
    return parts.map((part) => part?.trim()).find(Boolean) ?? "";
  }

  function webHostLabel(url: string | null) {
    if (!url) {
      return null;
    }

    try {
      return new URL(url).host;
    } catch {
      return null;
    }
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
    return (
      action === "open_path" ||
      action === "open_in_code" ||
      action === "reveal_path" ||
      action === "copy_path"
    );
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

  function calculatorMetadata(result: SearchResult) {
    return result.metadata?.kind === "calculator" ? result.metadata : null;
  }

  function webMetadata(result: SearchResult) {
    return result.metadata?.kind === "web" ? result.metadata : null;
  }

  function settingMetadata(result: SearchResult) {
    return result.metadata?.kind === "setting" ? result.metadata : null;
  }

  function historyMetadata(result: SearchResult) {
    return result.metadata?.kind === "history" ? result.metadata : null;
  }

  function clipboardMetadata(result: SearchResult) {
    return result.metadata?.kind === "clipboard" ? result.metadata : null;
  }

  function actionFailureMessage(action: SearchAction) {
    if (action === "launch_app") {
      return "Could not launch app";
    }

    if (action === "copy_path" || action === "copy_text") {
      return "Could not complete action";
    }

    if (action === "open_in_code") {
      return "Could not open item";
    }

    if (action === "open_calculator_app") {
      return "Could not open calculator";
    }

    if (action === "copy_clipboard_text" || action === "delete_clipboard_item") {
      return "Could not complete action";
    }

    return "Could not open item";
  }

  function failSelectedAction(action: SearchAction) {
    actionError = actionFailureMessage(action);
    focusSearchInput();
  }

  function recordSearchHistory(queryBeforeAction: string) {
    if (!queryBeforeAction.trim()) {
      return;
    }

    void invoke("record_search_history", { query: queryBeforeAction }).catch((error) => {
      console.error("failed to record search history", error);
    });
  }

  function removeResultById(resultId: string) {
    const nextResults = results.filter((result) => result.id !== resultId);
    results = nextResults;

    if (nextResults.length === 0) {
      selectedIndex = -1;
      return;
    }

    selectedIndex = Math.min(selectedIndex, nextResults.length - 1);
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
    const queryBeforeAction = query.trim();

    isRunningAction = true;
    actionError = null;

    try {
      switch (action) {
        case "launch_app":
          await invoke("launch_app", { appId: selected.id });
          break;

        case "open_path":
        case "open_in_code":
        case "reveal_path":
        case "copy_path":
          if (!selected.path || !isFileSystemResult(selected)) {
            failSelectedAction(action);
            return;
          }

          await invoke(action, { path: selected.path });
          break;

        case "copy_text": {
          const metadata = calculatorMetadata(selected);

          if (!metadata) {
            failSelectedAction(action);
            return;
          }

          await invoke("copy_text", { text: metadata.copy_text });
          break;
        }

        case "open_calculator_app": {
          const metadata = calculatorMetadata(selected);

          if (!metadata) {
            failSelectedAction(action);
            return;
          }

          await invoke("open_calculator_app", {
            expression: metadata.expression,
            result: metadata.result,
            copyText: metadata.copy_text,
          });
          break;
        }

        case "open_url": {
          const metadata = webMetadata(selected);

          if (!metadata) {
            failSelectedAction(action);
            return;
          }

          await invoke("open_url", { url: metadata.url });
          break;
        }

        case "open_setting": {
          const metadata = settingMetadata(selected);

          if (!metadata) {
            failSelectedAction(action);
            return;
          }

          await invoke("open_setting", { settingId: metadata.setting_id });
          break;
        }

        case "copy_clipboard_text": {
          const metadata = clipboardMetadata(selected);

          if (!metadata) {
            failSelectedAction(action);
            return;
          }

          await invoke("copy_clipboard_item", { itemId: metadata.item_id });
          break;
        }

        case "delete_clipboard_item": {
          const metadata = clipboardMetadata(selected);

          if (!metadata) {
            failSelectedAction(action);
            return;
          }

          await invoke("delete_clipboard_item", { itemId: metadata.item_id });
          removeResultById(selected.id);
          searchError = null;
          actionError = null;
          focusSearchInput();
          return;
        }

        case "reuse_query": {
          const metadata = historyMetadata(selected);

          if (!metadata) {
            failSelectedAction(action);
            return;
          }

          query = metadata.query;
          searchError = null;
          actionError = null;
          focusSearchInput();
          return;
        }
      }

      recordSearchHistory(queryBeforeAction);
      query = "";
      results = [];
      selectedIndex = -1;
      searchError = null;
      actionError = null;
      await resetToNativeCompact();
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

    void listen("launcher:shown", () => {
      query = "";
      resetVisualSessionState();
      requestedExpandedState = false;
      focusSearchInput();
    })
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
        void invoke("close_launcher")
          .then(() => resetToNativeCompact())
          .catch((error) => {
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
  <section
    class:expanded={useExpandedChrome()}
    class:visually-compact={isVisuallyCompact}
    class="command-palette"
    aria-label="Rat Search"
  >
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
      <button
        class:active={isPrivacyPanelOpen}
        class="privacy-toggle"
        type="button"
        aria-label="Clipboard privacy"
        onclick={togglePrivacyPanel}
      >
        <span class="symbolic-icon symbolic-clipboard"></span>
      </button>
    </div>

    {#if isPrivacyPanelOpen}
      <div class="privacy-panel" role="dialog" aria-label="Clipboard privacy">
        <div class="privacy-panel-header">
          <span>Clipboard</span>
          <strong>{clipboardPrivacyStatus?.enabled ? "On" : "Off"}</strong>
        </div>
        <div class="privacy-panel-details">
          <span>{clipboardPrivacyStatus?.entry_count ?? 0} items</span>
          <span>{clipboardPrivacyStatus?.retention_days ?? 7} days</span>
          <span>{clipboardPrivacyStatus?.max_entries ?? 100} max</span>
          <span>{clipboardPrivacyStatus?.max_text_bytes ?? 10000} bytes</span>
        </div>
        {#if clipboardPrivacyError}
          <div class="privacy-panel-note">{clipboardPrivacyError}</div>
        {/if}
        <div class="privacy-panel-actions">
          {#if clipboardPrivacyStatus?.enabled}
            <button
              type="button"
              disabled={isClipboardPrivacyBusy}
              onclick={() => runClipboardPrivacyCommand("disable_clipboard_history")}
            >
              Disable
            </button>
          {:else}
            <button
              type="button"
              disabled={isClipboardPrivacyBusy}
              onclick={() => runClipboardPrivacyCommand("enable_clipboard_history")}
            >
              Enable
            </button>
          {/if}
          <button
            type="button"
            disabled={isClipboardPrivacyBusy}
            onclick={() => runClipboardPrivacyCommand("clear_clipboard_history")}
          >
            Clear
          </button>
        </div>
      </div>
    {/if}

    {#if isExpanded && !isVisuallyCompact}
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
                  <span class="result-title-row">
                    <span class="result-title">{result.title}</span>
                    <span class="result-source">{sourceLabel(result.source)}</span>
                  </span>
                  <span class="result-subtitle">{subtitle}</span>
                </span>
              </li>
            {/each}
          </ul>
        {:else if query.trim().length > 0 || isCollapsing}
          <div class="empty-state">{searchError ?? "No results"}</div>
        {/if}
      </div>
    {/if}

    {#if isExpanded && !isVisuallyCompact && actionError}
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
    align-items: start;
    justify-items: center;
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
    padding: 4px;
    overflow: hidden !important;
    border: 1px solid rgba(255, 255, 255, 0.64);
    border-radius: 17px;
    background: rgba(246, 247, 248, 0.95);
    box-shadow:
      0 8px 22px rgba(0, 0, 0, 0.14),
      0 2px 8px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.7);
    transition: height 120ms ease-out;
    -ms-overflow-style: none;
    overscroll-behavior: none;
    backdrop-filter: blur(18px) saturate(1.35);
    -webkit-backdrop-filter: blur(18px) saturate(1.35);
  }

  .command-palette.visually-compact {
    height: 68px;
  }

  .spotlight-bar {
    position: absolute;
    z-index: 2;
    top: 4px;
    right: 4px;
    left: 4px;
    height: 60px;
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

  .privacy-toggle {
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: rgba(58, 60, 67, 0.62);
  }

  .privacy-toggle.active,
  .privacy-toggle:hover {
    background: rgba(58, 60, 67, 0.1);
    color: rgba(18, 18, 20, 0.84);
  }

  .privacy-panel {
    position: absolute;
    z-index: 4;
    top: 58px;
    right: 12px;
    width: min(300px, calc(100% - 24px));
    padding: 10px;
    overflow: hidden;
    border: 1px solid rgba(58, 60, 67, 0.12);
    border-radius: 10px;
    background: rgba(250, 251, 252, 0.98);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.16);
  }

  .privacy-panel-header,
  .privacy-panel-details,
  .privacy-panel-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .privacy-panel-header {
    justify-content: space-between;
    color: rgba(18, 18, 20, 0.9);
    font-size: 13px;
    font-weight: 650;
  }

  .privacy-panel-details {
    flex-wrap: wrap;
    margin-top: 7px;
    color: rgba(58, 60, 67, 0.66);
    font-size: 11px;
    line-height: 1.2;
  }

  .privacy-panel-note {
    margin-top: 8px;
    color: rgba(127, 82, 20, 0.92);
    font-size: 11px;
    line-height: 1.25;
  }

  .privacy-panel-actions {
    justify-content: flex-end;
    margin-top: 10px;
  }

  .privacy-panel-actions button {
    height: 26px;
    padding: 0 10px;
    border: 0;
    border-radius: 7px;
    background: rgba(12, 113, 238, 0.12);
    color: rgba(12, 83, 173, 0.96);
    font-size: 12px;
    font-weight: 650;
  }

  .privacy-panel-actions button:disabled {
    opacity: 0.54;
  }

  .results-region {
    position: absolute;
    top: 70px;
    right: 4px;
    bottom: 4px;
    left: 4px;
    min-height: 0;
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
    grid-template-columns: 34px minmax(0, 1fr);
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

  .symbolic-calculator {
    width: 18px;
    height: 21px;
  }

  .symbolic-calculator::before {
    content: "";
    position: absolute;
    inset: 1px 2px;
    border: 1.7px solid currentColor;
    border-radius: 3px;
    opacity: 0.82;
  }

  .symbolic-calculator::after {
    content: "";
    position: absolute;
    top: 5px;
    left: 6px;
    width: 6px;
    height: 2px;
    border-radius: 1px;
    background: currentColor;
    box-shadow:
      -2px 6px 0 currentColor,
      4px 6px 0 currentColor,
      -2px 10px 0 currentColor,
      4px 10px 0 currentColor;
    opacity: 0.78;
  }

  .symbolic-web {
    width: 21px;
    height: 21px;
  }

  .symbolic-web::before {
    content: "";
    position: absolute;
    inset: 2px;
    border: 1.8px solid currentColor;
    border-radius: 50%;
    opacity: 0.78;
  }

  .symbolic-web::after {
    content: "";
    position: absolute;
    top: 5px;
    right: 2px;
    bottom: 5px;
    left: 2px;
    border-top: 1.6px solid currentColor;
    border-bottom: 1.6px solid currentColor;
    box-shadow:
      inset 5px 0 0 -3.5px currentColor,
      inset -5px 0 0 -3.5px currentColor;
    opacity: 0.72;
  }

  .symbolic-settings {
    width: 20px;
    height: 20px;
  }

  .symbolic-settings::before {
    content: "";
    position: absolute;
    inset: 4px;
    border: 2px solid currentColor;
    border-radius: 50%;
    box-shadow:
      0 -6px 0 -2px currentColor,
      0 6px 0 -2px currentColor,
      -6px 0 0 -2px currentColor,
      6px 0 0 -2px currentColor;
    opacity: 0.78;
  }

  .symbolic-settings::after {
    content: "";
    position: absolute;
    inset: 8px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.78;
  }

  .symbolic-clipboard {
    width: 18px;
    height: 21px;
  }

  .symbolic-clipboard::before {
    content: "";
    position: absolute;
    right: 2px;
    bottom: 1px;
    left: 2px;
    height: 16px;
    border: 1.8px solid currentColor;
    border-radius: 3px;
    opacity: 0.78;
  }

  .symbolic-clipboard::after {
    content: "";
    position: absolute;
    top: 1px;
    left: 5px;
    width: 8px;
    height: 5px;
    border-radius: 3px 3px 1px 1px;
    background: currentColor;
    box-shadow: 0 7px 0 -3px currentColor;
    opacity: 0.74;
  }

  .symbolic-history {
    width: 21px;
    height: 21px;
  }

  .symbolic-history::before {
    content: "";
    position: absolute;
    inset: 2px;
    border: 1.8px solid currentColor;
    border-left-color: transparent;
    border-radius: 50%;
    opacity: 0.78;
  }

  .symbolic-history::after {
    content: "";
    position: absolute;
    top: 5px;
    left: 9px;
    width: 6px;
    height: 6px;
    border-left: 1.8px solid currentColor;
    border-bottom: 1.8px solid currentColor;
    border-radius: 0 0 0 2px;
    opacity: 0.78;
  }

  .result-copy {
    min-width: 0;
    display: grid;
    grid-template-rows: 17px 14px;
    gap: 1px;
    overflow: hidden;
  }

  .result-title-row {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
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
    flex: 0 0 auto;
    max-width: 52px;
    padding: 2px 6px;
    border-radius: 6px;
    background: rgba(58, 60, 67, 0.1);
    color: rgba(58, 60, 67, 0.62);
    font-size: 9px;
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
    .command-palette {
      transition: none;
    }

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

    .privacy-toggle {
      color: rgba(246, 247, 249, 0.62);
    }

    .privacy-toggle.active,
    .privacy-toggle:hover {
      background: rgba(246, 247, 249, 0.12);
      color: rgba(246, 247, 249, 0.9);
    }

    .privacy-panel {
      border-color: rgba(246, 247, 249, 0.14);
      background: rgba(45, 46, 50, 0.98);
    }

    .privacy-panel-header {
      color: rgba(246, 247, 249, 0.94);
    }

    .privacy-panel-details {
      color: rgba(246, 247, 249, 0.62);
    }

    .privacy-panel-note {
      color: rgba(245, 190, 112, 0.95);
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
