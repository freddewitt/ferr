// cert.js — Certificat tab: Create and Verify certificates

const CertTab = (() => {
    let _selected = 'create';
    let _cards    = [];

    const CARDS = [
        { id: 'create', titleKey: 'cert_create', descKey: 'cert_desc', icon: _certIcon() },
        { id: 'verify', titleKey: 'cert_verify_tab', descKey: 'verify_desc', icon: _verifyIcon() },
    ];

    function render(container) {
        container.innerHTML = `
            <div class="health-cards" id="cert-cards"></div>
            <div id="cert-panel"></div>
        `;

        const cardsEl = document.getElementById('cert-cards');
        _cards = CARDS.map(c => new HealthCard(cardsEl, {
            id:      c.id,
            icon:    c.icon,
            title:   t(c.titleKey),
            desc:    t(c.descKey),
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
        const panel = document.getElementById('cert-panel');
        if (!panel) return;
        if (id === 'create') _renderCreatePanel(panel);
        else _renderVerifyPanel(panel);
    }

    // ── Create ─────────────────────────────────────────────────────────────
    function _renderCreatePanel(panel) {
        panel.innerHTML = `
            <div class="health-panel">
                <div class="health-panel-title">${t('cert_create')}</div>
                <div id="cert-src-zone"></div>
            </div>
        `;

        const zone = new DropZone(document.getElementById('cert-src-zone'), {
            label: t('source_folder'),
            accept: 'folder'
        });

        App.setBottomBarAction(t('generate_cert'), async () => {
            const folder = zone.getPath();
            if (!folder) return App.flash(t('select_folder_first'));
            Progress.show(t('generate_cert') + '…');
            try {
                await Bridge.certCreate(folder);
            } finally {
                Progress.hide();
            }
        });
    }

    // ── Verify ─────────────────────────────────────────────────────────────
    function _renderVerifyPanel(panel) {
        panel.innerHTML = `
            <div class="health-panel">
                <div class="health-panel-title">${t('cert_verify_tab')}</div>
                <div id="cert-file-zone" style="margin-bottom:8px;"></div>
                <div id="cert-verify-folder-zone"></div>
            </div>
        `;

        const zones = {
            cert:   new DropZone(document.getElementById('cert-file-zone'),          { label: t('cert_file'),        accept: 'file', ext: ['ferrcert'] }),
            folder: new DropZone(document.getElementById('cert-verify-folder-zone'), { label: t('folder_to_verify'), accept: 'folder' })
        };

        App.setBottomBarAction(t('cert_verify_tab'), async () => {
            const cert   = zones.cert.getPath();
            const folder = zones.folder.getPath();
            if (!cert || !folder) return App.flash(t('select_folder_first'));
            Progress.show(t('verify') + '…');
            try {
                await Bridge.certVerify(cert, folder);
            } finally {
                Progress.hide();
            }
        });
    }

    function _certIcon() {
        return `<svg viewBox="0 0 24 24"><circle cx="12" cy="8" r="6"/><path d="M15.477 12.89 17 22l-5-3-5 3 1.523-9.11"/></svg>`;
    }

    function _verifyIcon() {
        return `<svg viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12"/></svg>`;
    }

    return { render };
})();
