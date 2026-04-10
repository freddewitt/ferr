// settings.js — read/write app settings (Tauri store plugin or localStorage in dev)

const Settings = (() => {
    const STORE_PATH = 'ferr-settings.json';

    const DEFAULTS = {
        language:        'en',
        hashAlgorithm:   'xxhash',
        par2Enabled:     true,
        par2Percent:     10,
        videoMode:       false,
        renameTemplate:  '{camera}_{date}_{clip}',
        historyDedup:    false,
        ejectAfterCopy:  false,
        preserveMetadata: true,
        pdfReport:       true,
        notifications:   true,
        jsonManifest:    false,
    };

    let _cache = { ...DEFAULTS };

    async function load() {
        if (window.__TAURI__) {
            try {
                const store = await window.__TAURI__.store.load(STORE_PATH);
                // Charger toutes les entrées en un seul appel IPC au lieu de N appels
                // (tauri-plugin-store v2 expose store.entries())
                try {
                    const entries = await store.entries();
                    for (const [key, val] of entries) {
                        if (key in DEFAULTS && val !== null && val !== undefined) {
                            _cache[key] = val;
                        }
                    }
                } catch {
                    // Fallback : appels individuels (tauri-plugin-store v1 ou erreur)
                    for (const key of Object.keys(DEFAULTS)) {
                        const val = await store.get(key);
                        if (val !== null && val !== undefined) _cache[key] = val;
                    }
                }
            } catch {
                _loadFromLocalStorage();
            }
        } else {
            _loadFromLocalStorage();
        }
        return _cache;
    }

    function _loadFromLocalStorage() {
        const saved = localStorage.getItem('ferr-settings');
        if (saved) {
            try { Object.assign(_cache, JSON.parse(saved)); } catch {}
        }
    }

    async function set(key, value) {
        _cache[key] = value;
        if (window.__TAURI__) {
            try {
                const store = await window.__TAURI__.store.load(STORE_PATH);
                await store.set(key, value);
                await store.save();
            } catch {
                _saveToLocalStorage();
            }
        } else {
            _saveToLocalStorage();
        }
    }

    function _saveToLocalStorage() {
        localStorage.setItem('ferr-settings', JSON.stringify(_cache));
    }

    function get(key) {
        return _cache[key] ?? DEFAULTS[key];
    }

    function getAll() {
        return { ...DEFAULTS, ..._cache };
    }

    return { load, set, get, getAll, DEFAULTS };
})();
