// progress.js — progress overlay + machine-readable line parser

const Progress = (() => {
    const overlay  = document.getElementById('progress-overlay');
    let _onCancel  = null;

    function show(title, opts = {}) {
        _onCancel = opts.onCancel ?? null;
        overlay.innerHTML = `
            <div class="progress-title">${title}</div>
            <div class="progress-filename" id="prog-filename">${t('preparing')}</div>
            <div class="progress-bar-track">
                <div class="progress-bar-fill" id="prog-bar" style="width:0%"></div>
            </div>
            <div class="progress-stats">
                <span id="prog-files">—</span>
                <span id="prog-bytes">—</span>
                <span id="prog-speed">—</span>
            </div>
            ${_onCancel ? `<button class="btn progress-cancel" id="prog-cancel">${t('cancel_btn')}</button>` : ''}
        `;
        if (_onCancel) {
            overlay.querySelector('#prog-cancel').addEventListener('click', () => {
                _onCancel?.();
                hide();
            });
        }
        overlay.classList.remove('hidden');
    }

    function hide() {
        overlay.classList.add('hidden');
        overlay.innerHTML = '';
        _onCancel = null;
        _buffer = '';
    }

    let _buffer = '';

    function update(chunk) {
        _buffer += chunk;
        const lines = _buffer.split('\n');
        _buffer = lines.pop() || '';

        for (const line of lines) {
            const ev = parseLine(line.trim());
            if (!ev) continue;

            if (ev.type === 'PROGRESS') {
            const pct = ev.totalBytes ? (ev.bytes / ev.totalBytes * 100).toFixed(1) : 0;
            _set('prog-bar', el => el.style.width = pct + '%');
            _set('prog-filename', el => el.textContent = ev.filename);
            _set('prog-files', el => el.textContent = `${ev.files}/${ev.totalFiles} files`);
            _set('prog-bytes', el => el.textContent = Fmt.bytes(ev.bytes));
            _set('prog-speed', el => el.textContent = ev.speed);
        }

        if (ev.type === 'SCAN_PROGRESS') {
            const pct = ev.total ? (ev.scanned / ev.total * 100).toFixed(1) : 0;
            _set('prog-bar', el => el.style.width = pct + '%');
            _set('prog-filename', el => el.textContent = ev.file);
            _set('prog-files', el => el.textContent = `${ev.scanned}/${ev.total} files`);
        }

            if (ev.type === 'COMPLETE') {
                _set('prog-bar', el => el.style.width = '100%');
                _set('prog-filename', el => el.textContent = typeof t === 'function' ? t('complete') : 'Complete');
            }
        }
    }

    function _set(id, fn) {
        const el = document.getElementById(id);
        if (el) fn(el);
    }

    // ── Machine-readable line parser ───────────────────────────────────────
    function parseLine(line) {
        if (!line) return null;

        if (line.startsWith('PROGRESS:')) {
            const rest = line.slice(9);
            const parts = rest.split('|');
            if (parts.length < 4) return null;
            const [bytes, totalBytes] = parts[0].split('/').map(Number);
            const [files, totalFiles] = parts[1].split('/').map(Number);
            return { type: 'PROGRESS', bytes, totalBytes, files, totalFiles, speed: parts[2], filename: parts[3] };
        }

        if (line.startsWith('COMPLETE:')) {
            const p = line.slice(9).split('|');
            return { type: 'COMPLETE', files: +p[0], bytes: +p[1], errors: +p[2], manifestPath: p[3] };
        }

        if (line.startsWith('ERROR:')) {
            const p = line.slice(6).split('|');
            return { type: 'ERROR', filename: p[0], kind: p[1], details: p[2] };
        }

        if (line.startsWith('SCAN_PROGRESS:')) {
            const [counts, file] = line.slice(14).split('|');
            const [scanned, total] = counts.split('/').map(Number);
            return { type: 'SCAN_PROGRESS', scanned, total, file };
        }

        if (line.startsWith('SCAN_RESULT:')) {
            const p = line.slice(12).split('|');
            return { type: 'SCAN_RESULT', ok: p[0] === 'OK', checked: +p[1], corrupted: +p[2] };
        }

        if (line.startsWith('VERIFY_RESULT:')) {
            const p = line.slice(14).split('|');
            return { type: 'VERIFY_RESULT', ok: p[0] === 'OK', matched: +p[1], mismatched: +p[2], missing: +p[3] };
        }

        if (line.startsWith('WATCH_DETECTED:')) {
            const [volume, mount] = line.slice(15).split('|');
            return { type: 'WATCH_DETECTED', volume, mount };
        }

        if (line.startsWith('WATCH_COPY_START:')) {
            const [volume, dest] = line.slice(17).split('|');
            return { type: 'WATCH_COPY_START', volume, dest };
        }

        return { type: 'UNKNOWN', raw: line };
    }

    return { show, hide, update, parseLine };
})();
