// app.js — Main app: routing, state, init, topbar/sidebar/bottombar

const App = (() => {
    const state = {
        activeTab: 'copy',
        copyInProgress: false,
        watchActive: false,
        volumes: [],
        bottomBarAction: null,   // { label, fn, variant }
    };

    const TABS = [
        { id: 'copy',    label: t('copy'),    icon: _iconCopy() },
        { id: 'watch',   label: t('watch'),   icon: _iconWatch() },
        { id: 'health',  label: t('health'),  icon: _iconHealth() },
        { id: 'history', label: t('history'), icon: _iconHistory() },
        { id: 'settings',label: t('settings'),icon: _iconSettings() },
    ];

    // ── Init ───────────────────────────────────────────────────────────────
    async function init() {
        await Settings.load();
        await I18n.load(Settings.get('language'));

        _renderSidebar();
        _renderTopBar();
        _renderBottomBar();

        Bridge.onProgress(line => Progress.update(line));
        Bridge.onComplete(code => {
            Progress.hide();
            state.copyInProgress = false;
            _renderBottomBar();
        });
        Bridge.onError(line => console.error('[ferr]', line));
        Bridge.onWatchStarted(() => {
            state.watchActive = true;
            WatchTab.setWatching(true);
            refreshPills();
        });
        Bridge.onWatchStopped(() => {
            state.watchActive = false;
            WatchTab.setWatching(false);
            refreshPills();
        });

        switchTab('copy');
        loadVolumes();
    }

    // ── Sidebar ────────────────────────────────────────────────────────────
    function _renderSidebar() {
        const sidebar = document.getElementById('sidebar');
        sidebar.innerHTML = TABS.map(tab => `
            <button class="sidebar-item ${tab.id === state.activeTab ? 'active' : ''}"
                    data-tab="${tab.id}" title="${tab.label}">
                ${tab.icon}
                <span>${tab.label}</span>
            </button>
        `).join('') + `
            <div class="sidebar-spacer"></div>
            <button class="sidebar-item" id="quit-btn" title="${t('quit')}">
                <svg viewBox="0 0 24 24"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
                <span>${t('quit')}</span>
            </button>
        `;

        sidebar.querySelectorAll('.sidebar-item[data-tab]').forEach(btn => {
            btn.addEventListener('click', () => switchTab(btn.dataset.tab));
        });

        document.getElementById('quit-btn').addEventListener('click', () => {
            if (window.__TAURI__) {
                window.__TAURI__.core.invoke('quit_app');
            } else {
                window.close();
            }
        });
    }

    // ── Topbar ─────────────────────────────────────────────────────────────
    function _renderTopBar() {
        const topbar = document.getElementById('topbar');
        const s = Settings.getAll();
        const pills = Pills.buildFromSettings(s, state.watchActive);
        topbar.innerHTML = `
            <span class="topbar-title">${_tabTitle(state.activeTab)}</span>
            <div class="flex-spacer"></div>
            ${Pills.render(pills)}
        `;
    }

    function refreshPills() {
        const topbar = document.getElementById('topbar');
        const s = Settings.getAll();
        const pills = Pills.buildFromSettings(s, state.watchActive);
        topbar.innerHTML = `
            <span class="topbar-title">${_tabTitle(state.activeTab)}</span>
            <div class="flex-spacer"></div>
            ${Pills.render(pills)}
        `;
    }

    // ── Bottom bar ─────────────────────────────────────────────────────────
    function _renderBottomBar() {
        const bar = document.getElementById('bottombar');
        const vol = state.volumes[0];
        const diskHtml = vol ? `
            <div class="disk-info">
                <span>${vol.name} — ${Fmt.bytes(vol.free)} free of ${Fmt.bytes(vol.total)}</span>
                <div class="disk-bar">
                    <div class="disk-bar-fill" style="width:${Fmt.diskBar(vol.used ?? vol.total - vol.free, vol.total)}%"></div>
                </div>
            </div>
        ` : `<div class="disk-info"><span>${t('no_volumes')}</span></div>`;

        const actionHtml = _buildBottomActions();

        bar.innerHTML = diskHtml + '<div class="flex-spacer"></div>' + actionHtml;
        _bindBottomBarButtons();
    }

    function _buildBottomActions() {
        const tab = state.activeTab;

        if (tab === 'copy') {
            if (state.copyInProgress) {
                return `<button class="btn btn-danger" id="bb-cancel">${t('cancel_btn')}</button>`;
            }
            return `
                <button class="btn" id="bb-preview">${t('preview_btn')}</button>
                <button class="btn btn-primary" id="bb-copy">${t('copy_btn')}</button>
            `;
        }

        if (tab === 'watch') {
            if (state.watchActive) {
                return `<button class="btn btn-danger" id="bb-watch-stop">${t('stop_watching')}</button>`;
            }
            return `<button class="btn btn-primary" id="bb-watch-start">${t('start_watching')}</button>`;
        }

        if (tab === 'health' && state.bottomBarAction) {
            const { label, variant } = state.bottomBarAction;
            return `<button class="btn ${variant === 'danger' ? 'btn-danger' : 'btn-primary'}" id="bb-health-action">${label}</button>`;
        }

        return '';
    }

    function _bindBottomBarButtons() {
        document.getElementById('bb-copy')?.addEventListener('click', _doCopy);
        document.getElementById('bb-preview')?.addEventListener('click', _doPreview);
        document.getElementById('bb-cancel')?.addEventListener('click', () => {
            Progress.hide();
            state.copyInProgress = false;
            _renderBottomBar();
        });
        document.getElementById('bb-watch-start')?.addEventListener('click', _doWatchStart);
        document.getElementById('bb-watch-stop')?.addEventListener('click',  _doWatchStop);
        document.getElementById('bb-health-action')?.addEventListener('click', () => {
            state.bottomBarAction?.fn?.();
        });
    }

    function setBottomBarAction(label, fn, variant) {
        state.bottomBarAction = { label, fn, variant };
        _renderBottomBar();
    }

    function updateBottomBar() { _renderBottomBar(); }

    // ── Tab routing ────────────────────────────────────────────────────────
    function switchTab(name) {
        state.activeTab = name;
        state.bottomBarAction = null;

        document.querySelectorAll('.sidebar-item').forEach(btn => {
            btn.classList.toggle('active', btn.dataset.tab === name);
        });

        const content = document.getElementById('tab-content');
        switch (name) {
            case 'copy':     CopyTab.render(content);    break;
            case 'watch':    WatchTab.render(content);   break;
            case 'health':   HealthTab.render(content);  break;
            case 'history':  HistoryTab.render(content); break;
            case 'settings': SettingsTab.render(content); break;
        }

        _renderTopBar();
        _renderBottomBar();
    }

    // ── Copy actions ───────────────────────────────────────────────────────
    async function _doCopy() {
        if (!CopyTab.isReady()) return flash(t('select_first'));
        const src   = CopyTab.getSource();
        const dests = CopyTab.getDestinations();
        const args  = _buildCopyArgs();
        state.copyInProgress = true;
        _renderBottomBar();
        Progress.show(t('copying'), { onCancel: () => { state.copyInProgress = false; } });
        try {
            await Bridge.runCopy(src, dests, args);
        } catch (e) {
            flash('Copy failed: ' + e);
        } finally {
            state.copyInProgress = false;
            Progress.hide();
            _renderBottomBar();
        }
    }

    async function _doPreview() {
        if (!CopyTab.isReady()) return flash(t('select_first'));
        const src   = CopyTab.getSource();
        const dests = CopyTab.getDestinations();
        const args  = _buildCopyArgs();
        Progress.show(t('preview_dry'));
        try {
            await Bridge.runCopyPreview(src, dests, args);
        } finally {
            Progress.hide();
        }
    }

    function _buildCopyArgs() {
        const s = Settings.getAll();
        const args = ['--hash', s.hashAlgorithm];
        if (s.par2Enabled) args.push('--par2', String(s.par2Percent));
        if (s.videoMode) {
            args.push('--camera', '--rename', s.renameTemplate);
            if (s.historyDedup) args.push('--dedup');
        }
        if (s.ejectAfterCopy) args.push('--eject');
        if (!s.preserveMetadata) args.push('--no-preserve-meta');
        if (!s.pdfReport) args.push('--no-pdf');
        if (!s.notifications) args.push('--no-notify');
        return args;
    }

    // ── Watch actions ──────────────────────────────────────────────────────
    async function _doWatchStart() {
        if (!WatchTab.isReady()) return flash(t('select_first'));
        const folder = WatchTab.getFolder();
        const dests  = WatchTab.getDestinations();
        const s      = Settings.getAll();
        const args   = ['--hash', s.hashAlgorithm];
        if (s.par2Enabled) args.push('--par2', String(s.par2Percent));
        if (s.videoMode)   args.push('--camera');
        if (s.ejectAfterCopy) args.push('--eject');
        try {
            await Bridge.startWatch(folder, dests, args);
        } catch (e) {
            flash('Failed to start watch: ' + e);
        }
    }

    async function _doWatchStop() {
        try {
            await Bridge.stopWatch();
        } catch (e) {
            flash('Failed to stop watch: ' + e);
        }
    }

    // ── Volumes ────────────────────────────────────────────────────────────
    async function loadVolumes() {
        try {
            state.volumes = await Bridge.getVolumes();
            _renderBottomBar();
        } catch {}
    }

    // ── Flash message ──────────────────────────────────────────────────────
    function flash(msg) {
        let el = document.getElementById('flash-msg');
        if (!el) {
            el = document.createElement('div');
            el.id = 'flash-msg';
            el.style.cssText = `
                position:fixed; bottom:68px; left:50%; transform:translateX(-50%);
                background:var(--bg-tertiary); border:1px solid var(--border);
                border-radius:8px; padding:8px 16px; font-size:13px;
                color:var(--text-primary); z-index:300; pointer-events:none;
                transition:opacity 0.3s;
            `;
            document.body.appendChild(el);
        }
        el.textContent = msg;
        el.style.opacity = '1';
        clearTimeout(el._timer);
        el._timer = setTimeout(() => el.style.opacity = '0', 2500);
    }

    function _tabTitle(id) {
        return TABS.find(t => t.id === id)?.label ?? '';
    }

    // ── SVG icons ──────────────────────────────────────────────────────────
    function _iconCopy() {
        return `<svg viewBox="0 0 24 24"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
    }
    function _iconWatch() {
        return `<svg viewBox="0 0 24 24"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>`;
    }
    function _iconHealth() {
        return `<svg viewBox="0 0 24 24"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>`;
    }
    function _iconHistory() {
        return `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>`;
    }
    function _iconSettings() {
        return `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>`;
    }

    return {
        init,
        switchTab,
        refreshPills,
        updateBottomBar,
        setBottomBarAction,
        loadVolumes,
        flash,
        buildCopyArgs: () => _buildCopyArgs(),
        get state() { return state; },
    };
})();

document.addEventListener('DOMContentLoaded', () => App.init());
