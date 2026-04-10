// modal.js — Generic result modal component

const Modal = (() => {
    let _overlay = null;

    /** Échappe les caractères HTML pour prévenir les injections XSS. */
    function _esc(s) {
        if (s == null) return '';
        return String(s)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    }

    function init() {
        if (_overlay) return;
        _overlay = document.createElement('div');
        _overlay.id = 'modal-overlay';
        _overlay.className = 'modal-overlay';
        document.body.appendChild(_overlay);

        _overlay.addEventListener('click', (e) => {
            if (e.target === _overlay) hide();
        });
    }

    /**
     * @param {Object} opts
     * @param {string} opts.title
     * @param {string} opts.icon - 'check' | 'alert'
     * @param {string|number} opts.files
     * @param {string|number} opts.bytes
     * @param {string|number} opts.errors
     * @param {string} opts.msg
     * @param {boolean} opts.showManifestBtn
     * @param {string} opts.manifestPath
     * @param {Function} opts.onClose
     */
    function show(opts = {}) {
        init();
        const {
            title = t('operation_result'),
            icon = 'check',
            files = '-',
            bytes = '-',
            errors = '-',
            msg = '',
            showManifestBtn = false,
            manifestPath = null,
            onClose = null
        } = opts;

        _overlay.innerHTML = `
            <div class="modal">
                <div class="modal-header">
                    <div class="modal-icon">${_getIcon(icon)}</div>
                    <div class="modal-title">${_esc(title)}</div>
                </div>
                <div class="modal-body">
                    <div class="modal-summary">
                        <div class="summary-item">
                            <div class="summary-label">${t('files')}</div>
                            <div class="summary-value" id="modal-val-files">${_esc(files)}</div>
                        </div>
                        <div class="summary-item">
                            <div class="summary-label">${t('bytes')}</div>
                            <div class="summary-value" id="modal-val-bytes">${_esc(bytes)}</div>
                        </div>
                        <div class="summary-item">
                            <div class="summary-label">${t('errors')}</div>
                            <div class="summary-value" id="modal-val-errors" style="color:${errors > 0 ? 'var(--danger)' : 'inherit'}">${_esc(errors)}</div>
                        </div>
                    </div>
                    ${msg ? `<div class="modal-msg">${_esc(msg)}</div>` : ''}
                    ${opts.issues && opts.issues.length > 0 ? `
                        <div class="modal-issues">
                            ${opts.issues.map(i => `
                                <div class="issue-item severity-${_esc(i.severity.toLowerCase())}">
                                    <div class="issue-meta">
                                        <span class="issue-badge">${_esc(i.severity)}</span>
                                        <span class="issue-kind">${_esc(i.kind)}</span>
                                    </div>
                                    <div class="issue-path">${_esc(i.path)}</div>
                                    <div class="issue-detail">${_esc(i.detail)}</div>
                                </div>
                            `).join('')}
                        </div>
                    ` : ''}
                </div>
                <div class="modal-footer">
                    <button class="btn" id="modal-close-btn">${t('close')}</button>
                    ${showManifestBtn ? `<button class="btn btn-primary" id="modal-manifest-btn">${t('show_manifest')}</button>` : ''}
                </div>
            </div>
        `;

        _overlay.classList.add('active');

        document.getElementById('modal-close-btn').addEventListener('click', () => {
            hide();
            onClose?.();
        });

        if (showManifestBtn && manifestPath) {
            document.getElementById('modal-manifest-btn').addEventListener('click', () => {
                if (window.__TAURI__) {
                    window.__TAURI__.core.invoke('open_path', { path: manifestPath });
                }
            });
        }
    }

    function hide() {
        if (_overlay) _overlay.classList.remove('active');
    }

    function _getIcon(name) {
        if (name === 'check') return `<svg viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12"/></svg>`;
        if (name === 'alert') return `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>`;
        return '';
    }

    return { show, hide };
})();
