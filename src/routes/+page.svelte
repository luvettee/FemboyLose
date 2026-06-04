<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  type Branch = 'Release' | 'Nightly';
  type View = 'boot' | 'launcher' | 'details' | 'closingDetails' | 'launching';
  type ConfigEntry = {
    entry_id: number;
    name: string;
  };
  type LauncherVersion = {
    tag: string;
    name: string;
    changelog: string;
    updated_at: string;
    url: string;
    assets: LauncherAsset[];
  };
  type LauncherAsset = {
    name: string;
    url: string;
    size: number;
  };

  let view = $state<View>('boot');
  let branch = $state<Branch>('Release');
  let branchOpen = $state(false);
  let versionOpen = $state(false);
  let configOpen = $state(false);
  let configs = $state<ConfigEntry[]>([]);
  let selectedConfigId = $state<number | null>(null);
  let gitMetadata = $state<LauncherGitMetadata>({
    releases: [],
    nightlies: []
  });
  let selectedVersion = $state('');
  let launchPending = $state(false);
  let progress = $state(0);
  let themeVariables = $state('');
  let launchTimer: number | undefined;
  let closeTimer: number | undefined;

  const selectedVersionList = $derived(branch === 'Release' ? gitMetadata.releases : gitMetadata.nightlies);
  const selectedVersionData = $derived(
    selectedVersionList.find((version) => version.tag === selectedVersion) ?? selectedVersionList[0]
  );
  const selectedVersionLabel = $derived(selectedVersion || 'No builds');
  const selectedBranchLabel = $derived(`${branch} ${selectedVersionLabel}`);
  const selectedConfigName = $derived(
    configs.find((config) => config.entry_id === selectedConfigId)?.name ?? 'Loading...'
  );
  const versions = $derived(selectedVersionList);
  const updatedAtLabel = $derived(formatGitDate(selectedVersionData?.updated_at));
  const changelogEntries = $derived(parseChangelog(selectedVersionData?.changelog));
  const selectedReleaseUrl = $derived(selectedVersionData?.url ?? '');
  const appWindow = getCurrentWindow();

  type LauncherTheme = {
    source: string;
    variables: Record<string, string>;
  };

  type LauncherSettings = {
    username: string;
    selected_config_id: number | null;
    selected_config_name: string | null;
    configs: ConfigEntry[];
  };

  type LauncherGitMetadata = {
    releases: LauncherVersion[];
    nightlies: LauncherVersion[];
  };

  onMount(() => {
    const blockContextMenu = (event: MouseEvent) => {
      event.preventDefault();
    };

    const blockDevtoolsShortcuts = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      const opensDevtools =
        event.key === 'F12' ||
        (event.ctrlKey && event.shiftKey && (key === 'i' || key === 'j' || key === 'c')) ||
        (event.ctrlKey && key === 'u');

      if (opensDevtools) {
        event.preventDefault();
        event.stopPropagation();
        return;
      }

      if (event.key === 'Escape') {
        closeMenus();
        event.preventDefault();
        event.stopPropagation();
      }
    };

    const closeFloatingMenus = (event: MouseEvent) => {
      const target = event.target as HTMLElement;

      if (target.closest('.branch-trigger, .branch-menu, .metadata-trigger, .metadata-menu, .changelog a')) {
        return;
      }

      closeMenus();
    };

    window.addEventListener('contextmenu', blockContextMenu);
    window.addEventListener('keydown', blockDevtoolsShortcuts, true);
    window.addEventListener('mousedown', closeFloatingMenus, true);
    window.addEventListener('mousedown', dragWindow, true);
    void loadTheme();
    void loadSettings();
    void loadGitMetadata();

    const bootTimer = window.setTimeout(() => {
      showLauncher();
    }, 3000);

    return () => {
      window.clearTimeout(bootTimer);
      window.removeEventListener('contextmenu', blockContextMenu);
      window.removeEventListener('keydown', blockDevtoolsShortcuts, true);
      window.removeEventListener('mousedown', closeFloatingMenus, true);
      window.removeEventListener('mousedown', dragWindow, true);
    };
  });

  function showLauncher() {
    view = 'launcher';
  }

  async function loadTheme() {
    try {
      const theme = await invoke<LauncherTheme>('load_launcher_theme');
      themeVariables = Object.entries(theme.variables)
        .map(([key, value]) => `${key}: ${value}`)
        .join('; ');
    } catch (error) {
      console.warn('Failed to load launcher theme', error);
    }
  }

  async function loadSettings() {
    try {
      const settings = await invoke<LauncherSettings>('load_launcher_settings');
      configs = settings.configs;
      selectedConfigId = settings.selected_config_id ?? settings.configs[0]?.entry_id ?? null;
    } catch (error) {
      console.warn('Failed to load launcher settings', error);
    }
  }

  async function loadGitMetadata() {
    try {
      const metadata = await invoke<LauncherGitMetadata>('load_git_metadata');
      gitMetadata = metadata;
      selectedVersion = metadata.releases[0]?.tag ?? metadata.nightlies[0]?.tag ?? '';
    } catch (error) {
      console.warn('Failed to load GitHub metadata', error);
      gitMetadata = {
        releases: [
          {
            tag: 'Unavailable',
            name: 'Unavailable',
            changelog: 'Could not load GitHub release metadata.',
            updated_at: '',
            url: '',
            assets: []
          }
        ],
        nightlies: []
      };
      selectedVersion = 'Unavailable';
    }
  }

  async function minimizeWindow(event?: MouseEvent) {
    event?.preventDefault();
    event?.stopPropagation();
    await invoke('minimize_main_window');
  }

  async function closeWindow(event?: MouseEvent) {
    event?.preventDefault();
    event?.stopPropagation();
    await invoke('close_main_window');
  }

  function openDetails() {
    view = 'details';
    branchOpen = false;
    versionOpen = false;
    configOpen = false;
    launchPending = false;
  }

  function closeDetails() {
    if (launchTimer) {
      window.clearTimeout(launchTimer);
      launchTimer = undefined;
    }

    branchOpen = false;
    versionOpen = false;
    configOpen = false;
    launchPending = false;

    if (view === 'details') {
      view = 'closingDetails';
      closeTimer = window.setTimeout(() => {
        view = 'launcher';
        closeTimer = undefined;
      }, 240);
      return;
    }

    view = 'launcher';
  }

  function toggleBranch(event: MouseEvent) {
    event.stopPropagation();
    versionOpen = false;
    configOpen = false;
    branchOpen = !branchOpen;
  }

  function selectBranch(next: Branch) {
    branch = next;
    selectedVersion = (next === 'Release' ? gitMetadata.releases : gitMetadata.nightlies)[0]?.tag ?? '';
    branchOpen = false;
  }

  function toggleVersion(event: MouseEvent) {
    event.stopPropagation();
    configOpen = false;
    branchOpen = false;
    versionOpen = !versionOpen;
  }

  function selectVersion(version: string) {
    selectedVersion = version;
    versionOpen = false;
  }

  function toggleConfig(event: MouseEvent) {
    event.stopPropagation();
    branchOpen = false;
    versionOpen = false;
    configOpen = !configOpen;
  }

  function selectConfig(config: ConfigEntry) {
    selectedConfigId = config.entry_id;
    configOpen = false;
  }

  function launch() {
    branchOpen = false;
    launchPending = true;
    progress = 0;
    const versionToLaunch = selectedVersion;
    const configIdToLaunch = selectedConfigId;

    if (closeTimer) {
      window.clearTimeout(closeTimer);
      closeTimer = undefined;
    }

    launchTimer = window.setTimeout(() => {
      if (!launchPending) {
        return;
      }

      view = 'launching';
      launchTimer = undefined;
      void startLaunchProgress(versionToLaunch, configIdToLaunch);
    }, 230);
  }

  async function startLaunchProgress(versionToLaunch: string, configIdToLaunch: number | null) {
    const startedAt = performance.now();
    let finished = false;
    const tick = () => {
      const elapsed = performance.now() - startedAt;
      progress = finished ? 100 : Math.min(92, 18 + (elapsed / 3200) * 74);

      if (!finished) {
        requestAnimationFrame(tick);
      }
    };

    requestAnimationFrame(tick);

    try {
      await invoke('download_and_launch_version', { tag: versionToLaunch, configId: configIdToLaunch });
      finished = true;
      progress = 100;
      window.setTimeout(() => {
        void appWindow.close().catch(() => undefined);
      }, 1800);
    } catch (error) {
      console.warn('Failed to launch selected version', error);
      launchPending = false;
      view = 'details';
      progress = 0;
    }
  }

  function closeBranchFromBackdrop() {
    closeMenus();
  }

  function closeMenus() {
    branchOpen = false;
    versionOpen = false;
    configOpen = false;
  }

  function formatGitDate(value: string | undefined) {
    if (!value) {
      return 'Unknown';
    }

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return value;
    }

    const day = date.getDate().toString().padStart(2, '0');
    const month = (date.getMonth() + 1).toString().padStart(2, '0');
    const year = date.getFullYear();
    const hour = date.getHours().toString().padStart(2, '0');
    const minute = date.getMinutes().toString().padStart(2, '0');

    return `${day}.${month}.${year} ${hour}:${minute}`;
  }

  function parseChangelog(value: string | undefined) {
    const trimmed = value?.trim();

    if (!trimmed) {
      return ['No changelog provided.'];
    }

    return trimmed
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => line.replace(/^[-*]\s*/, '- '));
  }

  function renderMarkdownLine(value: string) {
    return escapeHtml(value).replace(
      /\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g,
      '<a href="$2" target="_blank" rel="noreferrer">$1</a>'
    );
  }

  function escapeHtml(value: string) {
    return value
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;');
  }

  function dragWindow(event: MouseEvent) {
    if (event.buttons !== 1) {
      return;
    }

    const target = event.target as HTMLElement;
    const scrollArea = target.closest('.changelog');

    if (scrollArea) {
      const bounds = scrollArea.getBoundingClientRect();
      const scrollbarWidth = 10;

      if (event.clientX >= bounds.right - scrollbarWidth) {
        return;
      }
    }

    if (!target.closest('.shell')) {
      return;
    }

    if (isInteractiveElement(target)) {
      return;
    }

    event.preventDefault();
    void appWindow.startDragging().catch(() => undefined);
  }

  function isInteractiveElement(element: HTMLElement) {
    const interactive = element.closest(
      'button, a, input, textarea, select, option, [role="button"], [role="link"], [tabindex], [contenteditable="true"], [data-no-drag]'
    );

    if (interactive) {
      return true;
    }

    return getComputedStyle(element).cursor === 'pointer';
  }
