// copy.js — Copy tab: source + primary destination + up to 2 mirrors

const CopyTab = (() => {
    let _sourceZone   = null;
    let _destZone     = null;
    let _mirrorZones  = [];   // [{zone, container}]

    function render(container) {
        container.innerHTML = `
            <div class="copy-zones">
                <div class="copy-zone-row">
                    <div class="zone-header">
                        <span class="section-label">${t('source')}</span>
                    </div>
                    <div id="copy-source-zone"></div>
                </div>

                <div class="copy-zone-row">
                    <div class="zone-header">
                        <span class="section-label">${t('destination')}</span>
                    </div>
                    <div id="copy-dest-zone"></div>
                </div>

                <div id="copy-mirror-list" class="mirror-list"></div>

                <div style="margin-top:4px;">
                    <button class="link-action" id="copy-add-mirror">${t('add_mirror')}</button>
                </div>
            </div>
        `;

        _sourceZone = new DropZone(
            document.getElementById('copy-source-zone'),
            { label: t('select_source'), hint: t('select_source_hint'), accept: 'any',
              onPick: () => App.updateBottomBar() }
        );

        _destZone = new DropZone(
            document.getElementById('copy-dest-zone'),
            { label: t('select_dest'), hint: t('select_dest_hint'), accept: 'folder',
              onPick: () => App.updateBottomBar() }
        );

        _mirrorZones = [];

        document.getElementById('copy-add-mirror').addEventListener('click', () => {
            if (_mirrorZones.length >= 2) return;
            _addMirror();
        });
    }

    function _addMirror() {
        const list = document.getElementById('copy-mirror-list');
        const idx  = _mirrorZones.length;
        const wrap = document.createElement('div');
        wrap.className = 'mirror-item';
        wrap.innerHTML = `
            <div class="mirror-dropzone" id="copy-mirror-zone-${idx}"></div>
            <button class="mirror-remove" title="Remove mirror">
                <svg viewBox="0 0 24 24"><line x1="5" y1="12" x2="19" y2="12"/></svg>
            </button>
        `;
        list.appendChild(wrap);

        const zone = new DropZone(
            document.getElementById(`copy-mirror-zone-${idx}`),
            { label: t('mirror_label', { n: idx + 1 }), hint: t('mirror_hint'), accept: 'folder' }
        );

        const entry = { zone, container: wrap };
        _mirrorZones.push(entry);

        wrap.querySelector('.mirror-remove').addEventListener('click', () => {
            _removeMirror(entry);
        });

        // Hide "add mirror" if we now have 2
        if (_mirrorZones.length >= 2) {
            document.getElementById('copy-add-mirror').classList.add('hidden');
        }
    }

    function _removeMirror(entry) {
        const idx = _mirrorZones.indexOf(entry);
        if (idx === -1) return;
        _mirrorZones.splice(idx, 1);
        entry.container.remove();
        document.getElementById('copy-add-mirror').classList.remove('hidden');
    }

    function getSource()       { return _sourceZone?.getPath() ?? null; }
    function getDestinations() {
        const d = [];
        const p = _destZone?.getPath();
        if (p) d.push(p);
        for (const { zone } of _mirrorZones) {
            const mp = zone.getPath();
            if (mp) d.push(mp);
        }
        return d;
    }

    function isReady() {
        return !!getSource() && getDestinations().length > 0;
    }

    return { render, getSource, getDestinations, isReady };
})();
