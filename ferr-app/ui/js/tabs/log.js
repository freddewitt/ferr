// log.js — Live console log tab
//
// Captures all ferr-progress, ferr-error and ferr-complete events
// and displays them with timestamps in a scrollable terminal view.
// Also intercepts console.error() from JS itself.

const LogTab = (() => {
    // ── In-memory ring buffer (max 2000 entries) ─────────────────────────────
    const MAX_ENTRIES = 2000;
    const _entries = [];     // { ts, level, text }
    let _badgeCount = 0;     // unread errors shown on sidebar badge
    let _isOpen = false;
    let _filter = 'all';     // 'all' | 'info' | 'error'

    // ── Entry levels ─────────────────────────────────────────────────────────
    const LEVEL = {
        INFO:     'info',
        ERROR:    'error',
        WARN:     'warn',
        SUCCESS:  'success',
        SYSTEM:   'system',
    };

    function _push(level, text) {
        const ts = new Date().toLocaleTimeString('en-GB', { hour12: false,
            hour: '2-digit', minute: '2-digit', second: '2-digit' });
        _entries.push({ ts, level, text });
        if (_entries.length > MAX_ENTRIES) _entries.shift();

        if (level === LEVEL.ERROR || level === LEVEL.WARN) {
            if (!_isOpen) {
                _badgeCount++;
                _updateBadge();
            }
        }
        _appendLine(ts, level, text);
    }

    function _classify(line) {
        if (!line || !line.trim()) return null;
        const l = line.toLowerCase();
        if (l.includes('error') || l.includes('failed') || l.includes('✗'))
            return LEVEL.ERROR;
        if (l.includes('warn') || l.includes('missing') || l.includes('corrupted'))
            return LEVEL.WARN;
        if (l.includes('✓') || l.includes('ok') || l.includes('complete') || l.includes('success'))
            return LEVEL.SUCCESS;
        if (l.startsWith('progress:') || l.startsWith('scan_progress:') ||
            l.startsWith('verify_result:') || l.startsWith('complete:') ||
            l.startsWith('scan_result:'))
            return LEVEL.SYSTEM;
        return LEVEL.INFO;
    }

    // ── Event wiring (called once at startup) ────────────────────────────────
    function init() {
        Bridge.onProgress(line => {
            const level = _classify(line) ?? LEVEL.SYSTEM;
            _push(level, '▶ ' + line);
        });

        Bridge.onError(line => {
            if (line && line.trim()) _push(LEVEL.ERROR, '✗ ' + line);
        });

        Bridge.onComplete(code => {
            const level = code === 0 ? LEVEL.SUCCESS : LEVEL.ERROR;
            const msg = code === 0
                ? `✓ Process finished (exit 0)`
                : `✗ Process finished with exit code ${code}`;
            _push(level, msg);
            _push(LEVEL.SYSTEM, '─'.repeat(52));
        });

        // Capture JS errors
        const _origError = console.error.bind(console);
        console.error = (...args) => {
            _push(LEVEL.ERROR, '[JS] ' + args.join(' '));
            _origError(...args);
        };
        const _origWarn = console.warn.bind(console);
        console.warn = (...args) => {
            _push(LEVEL.WARN, '[JS] ' + args.join(' '));
            _origWarn(...args);
        };

        _push(LEVEL.SYSTEM, '══ ferr log started ══');
    }

    // ── Tab render ───────────────────────────────────────────────────────────
    function render(container) {
        _isOpen = true;
        _badgeCount = 0;
        _updateBadge();

        container.innerHTML = `
            <div class="log-tab">
                <div class="log-toolbar">
                    <div class="log-filters">
                        <button class="log-filter-btn ${_filter === 'all'     ? 'active' : ''}" data-filter="all">All</button>
                        <button class="log-filter-btn ${_filter === 'info'    ? 'active' : ''}" data-filter="info">Info</button>
                        <button class="log-filter-btn ${_filter === 'error'   ? 'active' : ''}" data-filter="error">Errors</button>
                    </div>
                    <div style="display:flex;gap:6px;align-items:center;">
                        <label class="log-autoscroll-label">
                            <input type="checkbox" id="log-autoscroll" checked>
                            Auto-scroll
                        </label>
                        <button class="btn log-clear-btn" id="log-clear">Clear</button>
                    </div>
                </div>
                <div class="log-console" id="log-console"></div>
            </div>
        `;

        // Render all buffered entries
        const console_el = document.getElementById('log-console');
        const frag = document.createDocumentFragment();
        _entries
            .filter(e => _shouldShow(e.level))
            .forEach(e => frag.appendChild(_makeRow(e.ts, e.level, e.text)));
        console_el.appendChild(frag);
        _scrollToBottom();

        // Toolbar events
        container.querySelectorAll('.log-filter-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                _filter = btn.dataset.filter;
                container.querySelectorAll('.log-filter-btn')
                    .forEach(b => b.classList.toggle('active', b.dataset.filter === _filter));
                _rebuildView();
            });
        });

        document.getElementById('log-clear')?.addEventListener('click', () => {
            _entries.length = 0;
            document.getElementById('log-console').innerHTML = '';
            _push(LEVEL.SYSTEM, '── cleared ──');
        });
    }

    function _shouldShow(level) {
        if (_filter === 'all') return true;
        if (_filter === 'error') return level === LEVEL.ERROR || level === LEVEL.WARN;
        if (_filter === 'info') return level === LEVEL.INFO || level === LEVEL.SUCCESS || level === LEVEL.SYSTEM;
        return true;
    }

    function _rebuildView() {
        const el = document.getElementById('log-console');
        if (!el) return;
        el.innerHTML = '';
        const frag = document.createDocumentFragment();
        _entries.filter(e => _shouldShow(e.level)).forEach(e =>
            frag.appendChild(_makeRow(e.ts, e.level, e.text)));
        el.appendChild(frag);
        _scrollToBottom();
    }

    function _makeRow(ts, level, text) {
        const row = document.createElement('div');
        row.className = `log-row log-${level}`;
        row.innerHTML = `<span class="log-ts">${ts}</span><span class="log-text">${_esc(text)}</span>`;
        return row;
    }

    function _appendLine(ts, level, text) {
        const el = document.getElementById('log-console');
        if (!el) return;
        if (!_shouldShow(level)) return;
        el.appendChild(_makeRow(ts, level, text));
        const cb = document.getElementById('log-autoscroll');
        if (!cb || cb.checked) _scrollToBottom();
    }

    function _scrollToBottom() {
        const el = document.getElementById('log-console');
        if (el) el.scrollTop = el.scrollHeight;
    }

    function _esc(str) {
        return String(str)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;');
    }

    // ── Badge on sidebar icon ─────────────────────────────────────────────────
    function _updateBadge() {
        const badge = document.getElementById('log-badge');
        if (!badge) return;
        if (_badgeCount > 0) {
            badge.textContent = _badgeCount > 99 ? '99+' : String(_badgeCount);
            badge.style.display = 'flex';
        } else {
            badge.style.display = 'none';
        }
    }

    function onTabLeave() { _isOpen = false; }

    return { init, render, onTabLeave };
})();
