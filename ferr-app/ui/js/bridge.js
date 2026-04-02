// bridge.js — JS wrapper around Tauri invoke() + event listeners
//
// Set DEV_MODE = true while the real ferr CLI is not yet bundled.
// All mock responses match the machine-readable progress protocol exactly.

const DEV_MODE = false;

const Bridge = (() => {
    // ── Tauri API helpers ──────────────────────────────────────────────────
    function invoke(cmd, args) {
        return window.__TAURI__?.core.invoke(cmd, args)
            ?? Promise.reject(new Error('Tauri not available'));
    }

    function listen(event, cb) {
        if (window.__TAURI__) {
            return window.__TAURI__.event.listen(event, e => cb(e.payload));
        }
        return Promise.resolve(() => {});
    }

    // ── Dev mode mock helpers ──────────────────────────────────────────────
    let _progressCb = null;
    let _errorCb = null;
    let _completeCb = null;

    function _mockCopy() {
        return new Promise(resolve => {
            let bytes = 0;
            const total = 2_400_000_000;
            const files = ['A001C001_240101.MXF', 'A001C002_240101.MXF', 'A001C003_240101.MXF'];
            let fileIdx = 0;
            let count = 0;
            const totalFiles = files.length;

            const iv = setInterval(() => {
                bytes += 80_000_000 + Math.random() * 20_000_000;
                if (bytes > total) bytes = total;
                count = Math.floor((bytes / total) * totalFiles);
                const speed = (75 + Math.random() * 20).toFixed(0) + ' MB/s';
                const line = `PROGRESS:${Math.floor(bytes)}/${total}|${count}/${totalFiles}|${speed}|${files[Math.min(fileIdx, files.length - 1)]}`;
                _progressCb?.(line);
                fileIdx = Math.min(Math.floor((bytes / total) * files.length), files.length - 1);
                if (bytes >= total) {
                    clearInterval(iv);
                    _progressCb?.(`COMPLETE:${totalFiles}|${total}|0|/tmp/mock_manifest.json`);
                    _completeCb?.(0);
                    resolve();
                }
            }, 150);
        });
    }

    function _mockHistory() {
        return [
            { id: '1', date: '2024-01-15T09:23:11Z', source: '/Volumes/A001', destinations: ['/Volumes/BACKUP1'], files: 47, bytes: 12_800_000_000, errors: 0, manifest: '/tmp/a.json' },
            { id: '2', date: '2024-01-14T14:05:33Z', source: '/Volumes/B002', destinations: ['/Volumes/CARD_B'], files: 23, bytes: 4_200_000_000, errors: 0, manifest: '/tmp/b.json' },
            { id: '3', date: '2024-01-13T08:11:00Z', source: '/Volumes/A002', destinations: ['/Volumes/BACKUP2', '/Volumes/OFFSITE'], files: 61, bytes: 18_000_000_000, errors: 1, manifest: '/tmp/c.json' },
        ];
    }

    function _mockVolumes() {
        return [
            { name: 'Macintosh HD', mount: '/', total: 500_000_000_000, used: 220_000_000_000, free: 280_000_000_000, removable: false },
            { name: 'A001', mount: '/Volumes/A001', total: 256_000_000_000, used: 180_000_000_000, free: 76_000_000_000, removable: true },
        ];
    }

    function _mockScan() {
        return new Promise(resolve => {
            let i = 0;
            const total = 12;
            const files = ['clip_001.mxf','clip_002.mxf','clip_003.mxf','sidecar.xml','report.pdf'];
            const iv = setInterval(() => {
                i++;
                const f = files[i % files.length];
                _progressCb?.(`SCAN_PROGRESS:${i}/${total}|${f}`);
                if (i >= total) {
                    clearInterval(iv);
                    _progressCb?.(`SCAN_RESULT:OK|${total}|0`);
                    _completeCb?.(0);
                    resolve();
                }
            }, 100);
        });
    }

    // ── Public API ─────────────────────────────────────────────────────────
    const api = {
        // File pickers — always use real Tauri dialog (even in DEV_MODE)
        pickFolder: () => invoke('pick_folder'),
        pickFile: (ext) => invoke('pick_file', { extensions: ext || [] }),
        pickSaveLocation: (name) => invoke('pick_save_location', { defaultName: name }),

        // Copy
        runCopy: (src, dests, args) => DEV_MODE
            ? _mockCopy()
            : invoke('run_copy', { source: src, destinations: dests, args }),

        runCopyPreview: (src, dests, args) => DEV_MODE
            ? _mockCopy()
            : invoke('run_copy_preview', { source: src, destinations: dests, args }),

        // Watch
        startWatch: (folder, dests, args) => DEV_MODE
            ? Promise.resolve()
            : invoke('run_watch_start', { folder, destinations: dests, args }),

        stopWatch: () => DEV_MODE
            ? Promise.resolve()
            : invoke('run_watch_stop'),

        // Health
        runScan: (folder, since) => DEV_MODE
            ? _mockScan()
            : invoke('run_scan', { folder, sinceDate: since }),

        runVerify: (srcOrManifest, dest) => DEV_MODE
            ? new Promise(r => { setTimeout(() => { _progressCb?.('VERIFY_RESULT:OK|47|0|0'); _completeCb?.(0); r(); }, 1200); })
            : invoke('run_verify', { sourceOrManifest: srcOrManifest, dest }),

        runRepair: (folder) => DEV_MODE
            ? new Promise(r => { setTimeout(() => { _completeCb?.(0); r(); }, 800); })
            : invoke('run_repair', { folder }),

        certCreate: (folder, out) => DEV_MODE
            ? new Promise(r => { setTimeout(() => { _completeCb?.(0); r(); }, 600); })
            : invoke('run_cert_create', { folder, outputPath: out }),

        certVerify: (cert, folder) => DEV_MODE
            ? new Promise(r => { setTimeout(() => { _progressCb?.('VERIFY_RESULT:OK|1|0|0'); _completeCb?.(0); r(); }, 700); })
            : invoke('run_cert_verify', { certPath: cert, folder }),

        // Export
        exportALE: (manifest, out) => DEV_MODE
            ? Promise.resolve()
            : invoke('run_export', { manifestPath: manifest, format: 'ale', outputPath: out }),

        exportCSV: (manifest, out) => DEV_MODE
            ? Promise.resolve()
            : invoke('run_export', { manifestPath: manifest, format: 'csv', outputPath: out }),

        generateReport: (manifest, out) => DEV_MODE
            ? Promise.resolve()
            : invoke('run_report', { manifestPath: manifest, outputPath: out }),

        // History
        getHistory: () => DEV_MODE
            ? Promise.resolve(_mockHistory())
            : invoke('get_history').then(JSON.parse),

        searchHistory: (q) => DEV_MODE
            ? Promise.resolve(_mockHistory().filter(s => JSON.stringify(s).toLowerCase().includes(q.toLowerCase())))
            : invoke('search_history', { query: q }).then(JSON.parse),

        // Profiles
        getProfiles: () => DEV_MODE
            ? Promise.resolve([{ name: 'Documentary', hash: 'xxhash', par2: true, par2Percent: 10 }])
            : invoke('get_profiles').then(JSON.parse),

        saveProfile: (name) => DEV_MODE
            ? Promise.resolve()
            : invoke('save_profile', { name }),

        // Volumes
        getVolumes: () => DEV_MODE
            ? Promise.resolve(_mockVolumes())
            : invoke('get_volumes'),

        // Event subscriptions
        onProgress: (cb) => { _progressCb = cb; return listen('ferr-progress', cb); },
        onError:    (cb) => { _errorCb = cb;    return listen('ferr-error', cb); },
        onComplete: (cb) => { _completeCb = cb; return listen('ferr-complete', cb); },
        onWatchStarted: (cb) => listen('ferr-watch-started', cb),
        onWatchStopped: (cb) => listen('ferr-watch-stopped', cb),
    };

    return api;
})();
