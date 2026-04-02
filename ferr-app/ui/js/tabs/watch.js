// watch.js — Watch tab: monitor folder + one or more destinations

const WatchTab = (() => {
    let _folderZone  = null;
    let _destZones   = [];
    let _watching    = false;

    function render(container) {
        container.innerHTML = `
            <div class="copy-zones">
                <div class="copy-zone-row">
                    <div class="zone-header">
                        <span class="section-label">${t('watch_folder')}</span>
                    </div>
                    <div id="watch-folder-zone"></div>
                </div>

                <div class="copy-zone-row" style="margin-top:8px;">
                    <div class="zone-header">
                        <span class="section-label">${t('destinations')}</span>
                    </div>
                    <div id="watch-dest-list" class="mirror-list"></div>
                    <div style="margin-top:4px;">
                        <button class="link-action" id="watch-add-dest">${t('add_destination')}</button>
                    </div>
                </div>

                <p class="watch-note">${t('watch_note')}</p>

                <div id="watch-status-bar" class="hidden"></div>
            </div>
        `;

        _folderZone = new DropZone(
            document.getElementById('watch-folder-zone'),
            { label: t('watch_folder'), hint: t('watch_folder_hint'), accept: 'folder' }
        );

        _destZones = [];
        _addDest(); // start with one destination slot

        document.getElementById('watch-add-dest').addEventListener('click', () => _addDest());

        // Restore watching pill if active
        if (_watching) _showWatchStatus(true);
    }

    function _addDest() {
        const list = document.getElementById('watch-dest-list');
        const idx  = _destZones.length;
        const wrap = document.createElement('div');
        wrap.className = 'mirror-item';
        wrap.innerHTML = `
            <div id="watch-dest-zone-${idx}"></div>
            <button class="mirror-remove" title="Remove destination">
                <svg viewBox="0 0 24 24"><line x1="5" y1="12" x2="19" y2="12"/></svg>
            </button>
        `;
        list.appendChild(wrap);

        const zone = new DropZone(
            document.getElementById(`watch-dest-zone-${idx}`),
            { label: t('dest_label', { n: idx + 1 }), hint: t('dest_hint'), accept: 'folder' }
        );

        const entry = { zone, container: wrap };
        _destZones.push(entry);

        wrap.querySelector('.mirror-remove').addEventListener('click', () => {
            if (_destZones.length <= 1) return; // keep at least one
            const i = _destZones.indexOf(entry);
            if (i !== -1) _destZones.splice(i, 1);
            wrap.remove();
        });
    }

    function _showWatchStatus(active) {
        const bar = document.getElementById('watch-status-bar');
        if (!bar) return;
        if (active) {
            bar.className = 'watch-status';
            bar.innerHTML = `<div class="watch-status-dot"></div> ${t('watching_status')}`;
        } else {
            bar.className = 'hidden';
            bar.innerHTML = '';
        }
    }

    function setWatching(active) {
        _watching = active;
        _showWatchStatus(active);
    }

    function getFolder()       { return _folderZone?.getPath() ?? null; }
    function getDestinations() {
        return _destZones.map(e => e.zone.getPath()).filter(Boolean);
    }
    function isReady()         { return !!getFolder() && getDestinations().length > 0; }
    function isWatching()      { return _watching; }

    return { render, setWatching, getFolder, getDestinations, isReady, isWatching };
})();
