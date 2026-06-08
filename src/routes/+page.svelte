<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  type Branch = 'Release' | 'Nightly';
  type Game = 'csgo_legacy' | 'csgo';
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

  const WEBSITE_URL = 'https://gs.femboy.tw';
  const DISCORD_URL = 'https://discord.gg/YzDpcsEhCE';
  const API_DOCS_URL = 'https://docs-csgo.neverlose.cc/';

  let view = $state<View>('boot');
  let game = $state<Game>('csgo');
  let branch = $state<Branch>('Release');
  let branchOpen = $state(false);
  let versionOpen = $state(false);
  let configOpen = $state(false);
  let profileOpen = $state(false);
  let profileSaving = $state(false);
  let profileError = $state('');
  let username = $state('Player');
  let profileNameInput = $state('');
  let avatarDataUrl = $state('');
  let avatarDataUrlBeforeEdit = $state('');
  let pendingAvatarBytes = $state<number[] | null>(null);
  let avatarInput = $state<HTMLInputElement | null>(null);
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
  const appWindow = getOptionalCurrentWindow();

  type LauncherTheme = {
    source: string;
    variables: Record<string, string>;
  };

  type LauncherSettings = {
    username: string;
    avatar_data_url: string | null;
    selected_config_id: number | null;
    configs: ConfigEntry[];
  };

  type LauncherGitMetadata = {
    releases: LauncherVersion[];
    nightlies: LauncherVersion[];
  };

  function getOptionalCurrentWindow() {
    try {
      return getCurrentWindow();
    } catch {
      return null;
    }
  }

  function hasTauriRuntime() {
    return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  }

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

      if (
        target.closest(
          '.branch-trigger, .branch-menu, .metadata-trigger, .metadata-menu, .changelog a, .profile-trigger, .profile-popout'
        )
      ) {
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

  const appids: Record<Game, number> = {
      csgo: 4465480,
      csgo_legacy: 730,
  };

  function showLauncher() {
    view = 'launcher';
  }

  async function loadTheme() {
    if (!hasTauriRuntime()) {
      return;
    }

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
    if (!hasTauriRuntime()) {
      profileNameInput = username;
      return;
    }

    try {
      const settings = await invoke<LauncherSettings>('load_launcher_settings');
      applyLauncherSettings(settings);
    } catch (error) {
      console.warn('Failed to load launcher settings', error);
    }
  }

  function applyLauncherSettings(settings: LauncherSettings) {
    username = settings.username || 'Player';
    profileNameInput = username;
    avatarDataUrl = settings.avatar_data_url ?? avatarDataUrl;
    avatarDataUrlBeforeEdit = avatarDataUrl;
    configs = settings.configs;
    selectedConfigId = settings.selected_config_id ?? settings.configs[0]?.entry_id ?? null;
  }

  async function loadGitMetadata() {
    if (!hasTauriRuntime()) {
      gitMetadata = {
        releases: [
          {
            tag: 'Unavailable',
            name: 'Unavailable',
            changelog: 'Git metadata is loaded from Tauri.',
            updated_at: '',
            url: '',
            assets: []
          }
        ],
        nightlies: []
      };
      selectedVersion = 'Unavailable';
      return;
    }

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
    if (!hasTauriRuntime()) {
      return;
    }
    await invoke('minimize_main_window');
  }

  async function closeWindow(event?: MouseEvent) {
    event?.preventDefault();
    event?.stopPropagation();
    if (!hasTauriRuntime()) {
      return;
    }
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

  async function openExternal(url: string) {
    if (!url) {
      return;
    }

    if (!hasTauriRuntime()) {
      window.open(url, '_blank', 'noreferrer');
      return;
    }

    try {
      await openUrl(url);
    } catch (error) {
      console.warn('Failed to open link', error);
    }
  }

  function openProfile(event: MouseEvent) {
    event.stopPropagation();
    profileNameInput = username;
    avatarDataUrlBeforeEdit = avatarDataUrl;
    pendingAvatarBytes = null;
    profileError = '';
    branchOpen = false;
    versionOpen = false;
    configOpen = false;
    profileOpen = !profileOpen;
  }

  function chooseAvatar() {
    avatarInput?.click();
  }

  async function handleAvatarChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) {
      return;
    }

    if (!file.type.startsWith('image/')) {
      profileError = 'Choose an image file.';
      input.value = '';
      return;
    }

    try {
      const rounded = await roundedAvatarPng(file);
      pendingAvatarBytes = Array.from(rounded.bytes);
      avatarDataUrl = rounded.dataUrl;
      profileError = '';
    } catch (error) {
      profileError = String(error);
    } finally {
      input.value = '';
    }
  }

  function cancelProfileEdit() {
    profileNameInput = username;
    avatarDataUrl = avatarDataUrlBeforeEdit;
    pendingAvatarBytes = null;
    profileError = '';
  }

  async function roundedAvatarPng(file: File) {
    const source = await fileToDataUrl(file);
    const image = await loadImage(source);
    const size = 256;
    const canvas = document.createElement('canvas');
    canvas.width = size;
    canvas.height = size;
    const context = canvas.getContext('2d');
    if (!context) {
      throw new Error('Could not prepare profile image.');
    }

    const edge = Math.min(image.naturalWidth, image.naturalHeight);
    const sx = (image.naturalWidth - edge) / 2;
    const sy = (image.naturalHeight - edge) / 2;
    context.clearRect(0, 0, size, size);
    context.save();
    context.beginPath();
    context.arc(size / 2, size / 2, size / 2, 0, Math.PI * 2);
    context.clip();
    context.drawImage(image, sx, sy, edge, edge, 0, 0, size, size);
    context.restore();

    const dataUrl = canvas.toDataURL('image/png');
    const response = await fetch(dataUrl);
    const bytes = new Uint8Array(await response.arrayBuffer());
    return { bytes, dataUrl };
  }

  function loadImage(src: string) {
    return new Promise<HTMLImageElement>((resolve, reject) => {
      const image = new Image();
      image.onload = () => resolve(image);
      image.onerror = () => reject(new Error('Could not load profile image.'));
      image.src = src;
    });
  }

  function fileToDataUrl(file: File) {
    return new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result ?? ''));
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
  }

  async function saveProfile() {
    const nextUsername = profileNameInput.trim();
    if (!nextUsername) {
      profileError = 'Enter a profile name.';
      return;
    }

    profileSaving = true;
    profileError = '';
    if (!hasTauriRuntime()) {
      username = nextUsername;
      profileNameInput = username;
      avatarDataUrlBeforeEdit = avatarDataUrl;
      pendingAvatarBytes = null;
      profileSaving = false;
      return;
    }

    try {
      const settings = await invoke<LauncherSettings>('save_launcher_profile', {
        username: nextUsername,
        avatarBytes: pendingAvatarBytes
      });
      applyLauncherSettings(settings);
      pendingAvatarBytes = null;
    } catch (error) {
      profileError = String(error);
    } finally {
      profileSaving = false;
    }
  }

  async function saveProfileName() {
    if (profileSaving || profileNameInput.trim() === username) {
      return;
    }
    await saveProfile();
  }

  function handleProfileNameKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void saveProfileName();
      return;
    }

    if (event.key === 'Escape') {
      event.preventDefault();
      profileNameInput = username;
      profileError = '';
    }
  }

  function launch(appid: number) {
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
      void startLaunchProgress(versionToLaunch, configIdToLaunch, appid);
    }, 230);
  }

  async function startLaunchProgress(versionToLaunch: string, configIdToLaunch: number | null, appid: number | null) {
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
      await invoke('download_and_launch_version', { tag: versionToLaunch, configId: configIdToLaunch, appid });
      // todo: wait until we see csgo.exe to determine if we're actually done
      finished = true;
      progress = 100;
      window.setTimeout(() => {
        void appWindow?.close().catch(() => undefined);
      }, 1800);
    } catch (error) {
      console.warn('Failed to launch selected version', error);
      launchPending = false;
      view = 'details';
      progress = 0;
      await invoke('kill_background_processes');
    }
  }

  function closeBranchFromBackdrop() {
    closeMenus();
  }

  function closeMenus() {
    branchOpen = false;
    versionOpen = false;
    configOpen = false;
    if (profileOpen && pendingAvatarBytes) {
      avatarDataUrl = avatarDataUrlBeforeEdit;
      pendingAvatarBytes = null;
    }
    profileOpen = false;
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
    void appWindow?.startDragging().catch(() => undefined);
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

{#snippet IconConstruction()}
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <rect x="2" y="6" width="20" height="8" rx="1" />
    <path d="M17 14v7" />
    <path d="M7 14v7" />
    <path d="M17 3v3" />
    <path d="M7 3v3" />
    <path d="M10 14 2.3 6.3" />
    <path d="M14 6l8 8" />
    <path d="M8 6l8 8" />
  </svg>
{/snippet}

<svelte:head>
  <title>Neverlose Launcher</title>
</svelte:head>

{#if branchOpen || versionOpen || configOpen || profileOpen}
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
        <button onclick={() => openExternal(WEBSITE_URL)}>Website</button>
        <button onclick={() => openExternal(DISCORD_URL)}>Support</button>
        <button>Market</button>
      </nav>

      <button class:active={profileOpen} class="profile profile-trigger" data-no-drag onclick={openProfile}>
        <span class="avatar">
          {#if avatarDataUrl}
            <img src={avatarDataUrl} alt="" draggable="false" />
          {/if}
        </span>
        <span>{username}</span>
      </button>

      {#if profileOpen}
        <button
          class="background-dim visible profile-dim"
          aria-label="Close profile settings"
          onclick={closeMenus}
          transition:fade={{ duration: 210 }}
        ></button>
        <section
          class="profile-popout"
          data-no-drag
          aria-label="Profile settings"
          transition:scale={{ duration: 220, start: 0.985, opacity: 0 }}
        >
          <button aria-label="Close profile settings" class="detail-close profile-close" onclick={closeMenus}>
            {@render IconClose()}
          </button>

          <div class="profile-content">
            <button class="avatar profile-popout-avatar" aria-label="Change profile image" onclick={chooseAvatar}>
              {#if avatarDataUrl}
                <img src={avatarDataUrl} alt="" draggable="false" />
              {/if}
              <span>Change</span>
            </button>
            <input
              bind:this={avatarInput}
              class="avatar-file"
              type="file"
              accept="image/png,image/jpeg,image/gif,image/webp"
              onchange={handleAvatarChange}
            />

            <div class="profile-name-card">
              <input
                bind:value={profileNameInput}
                maxlength="32"
                spellcheck="false"
                aria-label="Profile name"
                onblur={saveProfileName}
                onkeydown={handleProfileNameKeydown}
              />
            </div>

            {#if profileError}
              <p class="profile-error">{profileError}</p>
            {/if}

            {#if pendingAvatarBytes}
              <div class="profile-actions">
                <button onclick={cancelProfileEdit} disabled={profileSaving}>Cancel</button>
                <button class="save-profile" onclick={saveProfile} disabled={profileSaving}>
                  {profileSaving ? 'Saving...' : 'Save'}
                </button>
              </div>
            {/if}

            <div class="profile-wip">
              {@render IconConstruction()}
              <span>Work In Progress</span>
            </div>
          </div>
        </section>
      {/if}

      <div class="window-controls" data-no-drag>
        <button aria-label="Minimize" class="minimize" data-no-drag onclick={minimizeWindow}><span></span></button>
        <button aria-label="Close" class="close" data-no-drag onclick={closeWindow}>{@render IconClose()}</button>
      </div>

      <section class="subscriptions" data-tauri-drag-region aria-label="Subscriptions">
        <h1 data-tauri-drag-region>Subscription</h1>
        <p data-tauri-drag-region>Available subscriptions</p>

        <button class="subscription-card active" onclick={() => (game = 'csgo_legacy') && openDetails()}>
          <span>
            <strong>CS:GO (csgo_legacy)</strong>
            <em>Expires Never</em>
          </span>
          <img class="game-icon" src="/csgo.png" alt="" draggable="false" />
        </button>

        <button class="subscription-card active" onclick={() => (game = 'csgo') && openDetails()}>
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
              {#if game === 'csgo_legacy'}
                <h2>CS:GO (csgo_legacy)</h2>
              {:else}
                <h2>Counter-Strike: Global Offensive</h2>
              {/if}
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
                <button onclick={() => openExternal(DISCORD_URL)}>Community</button>
                <button onclick={() => openExternal(API_DOCS_URL)}>API Documentation</button>
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
                <button disabled={branchOpen || launchPending} class="load" onclick={() => launch(appids[game])}>
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

<style lang="postcss">
  @reference "../app.css";

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
    @apply fixed inset-0 z-[5] border-0 bg-transparent p-0;
  }

  .shell {
    @apply fixed left-1/2 top-1/2 z-10 h-[260px] w-[260px] overflow-hidden rounded-[9px] bg-[var(--nl-main-bg-opaque)];
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
    @apply h-[400px] w-[530px];
  }

  .boot-view {
    @apply absolute inset-0 grid place-items-center;
  }

  .boot-view > * {
    grid-area: 1 / 1;
  }

  .boot-spinner {
    @apply relative size-[34px] rounded-full animate-[spin_1400ms_linear_infinite];
  }

  .boot-spinner svg {
    @apply size-full text-[var(--nl-spinner)];
    filter: drop-shadow(0 0 7px rgba(150, 164, 255, 0.22));
  }

  .boot-spinner circle {
    @apply fill-none stroke-current;
    stroke-width: 4;
    stroke-linecap: round;
    stroke-dasharray: 1 107;
    stroke-dashoffset: 0;
    transform-origin: 50% 50%;
    animation: spinner-arc 1400ms ease-in-out infinite;
  }

  .launcher-view {
    @apply absolute inset-0;
    opacity: 0;
    animation: fade-in 280ms 170ms ease-in-out both;
  }

  .logo {
    @apply absolute left-[22px] top-[22px] text-[38px] font-black leading-none text-[var(--nl-logo)];
    font-family: "Museo Sans Local", "Museo Sans Cyrl", "Museo Sans Cyrillic", "Museo Sans", "Inter Variable", Inter, sans-serif;
    text-shadow: none;
  }

  .side-nav {
    @apply absolute left-[18px] top-[105px] grid gap-4;
  }

  .side-nav button,
  .footer-links button {
    @apply border-0 bg-transparent p-0 text-left text-[13px] font-light transition-colors duration-[130ms] ease-in-out;
  }

  .side-nav button {
    @apply text-[var(--nl-text)];
  }

  .footer-links button {
    @apply text-[var(--nl-text)];
  }

  .side-nav button:hover,
  .footer-links button:hover {
    color: color-mix(in srgb, var(--nl-text), var(--nl-active-text) 50%);
  }

  .profile {
    @apply absolute bottom-5 left-5 flex w-[132px] items-center gap-2 border-0 bg-transparent p-0 text-left text-sm font-thin leading-7 text-[var(--nl-active-text)] transition-opacity duration-[130ms] ease-in-out;
  }

  .profile:hover,
  .profile.active {
    @apply opacity-[0.82];
  }

  .profile span:last-child {
    @apply min-w-0 overflow-hidden text-ellipsis whitespace-nowrap;
  }

  .avatar {
    @apply relative block size-10 flex-none overflow-hidden rounded-full;
    background:
      radial-gradient(circle at 50% 36%, rgba(255, 255, 255, 0.82) 0 10%, transparent 11%),
      radial-gradient(circle at 38% 50%, #efe1d6 0 13%, transparent 14%),
      radial-gradient(circle at 58% 52%, #f3d7c9 0 12%, transparent 13%),
      radial-gradient(circle at 50% 70%, #1d1d25 0 25%, transparent 26%),
      linear-gradient(135deg, #c9cbd2, #4b4d57);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
    @apply opacity-90;
  }

  .avatar img {
    @apply block size-full object-cover;
  }

  .profile-popout {
    @apply absolute left-1/2 top-1/2 z-10 h-[340px] w-[460px] overflow-hidden rounded-[10px] border border-[rgba(255,255,255,0.075)];
    background: var(--nl-block-bg-opaque);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.018),
      0 18px 42px var(--nl-shadow-soft);
    transform: translate(-50%, -50%);
  }

  .profile-close {
    @apply right-[15px] top-[18px];
  }

  .profile-content {
    @apply absolute right-5 bottom-[17px] left-5 top-[46px];
  }

  .profile-popout-avatar {
    @apply mx-auto size-[84px] border-0 p-0 opacity-100;
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.2),
      0 10px 28px color-mix(in srgb, var(--nl-shadow), transparent 22%);
  }

  .profile-popout-avatar::before {
    content: "";
    @apply absolute inset-0 z-[1] rounded-full bg-black/0 transition-colors duration-150 ease-in-out;
  }

  .profile-popout-avatar span {
    @apply absolute inset-x-0 bottom-0 z-[2] translate-y-full bg-black/[0.72] pt-[5px] pb-[7px] text-center text-[10px] font-medium leading-none text-white transition-transform duration-[170ms] ease-in-out;
  }

  .profile-popout-avatar:hover::before {
    @apply bg-black/[0.28];
  }

  .profile-popout-avatar:hover span {
    @apply translate-y-0;
  }

  .avatar-file {
    @apply hidden;
  }

  .profile-name-card {
    @apply mt-3 grid h-[32px] place-items-center;
  }

  .profile-name-card input {
    @apply h-[32px] w-[178px] box-border rounded-[5px] border border-transparent bg-transparent px-2.5 py-0 text-center text-[15px] font-medium text-[var(--nl-active-text)] outline-0 transition-[background,border-color] duration-[130ms] ease-in-out;
  }

  .profile-name-card input:hover,
  .profile-name-card input:focus {
    background: color-mix(in srgb, var(--nl-main-bg-opaque), white 4%);
    border-color: color-mix(in srgb, var(--nl-link), white 18%);
  }

  .profile-error {
    @apply mt-[7px] mb-0 text-center text-[11px] leading-[1.35] text-[#ff8d8d];
  }

  .profile-actions {
    @apply mt-[10px] flex justify-center gap-2;
  }

  .profile-actions button {
    @apply h-7 min-w-[62px] rounded-md border border-[var(--nl-border)] bg-transparent px-2.5 py-0 text-xs font-light text-[var(--nl-text)] transition-[background,color,opacity] duration-[130ms] ease-in-out;
  }

  .profile-actions button:hover {
    color: color-mix(in srgb, var(--nl-text), var(--nl-active-text) 50%);
  }

  .profile-actions button:disabled {
    @apply opacity-[0.55];
  }

  .profile-actions .save-profile {
    @apply border-transparent bg-[var(--nl-button)] text-[var(--nl-button-active-text)];
  }

  .profile-actions .save-profile:hover {
    background: var(--nl-button-active);
    color: var(--nl-button-active-text);
  }

  .profile-wip {
    @apply absolute inset-x-0 bottom-2 flex items-center justify-center gap-2 text-[12px] font-light text-[var(--nl-small-text)];
  }

  .profile-wip svg {
    @apply size-[15px];
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }


  .window-controls {
    @apply absolute right-[17px] top-[15px] z-[15] flex items-center gap-2;
  }

  .window-controls button,
  .detail-close {
    @apply grid size-6 place-items-center border-0 bg-transparent p-0 text-[var(--nl-text)] transition-[color,opacity] duration-[120ms] ease-in-out;
  }

  .minimize::before {
    content: none;
  }

  .minimize span {
    @apply block h-[1.5px] w-3 rounded-full bg-current;
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
    @apply absolute left-44 top-4 w-[323px];
  }

  .subscriptions h1 {
    @apply m-0 translate-x-[7px] text-[19px] font-medium leading-[1.7] text-[var(--nl-active-text)];
  }

  .subscriptions > p {
    @apply mt-1.5 mb-[45px] translate-x-[7px] text-[14.5px] font-light text-[var(--nl-text)];
  }

  .subscription-card {
    @apply relative mb-[11px] block h-[73px] w-full overflow-hidden rounded-xl border-[0.5px] border-solid border-[var(--nl-border)] bg-[var(--nl-block-bg)] p-0 text-left;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.018);
  }

  .subscription-card.active {
    @apply cursor-default;
    animation: card-in 360ms 260ms ease-in-out both;
    transition: none;
  }

  .subscription-card span {
    @apply absolute left-3 top-2;
  }

  .subscription-card strong {
    @apply block text-[13.5px] font-medium text-[var(--nl-active-text)];
  }

  .subscription-card em {
    @apply mt-[9px] block text-[13px] font-thin not-italic text-[var(--nl-text-preview)];
  }

  .game-icon {
    @apply pointer-events-none absolute right-2.5 top-[11px] size-[22px] rounded-[5px] object-cover;
    box-shadow: 0 0 18px rgba(240, 139, 11, 0.2);
  }

  .background-dim {
    @apply absolute inset-0 z-[8] cursor-default border-0 bg-black/0 p-0;
    transition: background 360ms var(--ease-soft);
  }

  .background-dim.visible {
    @apply bg-[rgba(0,0,0,0.66)];
  }

  .details {
    @apply absolute left-1/2 top-1/2 z-10 h-[340px] w-[460px] overflow-hidden rounded-[10px] border border-[rgba(255,255,255,0.075)] bg-[var(--nl-block-bg-opaque)];
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
    @apply h-[250px] w-[375px] rounded bg-[var(--nl-block-bg-opaque)] pointer-events-none;
    animation: none;
  }

  .details.closing {
    @apply opacity-0 pointer-events-none;
    animation: none;
    transform: translate(-50%, -50%) scale(0.985);
    transition:
      opacity 210ms var(--ease-soft),
      transform 240ms var(--ease-smooth);
  }

  .detail-content {
    @apply absolute inset-0 z-[1] opacity-100;
    transition: opacity 280ms var(--ease-soft), transform 320ms var(--ease-smooth);
    transform: translateY(0) scale(1);
  }

  .details.fading .detail-content {
    @apply opacity-0 pointer-events-none;
    transform: translateY(2px) scale(0.995);
  }

  .details header {
    @apply relative h-[70px] border-b border-[var(--nl-separator)];
  }

  .details .large {
    @apply left-[17px] top-[19px] size-[30px] rounded-lg;
  }

  .details h2 {
    @apply absolute left-[58px] top-1/2 m-0 -translate-y-1/2 text-lg font-medium text-[var(--nl-active-text)];
  }

  .detail-close {
    @apply absolute right-[15px] top-[25px] text-lg;
  }

  .detail-body {
    @apply grid grid-cols-[197px_1fr] gap-0 px-5 pt-4 pb-0;
  }

  .metadata p,
  .metadata-row,
  .changelog p {
    @apply m-0 text-[13px] font-light leading-[1.85];
  }

  .metadata p,
  .metadata-row {
    @apply mb-[3px];
  }

  .metadata-row {
    @apply relative;
  }

  .metadata-row.with-menu {
    @apply flex flex-nowrap items-center gap-1;
  }

  .metadata .label {
    @apply flex-none font-light text-[var(--nl-active-text)];
  }

  .metadata .value {
    @apply ml-1 font-medium text-[var(--nl-link-active)];
  }

  .metadata-trigger {
    @apply m-0 inline-grid h-[22px] max-w-[150px] items-center gap-1.5 rounded-[5px] bg-transparent px-[7px] py-0 text-left font-medium text-[var(--nl-link-active)] align-middle;
    grid-template-columns: 15px minmax(0, auto);
    border: 1px solid color-mix(in srgb, var(--nl-border), var(--nl-text) 14%);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--nl-border), transparent 45%);
    font: inherit;
    font-weight: 500;
    transition: background 130ms ease-in-out, border-color 130ms ease-in-out, box-shadow 130ms ease-in-out, color 130ms ease-in-out;
  }

  .metadata-trigger:focus {
    outline: none;
  }

  .metadata-trigger:focus-visible {
    outline: 1px solid color-mix(in srgb, var(--nl-button-active), transparent 45%);
    outline-offset: 1px;
  }

  .metadata-trigger span:last-child {
    @apply overflow-hidden text-ellipsis whitespace-nowrap;
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
    @apply block size-[15px] justify-self-center text-current;
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
    @apply size-[15px];
    stroke-width: 1.65;
  }

  .metadata-menu {
    @apply absolute left-[68px] top-7 z-[35] max-h-[116px] w-max min-w-[132px] max-w-[196px] overflow-auto rounded-[10px] py-1.5;
    border: 1px solid color-mix(in srgb, var(--nl-button-active), transparent 84%);
    background: color-mix(in srgb, var(--nl-popup-bg), transparent 22%);
    box-shadow:
      0 0 14px color-mix(in srgb, var(--nl-button-active), transparent 94%),
      inset 0 0 0 1px color-mix(in srgb, var(--nl-button-active), transparent 95%),
      0 12px 30px var(--nl-shadow-soft);
    backdrop-filter: blur(9px);
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--nl-active-text), transparent 78%) transparent;
    transform-origin: left top;
    animation: menu-in 190ms ease-in-out both;
  }

  .config-menu {
    min-width: 132px;
    max-width: 164px;
  }

  .metadata-menu button {
    @apply relative isolate mx-0 block h-[26px] w-full overflow-hidden text-ellipsis whitespace-nowrap rounded-none border-0 bg-transparent px-[11px] py-0 text-left text-[13px] font-light text-[var(--nl-text-preview)];
    transition: background 130ms ease-in-out, color 130ms ease-in-out, text-shadow 130ms ease-in-out;
  }

  .metadata-menu button::before {
    @apply pointer-events-none absolute inset-y-0 left-2 right-2 z-[-1] rounded-[5px] opacity-0;
    content: "";
    background: var(--nl-selection);
    transition: opacity 130ms ease-in-out;
  }

  .metadata-menu button:hover {
    color: color-mix(in srgb, var(--nl-active-text), var(--nl-text-preview) 50%);
    background: transparent;
    text-shadow: 0 0 10px color-mix(in srgb, var(--nl-active-text), transparent 86%);
  }

  .metadata-menu button:hover::before {
    @apply opacity-50;
  }

  .metadata-menu button:active {
    color: var(--nl-active-text);
  }

  .metadata-menu .selected,
  .metadata-menu .selected:hover {
    color: var(--nl-active-text);
    text-shadow: none;
  }

  .metadata-menu button:active::before,
  .metadata-menu .selected:active::before {
    @apply opacity-100;
  }

  .metadata-menu button:disabled {
    @apply pointer-events-none text-[var(--nl-text-preview)] opacity-[0.58];
  }

  .changelog {
    @apply max-h-[154px] overflow-x-hidden overflow-y-auto break-words pr-2 text-[var(--nl-text)];
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
    @apply absolute right-[17px] bottom-[15px] left-[17px] flex h-7 items-center justify-between;
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
    @apply inline-flex h-7 items-center justify-center rounded-md text-[13px] font-light text-[var(--nl-button-active-text)];
  }

  .branch-trigger {
    width: 32px;
    border: 1px solid color-mix(in srgb, var(--nl-border), var(--nl-text) 14%);
    background: transparent;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--nl-border), transparent 35%);
    transition: background 130ms ease-in-out, border-color 130ms ease-in-out, box-shadow 130ms ease-in-out;
  }

  .branch-trigger svg {
    @apply size-[19px];
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
    @apply w-[102px] border-0 bg-[var(--nl-button)] transition-[background,opacity] duration-[130ms] ease-in-out;
  }

  .load .play-icon {
    @apply mr-3 ml-[-4px] inline-flex text-[var(--nl-button-active-text)];
  }

  .load svg {
    @apply h-5 w-[18px] fill-none;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .load:hover {
    background: var(--nl-button-active);
  }

  .load:disabled {
    @apply pointer-events-none opacity-[0.55];
  }

  .branch-menu {
    @apply absolute left-[34px] top-[-34px] z-30 h-[66px] w-[122px] overflow-hidden rounded-md pt-2;
    border: 1px solid color-mix(in srgb, var(--nl-button-active), transparent 84%);
    background: color-mix(in srgb, var(--nl-popup-bg), transparent 28%);
    box-shadow:
      0 0 14px color-mix(in srgb, var(--nl-button-active), transparent 94%),
      inset 0 0 0 1px color-mix(in srgb, var(--nl-button-active), transparent 95%),
      0 12px 30px var(--nl-shadow-soft);
    backdrop-filter: blur(9px);
    transform-origin: left bottom;
    animation: menu-in 190ms ease-in-out both;
  }

  .branch-menu button {
    @apply relative isolate mx-0 grid h-[29px] w-full items-center rounded-none border-0 bg-transparent py-0 pr-0 pl-[14px] text-left text-sm font-thin text-[var(--nl-text-preview)];
    grid-template-columns: 24px 1fr;
    column-gap: 11px;
    transition: background 130ms ease-in-out, color 130ms ease-in-out, text-shadow 130ms ease-in-out;
  }

  .branch-menu button::before {
    @apply pointer-events-none absolute inset-y-0 left-2 right-2 z-[-1] rounded-[5px] opacity-0;
    content: "";
    background: var(--nl-selection);
    transition: opacity 130ms ease-in-out;
  }

  .branch-menu button > * {
    @apply relative z-[1];
  }

  .branch-icon {
    @apply block size-[17px] justify-self-center bg-current opacity-90;
    mask: url("/git-branch.svg") center / contain no-repeat;
    -webkit-mask: url("/git-branch.svg") center / contain no-repeat;
  }

  .branch-menu button:hover {
    color: var(--nl-active-text);
    background: transparent;
    text-shadow: 0 0 10px color-mix(in srgb, var(--nl-active-text), transparent 86%);
  }

  .branch-menu button:hover::before {
    @apply opacity-50;
  }

  .branch-menu button:active {
    color: var(--nl-active-text);
  }

  .branch-menu .selected,
  .branch-menu .selected:hover {
    color: var(--nl-active-text);
    text-shadow: none;
  }

  .branch-menu button:active::before,
  .branch-menu .selected:active::before {
    @apply opacity-100;
  }

  .launch-content {
    --launch-content-width: 249px;
    @apply pointer-events-none absolute inset-0 z-[2] grid place-items-center content-center opacity-0;
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
    @apply static size-[46px] rounded-[7px];
    animation: launch-icon-pulse 2s var(--ease-smooth) infinite;
    transform-origin: center;
  }

  .launch-content p {
    @apply mt-[30px] mb-4 w-[var(--launch-content-width)] justify-self-center text-center text-[13px] text-[var(--nl-text)] whitespace-nowrap;
  }

  .progress {
    @apply h-1 w-[var(--launch-content-width)] overflow-hidden rounded-full bg-[var(--nl-frame-bg)];
  }

  .progress span {
    @apply block h-full rounded-[inherit] bg-[var(--nl-button)];
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
    }

    to {
      opacity: 1;
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