</script>

{#snippet IconChevron()}
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <path d="M7 10l5 5 5-5" />
  </svg>
{/snippet}

{#snippet IconPlay()}
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <path d="M8 5v14l11-7z" />
  </svg>
{/snippet}

{#snippet IconBranch()}
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <circle cx="7" cy="6" r="2" />
    <circle cx="17" cy="6" r="2" />
    <circle cx="7" cy="18" r="2" />
    <path d="M7 8v8M9 6h4a4 4 0 0 1 4 4v0" />
  </svg>
{/snippet}

{#snippet IconClose()}
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <path d="M7 7l10 10M17 7L7 17" />
  </svg>
{/snippet}

{#snippet IconCog()}
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <path d="M12 8.25a3.75 3.75 0 1 0 0 7.5 3.75 3.75 0 0 0 0-7.5Z" />
    <path d="M19.43 12.98c.04-.32.07-.65.07-.98s-.02-.66-.07-.98l2.03-1.58-1.92-3.32-2.39.96a7.48 7.48 0 0 0-1.7-.98L15.1 3.5h-3.84l-.36 2.6c-.6.24-1.17.56-1.7.98l-2.39-.96-1.92 3.32 2.03 1.58c-.04.32-.07.65-.07.98s.02.66.07.98l-2.03 1.58 1.92 3.32 2.39-.96c.52.41 1.09.74 1.7.98l.36 2.6h3.84l.36-2.6c.6-.24 1.17-.56 1.7-.98l2.39.96 1.92-3.32-2.03-1.58Z" />
  </svg>
{/snippet}

<svelte:head>
  <title>Neverlose Launcher</title>
</svelte:head>

{#if branchOpen || versionOpen || configOpen}
  <button class="click-away" aria-label="Close menu" onclick={closeBranchFromBackdrop}></button>
{/if}

<main
  class:boot={view === 'boot'}
  class:expanded={view !== 'boot'}
  class="shell"
  data-tauri-drag-region
  style={themeVariables}
>
  {#if view === 'boot'}
    <section class="boot-view" data-tauri-drag-region aria-label="Loading">
      <div class="boot-spinner">
        <svg viewBox="0 0 48 48" aria-hidden="true">
          <circle cx="24" cy="24" r="17"></circle>
        </svg>
      </div>
    </section>
  {:else}
    <section class="launcher-view" data-tauri-drag-region aria-label="Launcher">
      <div class="logo" data-tauri-drag-region>BS</div>

      <nav class="side-nav" aria-label="Navigation">
        <button>Website</button>
        <button>Support</button>
        <button>Market</button>
      </nav>

      <div class="profile">
        <div class="avatar"></div>
        <span>Placeholder</span>
      </div>

      <div class="window-controls" data-no-drag>
        <button aria-label="Minimize" class="minimize" data-no-drag onclick={minimizeWindow}><span></span></button>
        <button aria-label="Close" class="close" data-no-drag onclick={closeWindow}>{@render IconClose()}</button>
      </div>

      <section class="subscriptions" data-tauri-drag-region aria-label="Subscriptions">
        <h1 data-tauri-drag-region>Subscription</h1>
        <p data-tauri-drag-region>Available subscriptions</p>

        <button class="subscription-card active" onclick={openDetails}>
          <span>
            <strong>Counter-Strike: Global Offensive</strong>
            <em>Expires Never</em>
          </span>
          <img class="game-icon" src="/csgo.png" alt="" draggable="false" />
        </button>

      </section>

      {#if view === 'details' || view === 'closingDetails' || view === 'launching'}
        <div class:visible={view !== 'closingDetails'} class="background-dim"></div>

        <section
          class:closing={view === 'closingDetails'}
          class:fading={launchPending || view === 'closingDetails' || view === 'launching'}
          class:launching={view === 'launching'}
          class="details"
          data-tauri-drag-region
          aria-label="Subscription details"
        >
          <div class="detail-content">
            <header>
              <img class="game-icon large" src="/csgo.png" alt="" draggable="false" />
              <h2>Counter-Strike: Global Offensive</h2>
              <button aria-label="Close details" class="detail-close" onclick={closeDetails}>{@render IconClose()}</button>
            </header>

            <div class="detail-body">
              <div class="metadata">
                <div class="metadata-row with-menu">
                  <span class="label">Branch:</span>
                  <button
                    class:active={versionOpen}
                    class="metadata-trigger version-trigger"
                    onclick={toggleVersion}
                  >
                    <span class="trigger-icon branch-icon-small"></span>
                    <span>{selectedBranchLabel}</span>
                  </button>
                  {#if versionOpen}
                    <div class="metadata-menu version-menu">
                      {#each versions as version}
                        <button class:selected={version.tag === selectedVersion} onclick={() => selectVersion(version.tag)}>
                          {version.tag}
                        </button>
                      {:else}
                        <button disabled>No builds</button>
                      {/each}
                    </div>
                  {/if}
                </div>
                <p><span class="label">Updated:</span> <span class="value">{updatedAtLabel}</span></p>
                <div class="metadata-row with-menu">
                  <span class="label">Config:</span>
                  <button
                    class:active={configOpen}
                    class="metadata-trigger config-trigger"
                    onclick={toggleConfig}
                  >
                    <span class="trigger-icon cog-icon-small">{@render IconCog()}</span>
                    <span>{selectedConfigName}</span>
                  </button>
                  {#if configOpen}
                    <div class="metadata-menu config-menu">
                      {#each configs as config}
                        <button class:selected={config.entry_id === selectedConfigId} onclick={() => selectConfig(config)}>
                          {config.name}
                        </button>
                      {/each}
                    </div>
                  {/if}
                </div>
                <p><span class="label">Last Launch:</span> <span class="value">Just Now</span></p>
              </div>

              <div class="changelog">
                <p class="date">
                  Changelogs
                  {#if selectedReleaseUrl}
                    <a href={selectedReleaseUrl} target="_blank" rel="noreferrer">{selectedVersionLabel}</a>
                  {:else}
                    <span>{selectedVersionLabel}</span>
                  {/if}
                </p>
                <p>
                  {#each changelogEntries as entry}
                    {@html renderMarkdownLine(entry)}<br />
                  {/each}
                </p>
              </div>
            </div>

            <footer>
              <div class="footer-links">
                <button>Community</button>
                <button>API Documentation</button>
              </div>

              <div class="actions">
                <button
                  disabled={launchPending}
                  class:active={branchOpen}
                  class="branch-trigger"
                  aria-label="Select branch"
                  onclick={toggleBranch}
                >
                  {@render IconChevron()}
                </button>
                <button disabled={branchOpen || launchPending} class="load" onclick={launch}>
                  <span class="play-icon">{@render IconPlay()}</span>
                  Load
                </button>

                {#if branchOpen}
                  <div class="branch-menu">
                    <button class:selected={branch === 'Release'} onclick={() => selectBranch('Release')}>
                      <span class="branch-icon"></span>
                      Release
                    </button>
                    <button class:selected={branch === 'Nightly'} onclick={() => selectBranch('Nightly')}>
                      <span class="branch-icon"></span>
                      Nightly
                    </button>
                  </div>
                {/if}
              </div>
            </footer>
          </div>

          <div class="launch-content" aria-hidden={view !== 'launching'}>
            <img class="game-icon launch" src="/csgo.png" alt="" draggable="false" />
            <p>The game will be launched automatically</p>
            <div class="progress"><span style={`width: ${progress}%`}></span></div>
          </div>
        </section>
      {/if}

    </section>
  {/if}
</main>

<style>
  @font-face {
    font-family: "Museo Sans Local";
    font-style: normal;
    font-weight: 100;
    font-display: block;
    src: url("/fonts/MuseoSans-100.woff") format("woff");
  }

  @font-face {
    font-family: "Museo Sans Local";
    font-style: normal;
    font-weight: 300;
    font-display: block;
    src: url("/fonts/MuseoSans-300.woff") format("woff");
  }

  @font-face {
    font-family: "Museo Sans Local";
    font-style: normal;
    font-weight: 500;
    font-display: block;
    src: url("/fonts/MuseoSans_500.woff") format("woff");
  }

  @font-face {
    font-family: "Museo Sans Local";
    font-style: normal;
    font-weight: 700;
    font-display: block;
    src: url("/fonts/MuseoSans_700.woff") format("woff");
  }

  @font-face {
    font-family: "Museo Sans Local";
    font-style: normal;
    font-weight: 900;
    font-display: block;
    src: url("/fonts/MuseoSans_900.woff") format("woff");
  }

  @font-face {
    font-family: "Inter Variable";
    font-style: normal;
    font-weight: 100 900;
    font-display: block;
    src: url("/fonts/InterVariable.woff2") format("woff2-variations");
  }

  :global(html),
  :global(body) {
    margin: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: transparent;
    font-family: "Museo Sans Local";
    font-feature-settings: "liga" 1, "calt" 1;
    color: white;
    user-select: none;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  :global(button) {
    font: inherit;
  }

  :global(:root) {
    --ease-smooth: cubic-bezier(.22, 1, .36, 1);
    --ease-soft: cubic-bezier(.25, .1, .25, 1);
    --nl-text: rgba(255, 255, 255, 0.88);
    --nl-disabled-text: rgba(255, 255, 255, 0.38);
    --nl-active-text: #FFFFFF;
    --nl-small-text: rgba(255, 255, 255, 0.575);
    --nl-sidebar-text: rgba(255, 255, 255, 0.589);
    --nl-logo: white;
    --nl-main-bg: #010306;
    --nl-main-bg-opaque: #010306;
    --nl-popup-bg: rgba(7, 12, 19, 0.65);
    --nl-preview-bg: #03080f;
    --nl-border: rgba(255, 255, 255, 0.075);
    --nl-frame-bg: #03080f;
    --nl-frame-active-bg: rgba(7, 12, 19, 0.72);
    --nl-text-preview: rgba(247, 245, 255, 0.8);
    --nl-window-title-bg: rgba(4, 8, 13, 0.96);
    --nl-active-window-title: rgba(255, 255, 255, 0.88);
    --nl-spinner: #aab6ff;
    --nl-block-bg: #03080f;
    --nl-block-bg-opaque: #03080f;
    --nl-sidebar-selection: rgba(255, 255, 255, 0.08);
    --nl-button: #3f66f5;
    --nl-button-active: #486cee;
    --nl-button-active-text: rgba(255, 255, 255, 0.9);
    --nl-link: #626be6;
    --nl-link-active: white;
    --nl-selection: rgba(16, 31, 49, 0.78);
    --nl-separator: rgba(255, 255, 255, 0.045);
    --nl-shadow: rgba(0, 0, 0, 0.52);
    --nl-shadow-soft: rgba(0, 0, 0, 0.48);
  }

  :global(svg) {
    display: block;
    width: 1em;
    height: 1em;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .click-away {
    position: fixed;
    inset: 0;
    z-index: 5;
    padding: 0;
    border: 0;
    background: transparent;
  }

  .shell {
    position: fixed;
    left: 50%;
    top: 50%;
    z-index: 10;
    width: 260px;
    height: 260px;
    overflow: hidden;
    border-radius: 9px;
    background: var(--nl-main-bg-opaque);
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--nl-shadow), transparent 78%),
      0 0 16px color-mix(in srgb, var(--nl-shadow), transparent 72%),
      0 10px 24px rgba(0, 0, 0, 0.24);
    transform: translate(-50%, -50%);
    transition:
      width 430ms var(--ease-smooth),
      height 430ms var(--ease-smooth),
      opacity 180ms ease-in-out;
  }

  .shell.expanded {
    width: 530px;
    height: 400px;
  }

  .boot-view {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }

  .boot-view > * {
    grid-area: 1 / 1;
  }

  .boot-spinner {
    position: relative;
    width: 34px;
    height: 34px;
    border-radius: 999px;
    animation: spin 1400ms linear infinite;
  }

  .boot-spinner svg {
    width: 100%;
    height: 100%;
    color: var(--nl-spinner);
    filter: drop-shadow(0 0 7px rgba(150, 164, 255, 0.22));
  }

  .boot-spinner circle {
    fill: none;
    stroke: currentColor;
    stroke-width: 4;
    stroke-linecap: round;
    stroke-dasharray: 1 107;
    stroke-dashoffset: 0;
    transform-origin: 50% 50%;
    animation: spinner-arc 1400ms ease-in-out infinite;
  }

  .launcher-view {
    position: absolute;
    inset: 0;
    opacity: 0;
    animation: fade-in 280ms 170ms ease-in-out both;
  }

  .logo {
    position: absolute;
    left: 22px;
    top: 22px;
    font-family: "Museo Sans Local", "Museo Sans Cyrl", "Museo Sans Cyrillic", "Museo Sans", "Inter Variable", Inter, sans-serif;
    font-size: 38px;
    font-weight: 900;
    line-height: 1;
    color: var(--nl-logo);
    text-shadow: none;
  }

  .side-nav {
    position: absolute;
    left: 18px;
    top: 105px;
    display: grid;
    gap: 16px;
  }

  .side-nav button,
  .footer-links button {
    padding: 0;
    border: 0;
    background: transparent;
    font-size: 13px;
    font-weight: 300;
    text-align: left;
    transition: color 130ms ease-in-out;
  }

  .side-nav button {
    color: var(--nl-text);
  }

  .footer-links button {
    color: var(--nl-text);
  }

  .side-nav button:hover,
  .footer-links button:hover {
    color: color-mix(in srgb, var(--nl-text), var(--nl-active-text) 50%);
  }

  .profile {
    position: absolute;
    left: 20px;
    bottom: 20px;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 100;
    line-height: 28px;
    color: var(--nl-active-text);
  }

  .avatar {
    width: 40px;
    height: 40px;
    border-radius: 999px;
    background:
      radial-gradient(circle at 50% 36%, rgba(255, 255, 255, 0.82) 0 10%, transparent 11%),
      radial-gradient(circle at 38% 50%, #efe1d6 0 13%, transparent 14%),
      radial-gradient(circle at 58% 52%, #f3d7c9 0 12%, transparent 13%),
      radial-gradient(circle at 50% 70%, #1d1d25 0 25%, transparent 26%),
      linear-gradient(135deg, #c9cbd2, #4b4d57);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
    opacity: 0.9;
  }

  .window-controls {
    position: absolute;
    right: 17px;
    top: 15px;
    display: flex;
    gap: 8px;
    align-items: center;
    z-index: 15;
  }

  .window-controls button,
  .detail-close {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    color: var(--nl-text);
    background: transparent;
    transition: color 120ms ease-in-out, opacity 120ms ease-in-out;
  }

  .minimize::before {
    content: none;
  }

  .minimize span {
    display: block;
    width: 12px;
    height: 1.5px;
    border-radius: 999px;
    background: currentColor;
  }

  .minimize:hover {
    color: var(--nl-active-text);
  }

  .close svg,
  .detail-close svg {
    width: 19px;
    height: 19px;
    stroke-width: 2.45;
    shape-rendering: geometricPrecision;
    pointer-events: none;
  }

  .close:hover,
  .detail-close:hover {
    color: var(--nl-active-text);
  }

  .subscriptions {
    position: absolute;
    left: 176px;
    top: 16px;
    width: 323px;
  }

  .subscriptions h1 {
    margin: 0;
    font-size: 19px;
    font-weight: 500;
    line-height: 1.7;
    transform: translateX(7px);
    color: var(--nl-active-text);
  }

  .subscriptions > p {
    margin: 6px 0 45px;
    color: var(--nl-text);
    font-size: 14.5px;
    font-weight: 300;
    transform: translateX(7px);
  }

  .subscription-card {
    position: relative;
    display: block;
    width: 100%;
    height: 73px;
    margin: 0 0 11px;
    padding: 0;
    overflow: hidden;
    border: 0.5px solid var(--nl-border);
    border-radius: 12px;
    background: var(--nl-block-bg);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.018);
    text-align: left;
  }

  .subscription-card.active {
    cursor: default;
    animation: card-in 360ms 260ms ease-in-out both;
    transition: none;
  }

  .subscription-card span {
    position: absolute;
    left: 12px;
    top: 8px;
  }

  .subscription-card strong {
    display: block;
    color: var(--nl-active-text);
    font-size: 13.5px;
    font-weight: 500;
  }

  .subscription-card em {
    display: block;
    margin-top: 9px;
    color: var(--nl-text-preview);
    font-size: 13px;
    font-weight: 100;
    font-style: normal;
  }

  .game-icon {
    position: absolute;
    right: 10px;
    top: 11px;
    width: 22px;
    height: 22px;
    border-radius: 5px;
    object-fit: cover;
    box-shadow: 0 0 18px rgba(240, 139, 11, 0.2);
    pointer-events: none;
  }

  .background-dim {
    position: absolute;
    inset: 0;
    z-index: 8;
    background: rgba(0, 0, 0, 0);
    cursor: default;
    transition: background 360ms var(--ease-soft);
  }

  .background-dim.visible {
    background: rgba(0, 0, 0, 0.66);
  }

  .details {
    position: absolute;
    left: 50%;
    top: 50%;
    z-index: 10;
    width: 460px;
    height: 340px;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.075);
    border-radius: 10px;
    background: var(--nl-block-bg-opaque);
    transform: translate(-50%, -50%);
    animation: panel-in 320ms var(--ease-smooth) both;
    transition:
      width 520ms var(--ease-smooth),
      height 520ms var(--ease-smooth),
      opacity 260ms var(--ease-soft),
      transform 320ms var(--ease-smooth),
      border-radius 420ms var(--ease-smooth),
      background 360ms var(--ease-soft);
  }

  .details.launching {
    width: 375px;
    height: 250px;
    border-radius: 4px;
    background: var(--nl-block-bg-opaque);
    animation: none;
    pointer-events: none;
  }

  .details.closing {
    animation: none;
    opacity: 0;
    transform: translate(-50%, -50%) scale(0.985);
    pointer-events: none;
    transition:
      opacity 210ms var(--ease-soft),
      transform 240ms var(--ease-smooth);
  }

  .detail-content {
    position: absolute;
    inset: 0;
    z-index: 1;
    opacity: 1;
    transition: opacity 280ms var(--ease-soft), transform 320ms var(--ease-smooth);
    transform: translateY(0) scale(1);
  }

  .details.fading .detail-content {
    opacity: 0;
    transform: translateY(2px) scale(0.995);
    pointer-events: none;
  }

  .details header {
    position: relative;
    height: 70px;
    border-bottom: 1px solid var(--nl-separator);
  }

  .details .large {
    left: 17px;
    top: 19px;
    width: 30px;
    height: 30px;
    border-radius: 8px;
  }

  .details h2 {
    position: absolute;
    left: 58px;
    top: 50%;
    margin: 0;
    font-size: 18px;
    font-weight: 500;
    color: var(--nl-active-text);
    transform: translateY(-50%);
  }

  .detail-close {
    position: absolute;
    right: 15px;
    top: 25px;
    font-size: 18px;
  }

  .detail-body {
    display: grid;
    grid-template-columns: 197px 1fr;
    gap: 0;
    padding: 16px 20px 0;
  }

  .metadata p,
  .metadata-row,
  .changelog p {
    margin: 0;
    font-size: 13px;
    line-height: 1.85;
    font-weight: 300;
  }

  .metadata p,
  .metadata-row {
    margin-bottom: 3px;
  }

  .metadata-row {
    position: relative;
  }

  .metadata-row.with-menu {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-wrap: nowrap;
  }

  .metadata .label {
    color: var(--nl-active-text);
    font-weight: 300;
    flex: 0 0 auto;
  }

  .metadata .value {
    color: var(--nl-link-active);
    font-weight: 500;
    margin-left: 4px;
  }

  .metadata-trigger {
    display: inline-grid;
    grid-template-columns: 15px minmax(0, auto);
    align-items: center;
    gap: 6px;
    max-width: 150px;
    height: 22px;
    padding: 0 7px;
    margin: 0;
    border: 1px solid color-mix(in srgb, var(--nl-border), var(--nl-text) 14%);
    border-radius: 5px;
    color: var(--nl-link-active);
    background: transparent;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--nl-border), transparent 45%);
    font: inherit;
    font-weight: 500;
    text-align: left;
    transition: background 130ms ease-in-out, border-color 130ms ease-in-out, box-shadow 130ms ease-in-out, color 130ms ease-in-out;
    vertical-align: middle;
  }

  .metadata-trigger:focus {
    outline: none;
  }

  .metadata-trigger:focus-visible {
    outline: 1px solid color-mix(in srgb, var(--nl-button-active), transparent 45%);
    outline-offset: 1px;
  }

  .metadata-trigger span:last-child {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .metadata-trigger:hover {
    border-color: transparent;
    box-shadow: inset 0 0 0 1px transparent;
    background: color-mix(in srgb, var(--nl-button-active), transparent 50%);
    color: var(--nl-active-text);
  }

  .metadata-trigger.active {
    border-color: transparent;
    box-shadow: inset 0 0 0 1px transparent;
    background: var(--nl-button);
    color: var(--nl-button-active-text);
  }

  .config-trigger {
    max-width: 128px;
  }

  .version-trigger {
    max-width: 132px;
  }

  .trigger-icon {
    display: block;
    width: 15px;
    height: 15px;
    justify-self: center;
    color: currentColor;
  }

  .branch-icon-small {
    background: currentColor;
    mask: url("/git-branch.svg") center / contain no-repeat;
    -webkit-mask: url("/git-branch.svg") center / contain no-repeat;
  }

  .cog-icon-small {
    display: inline-flex;
  }

  .cog-icon-small svg {
    width: 15px;
    height: 15px;
    stroke-width: 1.65;
  }

  .metadata-menu {
    position: absolute;
    left: 68px;
    top: 28px;
    z-index: 35;
    width: max-content;
    min-width: 132px;
    max-width: 196px;
    max-height: 116px;
    overflow: auto;
    padding: 6px 0;
    border: 1px solid color-mix(in srgb, var(--nl-button-active), transparent 84%);
    border-radius: 10px;
    background: color-mix(in srgb, var(--nl-popup-bg), transparent 22%);
    box-shadow:
      0 0 14px color-mix(in srgb, var(--nl-button-active), transparent 94%),
      inset 0 0 0 1px color-mix(in srgb, var(--nl-button-active), transparent 95%),
      0 12px 30px var(--nl-shadow-soft);
    backdrop-filter: blur(9px);
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--nl-active-text), transparent 78%) transparent;
    transform-origin: left top;
    animation: menu-in 145ms var(--ease-smooth) both;
  }

  .config-menu {
    min-width: 132px;
    max-width: 164px;
  }

  .metadata-menu button {
    display: block;
    width: 100%;
    height: 26px;
    padding: 0 11px;
    border: 0;
    color: var(--nl-disabled-text);
    background: transparent;
    font-size: 13px;
    font-weight: 300;
    text-align: left;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    transition: color 130ms ease-in-out, text-shadow 130ms ease-in-out;
  }

  .metadata-menu button:hover {
    color: color-mix(in srgb, var(--nl-active-text), var(--nl-disabled-text) 50%);
    text-shadow: 0 0 10px color-mix(in srgb, var(--nl-active-text), transparent 86%);
  }

  .metadata-menu .selected,
  .metadata-menu .selected:hover {
    color: var(--nl-active-text);
  }

  .metadata-menu button:disabled {
    color: var(--nl-disabled-text);
    opacity: 0.58;
    pointer-events: none;
  }

  .changelog {
    max-height: 154px;
    overflow-x: hidden;
    overflow-y: auto;
    padding-right: 8px;
    color: var(--nl-text);
    overflow-wrap: anywhere;
    mask-image: linear-gradient(to bottom, #000 0%, #000 calc(100% - 22px), transparent 100%);
    -webkit-mask-image: linear-gradient(to bottom, #000 0%, #000 calc(100% - 22px), transparent 100%);
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--nl-active-text), transparent 75%) transparent;
  }

  .changelog :global(a),
  .changelog :global(a:visited),
  .changelog :global(a:active),
  .changelog :global(a:focus) {
    color: var(--nl-link);
    cursor: default !important;
    text-decoration: none;
    outline: none;
    transition: color 130ms ease-in-out, text-shadow 130ms ease-in-out;
  }

  .changelog :global(a:hover),
  .changelog :global(a:visited:hover),
  .changelog :global(a:active:hover),
  .changelog :global(a:focus:hover) {
    color: var(--nl-link-active);
    cursor: default !important;
    text-shadow: 0 0 10px color-mix(in srgb, var(--nl-link-active), transparent 88%);
  }

  .changelog::-webkit-scrollbar {
    width: 4px;
  }

  .changelog::-webkit-scrollbar-track {
    background: transparent;
  }

  .changelog::-webkit-scrollbar-thumb {
    border-radius: 999px;
    background: color-mix(in srgb, var(--nl-active-text), transparent 78%);
  }

  .changelog .date {
    color: var(--nl-active-text);
    margin-bottom: 15px;
    font-weight: 300;
  }

  .changelog .date a,
  .changelog .date span {
    margin-left: 4px;
  }

  .changelog p:not(.date) {
    color: var(--nl-text);
    margin-bottom: 20px;
  }

  .details footer {
    position: absolute;
    left: 17px;
    right: 17px;
    bottom: 15px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .footer-links {
    display: flex;
    gap: 15px;
  }

  .actions {
    position: relative;
    display: grid;
    grid-template-columns: 32px 102px;
    gap: 8px;
    align-items: center;
  }

  .branch-trigger,
  .load {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 28px;
    border-radius: 6px;
    color: var(--nl-button-active-text);
    font-size: 13px;
    font-weight: 300;
  }

  .branch-trigger {
    width: 32px;
    border: 1px solid color-mix(in srgb, var(--nl-border), var(--nl-text) 14%);
    background: transparent;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--nl-border), transparent 35%);
    transition: background 130ms ease-in-out, border-color 130ms ease-in-out, box-shadow 130ms ease-in-out;
  }

  .branch-trigger svg {
    width: 19px;
    height: 19px;
    stroke-width: 1.65;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .branch-trigger:hover {
    border-color: transparent;
    box-shadow: inset 0 0 0 1px transparent;
    background: color-mix(in srgb, var(--nl-button-active), transparent 50%);
  }

  .branch-trigger.active {
    border-color: transparent;
    box-shadow: inset 0 0 0 1px transparent;
    background: var(--nl-button);
  }

  .load {
    width: 102px;
    border: 0;
    background: var(--nl-button);
    transition: background 130ms ease-in-out, opacity 130ms ease-in-out;
  }

  .load .play-icon {
    display: inline-flex;
    margin-right: 12px;
    margin-left: -4px;
    color: var(--nl-button-active-text);
  }

  .load svg {
    width: 18px;
    height: 20px;
    fill: none;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .load:hover {
    background: var(--nl-button-active);
  }

  .load:disabled {
    opacity: 0.55;
    pointer-events: none;
  }

  .branch-menu {
    position: absolute;
    left: 34px;
    top: -34px;
    z-index: 30;
    width: 122px;
    height: 66px;
    padding: 8px 0 0;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--nl-button-active), transparent 84%);
    border-radius: 6px;
    background: color-mix(in srgb, var(--nl-popup-bg), transparent 28%);
    box-shadow:
      0 0 14px color-mix(in srgb, var(--nl-button-active), transparent 94%),
      inset 0 0 0 1px color-mix(in srgb, var(--nl-button-active), transparent 95%),
      0 12px 30px var(--nl-shadow-soft);
    backdrop-filter: blur(9px);
    transform-origin: left bottom;
    animation: menu-in 145ms var(--ease-smooth) both;
  }

  .branch-menu button {
    display: grid;
    grid-template-columns: 24px 1fr;
    align-items: center;
    column-gap: 11px;
    width: 100%;
    height: 29px;
    margin: 0;
    padding: 0 0 0 14px;
    border: 0;
    border-radius: 0;
    color: var(--nl-disabled-text);
    background: transparent;
    font-size: 14px;
    font-weight: 100;
    text-align: left;
    transition: color 130ms ease-in-out, text-shadow 130ms ease-in-out;
  }

  .branch-icon {
    display: block;
    width: 17px;
    height: 17px;
    justify-self: center;
    background: currentColor;
    mask: url("/git-branch.svg") center / contain no-repeat;
    -webkit-mask: url("/git-branch.svg") center / contain no-repeat;
    opacity: 0.9;
  }

  .branch-menu button:hover {
    color: color-mix(in srgb, var(--nl-active-text), var(--nl-disabled-text) 50%);
    background: transparent;
    text-shadow: 0 0 10px color-mix(in srgb, var(--nl-active-text), transparent 86%);
  }

  .branch-menu .selected {
    color: var(--nl-active-text);
  }

  .branch-menu .selected:hover {
    color: var(--nl-active-text);
  }

  .launch-content {
    position: absolute;
    inset: 0;
    --launch-content-width: 249px;
    z-index: 2;
    display: grid;
    place-items: center;
    align-content: center;
    opacity: 0;
    pointer-events: none;
    transform: scale(0.985);
    transition:
      opacity 260ms 260ms var(--ease-soft),
      transform 420ms 220ms var(--ease-smooth);
  }

  .details.launching .launch-content {
    opacity: 1;
    transform: scale(1);
  }

  .game-icon.launch {
    position: static;
    width: 46px;
    height: 46px;
    border-radius: 7px;
    animation: launch-icon-pulse 2s var(--ease-smooth) infinite;
    transform-origin: center;
  }

  .launch-content p {
    width: var(--launch-content-width);
    margin: 30px 0 16px;
    color: var(--nl-text);
    font-size: 13px;
    white-space: nowrap;
    justify-self: center;
    text-align: center;
  }

  .progress {
    width: var(--launch-content-width);
    height: 4px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--nl-frame-bg);
  }

  .progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--nl-button);
  }

  @keyframes fill {
    from { transform: scaleX(0); }
    to { transform: scaleX(1); }
  }

  @keyframes spin {
    to { transform: rotate(1turn); }
  }

  @keyframes spinner-arc {
    0% {
      stroke-dasharray: 1 107;
      stroke-dashoffset: 0;
    }
    50% {
      stroke-dasharray: 78 107;
      stroke-dashoffset: -18;
    }
    100% {
      stroke-dasharray: 1 107;
      stroke-dashoffset: -106;
    }
  }

  @keyframes launch-icon-pulse {
    0%,
    100% {
      opacity: 0.68;
      filter: brightness(0.72) saturate(0.9) drop-shadow(0 0 4px rgba(240, 139, 11, 0.08));
      transform: scale(0.88);
    }
    50% {
      opacity: 1;
      filter: brightness(1.12) saturate(1.08) drop-shadow(0 0 18px rgba(240, 139, 11, 0.34));
      transform: scale(1.08);
    }
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(3px) scale(0.985);
    }

    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @keyframes card-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes panel-in {
    from {
      opacity: 0;
      transform: translate(-50%, -49%) scale(0.985);
    }
    to {
      opacity: 1;
      transform: translate(-50%, -50%) scale(1);
    }
  }
</style>
