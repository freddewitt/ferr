// history.js — History tab: auto-loaded sessions, search, context menus

const HistoryTab = (() => {
    let _sessions = [];
    let _menuEl   = null;

    function render(container) {
        container.innerHTML = `
            <div class="history-search">
                <svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                <input type="text" id="history-search" placeholder="${t('search')}" autocomplete="off">
            </div>
            <div id="history-list" class="history-list">
                <div class="history-empty">${t('loading')}</div>
            </div>
        `;

        document.getElementById('history-search').addEventListener('input', e => {
            _search(e.target.value.trim());
        });

        _load();
        document.addEventListener('click', _closeMenu);
    }

    async function _load() {
        try {
            _sessions = await Bridge.getHistory();
            _render(_sessions);
        } catch {
            document.getElementById('history-list').innerHTML =
                `<div class="history-empty">${t('history_error')}</div>`;
        }
    }

    async function _search(q) {
        if (!q) { _render(_sessions); return; }
        try {
            const results = await Bridge.searchHistory(q);
            _render(results);
        } catch {
            _render(_sessions.filter(s =>
                JSON.stringify(s).toLowerCase().includes(q.toLowerCase())
            ));
        }
    }

    function _render(sessions) {
        const list = document.getElementById('history-list');
        if (!list) return;
        if (!sessions.length) {
            list.innerHTML = `<div class="history-empty">${t('no_sessions')}</div>`;
            return;
        }
        list.innerHTML = sessions.map(s => `
            <div class="history-row" data-id="${s.id}" data-manifest="${s.manifest ?? ''}">
                <div class="history-row-date">${Fmt.dateShort(s.date)}</div>
                <div class="history-row-src">${Fmt.basename(s.source)}</div>
                <div class="history-row-stats">${s.files} files · ${Fmt.bytes(s.bytes)}</div>
                ${s.errors > 0 ? `<span class="pill warning">${s.errors} err</span>` : ''}
            </div>
        `).join('');

        list.querySelectorAll('.history-row').forEach(row => {
            row.addEventListener('contextmenu', e => {
                e.preventDefault();
                _showMenu(e, row.dataset.manifest, row.dataset.id);
            });
        });
    }

    function _showMenu(e, manifest, id) {
        _closeMenu();
        if (!manifest) return;

        _menuEl = document.createElement('div');
        _menuEl.className = 'context-menu';
        _menuEl.style.left = e.clientX + 'px';
        _menuEl.style.top  = e.clientY + 'px';
        _menuEl.innerHTML = `
            <button class="context-menu-item" data-action="ale">${t('export_ale')}</button>
            <button class="context-menu-item" data-action="csv">${t('export_csv')}</button>
            <button class="context-menu-item" data-action="pdf">${t('generate_pdf')}</button>
            <button class="context-menu-item" data-action="verify">${t('verify_again')}</button>
        `;
        document.body.appendChild(_menuEl);

        _menuEl.querySelectorAll('.context-menu-item').forEach(item => {
            item.addEventListener('click', () => {
                _handleAction(item.dataset.action, manifest);
                _closeMenu();
            });
        });
    }

    async function _handleAction(action, manifest) {
        if (action === 'ale') {
            const out = await Bridge.pickSaveLocation('export.ale');
            if (out) await Bridge.exportALE(manifest, out);
        } else if (action === 'csv') {
            const out = await Bridge.pickSaveLocation('export.csv');
            if (out) await Bridge.exportCSV(manifest, out);
        } else if (action === 'pdf') {
            const out = await Bridge.pickSaveLocation('report.pdf');
            if (out) await Bridge.generateReport(manifest, out);
        } else if (action === 'verify') {
            App.switchTab('health');
        }
    }

    function _closeMenu() {
        _menuEl?.remove();
        _menuEl = null;
    }

    return { render };
})();
