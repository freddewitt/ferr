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
        { id: 'copy',      labelKey: 'copy',       icon: _iconCopy() },
        { id: 'certificat',labelKey: 'certificat', icon: _iconCert() },
        { id: 'watch',     labelKey: 'watch',      icon: _iconWatch() },
        { id: 'health',    labelKey: 'health',     icon: _iconHealth() },
        { id: 'history',   labelKey: 'history',    icon: _iconHistory() },
        { id: 'settings',  labelKey: 'settings',   icon: _iconSettings() },
        { id: 'log',       labelKey: 'log',        icon: _iconLog(),    badge: true },
    ];

    // ── Init ───────────────────────────────────────────────────────────────
    async function init() {
        await Settings.load();
        await I18n.load(Settings.get('language'));

        _renderSidebar();
        _renderTopBar();
        _renderBottomBar();

        let _lastResult = null;
        Bridge.onProgress(line => {
            Progress.update(line);
            if (line.startsWith('COMPLETE:')) {
                const parts = line.substring(9).split('|');
                _lastResult = { type: 'copy', files: parts[0], bytes: Fmt.bytes(parts[1]), errors: parts[2], manifest: parts[3] };
            } else if (line.startsWith('SCAN_RESULT:')) {
                const parts = line.substring(12).split('|');
                _lastResult = { type: 'scan', files: parts[1], bytes: '—', errors: parts[2] };
            } else if (line.startsWith('VERIFY_RESULT:')) {
                const parts = line.substring(14).split('|');
                _lastResult = { type: 'verify', files: parts[1], bytes: '—', errors: parts[3] || parts[2] };
            } else if (line.startsWith('VERIFY_ISSUE:') || line.startsWith('SCAN_ISSUE:')) {
                const parts = line.substring(line.indexOf(':') + 1).split('|');
                if (!_lastResult) _lastResult = { issues: [] };
                if (!_lastResult.issues) _lastResult.issues = [];
                _lastResult.issues.push({
                    severity: parts[0],
                    kind: parts[1],
                    path: parts[2],
                    detail: parts[3]
                });
            }
        });
        Bridge.onComplete(code => {
            Progress.hide();
            state.copyInProgress = false;
            _renderBottomBar();

            if (_lastResult) {
                Modal.show({
                    title: _lastResult.type === 'copy' ? t('complete') : t('operation_result'),
                    files: _lastResult.files,
                    bytes: _lastResult.bytes,
                    errors: _lastResult.errors,
                    issues: _lastResult.issues || [],
                    showManifestBtn: !!_lastResult.manifest,
                    manifestPath: _lastResult.manifest,
                    icon: (_lastResult.errors > 0 || (_lastResult.issues?.length > 0)) ? 'alert' : 'check'
                });
                _lastResult = null;
            } else if (code !== 0) {
                Modal.show({ title: t('history_error'), icon: 'alert', msg: 'Process exited with code ' + code });
            }
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

        // Init log tab (starts listening to bridge events)
        LogTab.init();

        switchTab('copy');
        loadVolumes();
    }

    // ── Sidebar ────────────────────────────────────────────────────────────
    function _renderSidebar() {
        const sidebar = document.getElementById('sidebar');
        sidebar.innerHTML = TABS.map(tab => {
            const label = t(tab.labelKey);
            return `
                <button class="sidebar-item ${tab.id === state.activeTab ? 'active' : ''}"
                        data-tab="${tab.id}" title="${label}" style="position:relative">
                    ${tab.icon}
                    <span>${label}</span>
                    ${tab.badge ? `<span class="log-badge" id="log-badge" style="display:none"></span>` : ''}
                </button>
            `;
        }).join('') + `
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
        _renderTopBar();
    }

    function updateLanguage() {
        _renderSidebar();
        _renderTopBar();
        _renderBottomBar();
        // Re-render current tab
        switchTab(state.activeTab);
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

        if ((tab === 'health' || tab === 'certificat') && state.bottomBarAction) {
            const { label, variant } = state.bottomBarAction;
            return `<button class="btn ${variant === 'danger' ? 'btn-danger' : 'btn-primary'}" id="bb-tab-action">${label}</button>`;
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
        document.getElementById('bb-tab-action')?.addEventListener('click', () => {
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
        // Notify log tab it's being left
        if (state.activeTab === 'log' && name !== 'log') LogTab.onTabLeave();

        state.activeTab = name;
        state.bottomBarAction = null;

        document.querySelectorAll('.sidebar-item').forEach(btn => {
            btn.classList.toggle('active', btn.dataset.tab === name);
        });

        const content = document.getElementById('tab-content');
        switch (name) {
            case 'copy':     CopyTab.render(content);     break;
            case 'certificat': CertTab.render(content);   break;
            case 'watch':    WatchTab.render(content);    break;
            case 'health':   HealthTab.render(content);   break;
            case 'history':  HistoryTab.render(content);  break;
            case 'settings': SettingsTab.render(content); break;
            case 'log':      LogTab.render(content);      break;
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
        if (s.pdfReport) args.push('--pdf');
        if (!s.notifications) args.push('--no-notify');
        if (s.jsonManifest) args.push('--report');
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
        if (s.videoMode) {
            args.push('--camera', '--rename', s.renameTemplate);
            if (s.historyDedup) args.push('--dedup');
        }
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
            el.className = 'flash-msg';
            document.body.appendChild(el);
        }
        el.textContent = msg;
        el.classList.add('visible');
        clearTimeout(el._timer);
        el._timer = setTimeout(() => el.classList.remove('visible'), 2500);
    }

    function _tabTitle(id) {
        return t(TABS.find(t => t.id === id)?.labelKey ?? id);
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
    function _iconLog() {
        return `<svg viewBox="0 0 24 24"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>`;
    }

    function _iconCert() {
        return `<svg viewBox="0 0 24 24"><circle cx="12" cy="8" r="6"/><path d="M15.477 12.89 17 22l-5-3-5 3 1.523-9.11"/></svg>`;
    }

    return {
        init,
        switchTab,
        refreshPills,
        updateBottomBar,
        setBottomBarAction,
        loadVolumes,
        updateLanguage,
        flash,
        buildCopyArgs: () => _buildCopyArgs(),
        get state() { return state; },
    };
})();

document.addEventListener('DOMContentLoaded', () => App.init());
