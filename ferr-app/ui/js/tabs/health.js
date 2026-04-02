// health.js — Health tab: 4 selectable action cards + detail panels

const HealthTab = (() => {
    let _selected = 'scan';
    let _cards    = [];

    const CARDS = [
        { id: 'scan',   title: t('scan'),         desc: t('scan_desc'),         icon: _scanIcon() },
        { id: 'verify', title: t('verify'),       desc: t('verify_desc'),       icon: _verifyIcon() },
        { id: 'repair', title: t('repair'),       desc: t('repair_desc'),       icon: _repairIcon() },
        { id: 'cert',   title: t('cert'),         desc: t('cert_desc'),         icon: _certIcon() },
    ];

    function render(container) {
        container.innerHTML = `
            <div class="health-cards" id="health-cards"></div>
            <div id="health-panel"></div>
        `;

        const cardsEl = document.getElementById('health-cards');
        _cards = CARDS.map(c => new HealthCard(cardsEl, {
            id:      c.id,
            icon:    c.icon,
            title:   c.title,
            desc:    c.desc,
            onClick: id => _selectCard(id),
        }));

        _selectCard(_selected);
    }

    function _selectCard(id) {
        _selected = id;
        _cards.forEach(c => c.setSelected(c._id === id));
        _renderPanel(id);
    }

    function _renderPanel(id) {
        const panel = document.getElementById('health-panel');
        if (!panel) return;
        switch (id) {
            case 'scan':   _renderScanPanel(panel);   break;
            case 'verify': _renderVerifyPanel(panel); break;
            case 'repair': _renderRepairPanel(panel); break;
            case 'cert':   _renderCertPanel(panel);   break;
        }
    }

    // ── Scan ───────────────────────────────────────────────────────────────
    function _renderScanPanel(panel) {
        let mode = 'full'; // 'full' | 'since'
        panel.innerHTML = `
            <div class="health-panel">
                <div class="health-panel-title">${t('scan')}</div>
                <div id="scan-folder-zone" style="margin-bottom:12px;"></div>
                <div class="health-mode-tabs">
                    <button class="health-mode-tab active" data-mode="full">${t('full_scan')}</button>
                    <button class="health-mode-tab" data-mode="since">${t('since_date')}</button>
                </div>
                <div id="scan-since-row" class="hidden" style="margin-bottom:12px;">
                    <input type="date" id="scan-since-date" style="
                        border:1px solid var(--border); border-radius:6px; padding:6px 10px;
                        font-size:13px; background:var(--bg-secondary); color:var(--text-primary);
                        font-family:var(--font); cursor:pointer;
                    ">
                </div>
                <div id="scan-result" class="hidden"></div>
            </div>
        `;

        const folderZone = new DropZone(
            document.getElementById('scan-folder-zone'),
            { label: t('select_scan_folder'), hint: '', accept: 'folder' }
        );

        panel.querySelectorAll('.health-mode-tab').forEach(btn => {
            btn.addEventListener('click', () => {
                panel.querySelectorAll('.health-mode-tab').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                mode = btn.dataset.mode;
                document.getElementById('scan-since-row').classList.toggle('hidden', mode !== 'since');
            });
        });

        App.setBottomBarAction(t('run_scan'), async () => {
            const folder = folderZone.getPath();
            if (!folder) return App.flash(t('select_folder_first'));
            const since = mode === 'since' ? document.getElementById('scan-since-date').value : null;
            Progress.show(t('scanning'));
            try {
                await Bridge.runScan(folder, since);
            } finally {
                Progress.hide();
            }
        });
    }

    // ── Verify ─────────────────────────────────────────────────────────────
    function _renderVerifyPanel(panel) {
        let mode = 'folders';
        panel.innerHTML = `
            <div class="health-panel">
                <div class="health-panel-title">${t('verify')}</div>
                <div class="health-mode-tabs">
                    <button class="health-mode-tab active" data-mode="folders">${t('two_folders')}</button>
                    <button class="health-mode-tab" data-mode="manifest">${t('json_manifest')}</button>
                    <button class="health-mode-tab" data-mode="cert">${t('certificate')}</button>
                </div>
                <div id="verify-zones"></div>
                <div id="verify-result" class="hidden"></div>
            </div>
        `;

        const renderZones = () => {
            const el = document.getElementById('verify-zones');
            if (mode === 'folders') {
                el.innerHTML = '<div id="verify-src-zone" style="margin-bottom:8px;"></div><div id="verify-dest-zone"></div>';
                new DropZone(document.getElementById('verify-src-zone'), { label: t('source_folder'), accept: 'folder' });
                new DropZone(document.getElementById('verify-dest-zone'), { label: t('dest_folder'), accept: 'folder' });
            } else if (mode === 'manifest') {
                el.innerHTML = '<div id="verify-manifest-zone" style="margin-bottom:8px;"></div><div id="verify-dest2-zone"></div>';
                new DropZone(document.getElementById('verify-manifest-zone'), { label: t('json_manifest'), accept: 'file', ext: ['json'] });
                new DropZone(document.getElementById('verify-dest2-zone'), { label: t('dest_folder'), accept: 'folder' });
            } else {
                el.innerHTML = '<div id="verify-cert-zone" style="margin-bottom:8px;"></div><div id="verify-cert-folder-zone"></div>';
                new DropZone(document.getElementById('verify-cert-zone'), { label: t('cert_file'), accept: 'file', ext: ['ferrcert'] });
                new DropZone(document.getElementById('verify-cert-folder-zone'), { label: t('folder_to_verify'), accept: 'folder' });
            }
        };

        renderZones();

        panel.querySelectorAll('.health-mode-tab').forEach(btn => {
            btn.addEventListener('click', () => {
                panel.querySelectorAll('.health-mode-tab').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                mode = btn.dataset.mode;
                renderZones();
            });
        });
    }

    // ── Repair ─────────────────────────────────────────────────────────────
    function _renderRepairPanel(panel) {
        panel.innerHTML = `
            <div class="health-panel">
                <div class="health-panel-title">${t('repair')}</div>
                <div class="health-warning">
                    ${t('repair_warning')}
                </div>
                <div id="repair-folder-zone" style="margin-bottom:12px;"></div>
            </div>
        `;

        const zone = new DropZone(
            document.getElementById('repair-folder-zone'),
            { label: t('select_repair_folder'), accept: 'folder' }
        );

        App.setBottomBarAction(t('repair_btn'), async () => {
            const folder = zone.getPath();
            if (!folder) return App.flash(t('select_folder_first'));
            if (!confirm(t('repair_confirm'))) return;
            Progress.show(t('repairing'));
            try {
                await Bridge.runRepair(folder);
            } finally {
                Progress.hide();
            }
        }, 'danger');
    }

    // ── Certificate ────────────────────────────────────────────────────────
    function _renderCertPanel(panel) {
        let mode = 'create';
        panel.innerHTML = `
            <div class="health-panel">
                <div class="health-panel-title">${t('certificate')}</div>
                <div class="health-mode-tabs">
                    <button class="health-mode-tab active" data-mode="create">${t('cert_create')}</button>
                    <button class="health-mode-tab" data-mode="verify">${t('cert_verify_tab')}</button>
                </div>
                <div id="cert-zones"></div>
            </div>
        `;

        const renderZones = () => {
            const el = document.getElementById('cert-zones');
            if (mode === 'create') {
                el.innerHTML = '<div id="cert-src-zone"></div>';
                new DropZone(document.getElementById('cert-src-zone'), { label: t('source_folder'), accept: 'folder' });
            } else {
                el.innerHTML = '<div id="cert-file-zone" style="margin-bottom:8px;"></div><div id="cert-verify-folder-zone"></div>';
                new DropZone(document.getElementById('cert-file-zone'), { label: t('cert_file'), accept: 'file', ext: ['ferrcert'] });
                new DropZone(document.getElementById('cert-verify-folder-zone'), { label: t('folder_to_verify'), accept: 'folder' });
            }
        };

        renderZones();

        panel.querySelectorAll('.health-mode-tab').forEach(btn => {
            btn.addEventListener('click', () => {
                panel.querySelectorAll('.health-mode-tab').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                mode = btn.dataset.mode;
                renderZones();
            });
        });
    }

    // ── Icons ──────────────────────────────────────────────────────────────
    function _scanIcon() {
        return `<svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="11" y1="8" x2="11" y2="11"/><line x1="11" y1="14" x2="11.01" y2="14"/></svg>`;
    }
    function _verifyIcon() {
        return `<svg viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12"/></svg>`;
    }
    function _repairIcon() {
        return `<svg viewBox="0 0 24 24"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/></svg>`;
    }
    function _certIcon() {
        return `<svg viewBox="0 0 24 24"><circle cx="12" cy="8" r="6"/><path d="M15.477 12.89 17 22l-5-3-5 3 1.523-9.11"/></svg>`;
    }

    return { render };
})();
