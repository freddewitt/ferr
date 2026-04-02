// dropzone.js — FolderDropZone / FileDropZone component
//
// Usage:
//   const zone = new DropZone(containerEl, { label, hint, onPick, accept: 'folder'|'file', ext: [...] })
//   zone.getPath() → string|null
//   zone.setPath(p) → void
//   zone.reset()   → void

class DropZone {
    constructor(container, opts = {}) {
        this._container = container;
        this._opts = {
            label:    opts.label  ?? 'Click to select folder',
            hint:     opts.hint   ?? 'or drag and drop here',
            accept:   opts.accept ?? 'folder',   // 'folder' | 'file'
            ext:      opts.ext    ?? [],
            onPick:   opts.onPick ?? (() => {}),
            onClear:  opts.onClear ?? (() => {}),
        };
        this._path = null;
        this._render();
        this._bind();
    }

    _render() {
        this._container.innerHTML = '';
        const el = document.createElement('div');
        el.className = 'dropzone';
        el.innerHTML = `
            <div class="dropzone-icon">
                <svg viewBox="0 0 24 24"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/></svg>
            </div>
            <div class="dropzone-label">${this._opts.label}</div>
            <div class="dropzone-hint">${this._opts.hint}</div>
        `;
        if (this._opts.accept === 'any') {
            el.innerHTML += `
                <div style="margin-top: 12px; display: flex; gap: 8px; justify-content: center;">
                    <button class="btn" style="z-index: 2; position: relative;" id="dz-btn-file">${typeof t === 'function' ? t('select_file') : 'Select file'}</button>
                    <button class="btn" style="z-index: 2; position: relative;" id="dz-btn-folder">${typeof t === 'function' ? t('select_folder') : 'Select folder'}</button>
                </div>
            `;
        }
        this._el = el;
        this._container.appendChild(el);
    }

    _renderFilled(path) {
        const name = Fmt.basename(path);
        this._el.className = 'dropzone filled';
        this._el.innerHTML = `
            <div class="dropzone-row">
                <div class="dropzone-icon" style="opacity:0.7">
                    <svg viewBox="0 0 24 24"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/></svg>
                </div>
                <div style="flex:1;overflow:hidden">
                    <div class="dropzone-name">${name}</div>
                    <div class="dropzone-path" title="${path}">${path}</div>
                </div>
                <button class="dropzone-clear" title="Remove">✕</button>
            </div>
        `;
        this._el.querySelector('.dropzone-clear').addEventListener('click', e => {
            e.stopPropagation();
            this.reset();
            this._opts.onClear();
        });
    }

    _bind() {
        if (this._opts.accept === 'any' && this._path === null) {
            const bf = this._el.querySelector('#dz-btn-file');
            const bd = this._el.querySelector('#dz-btn-folder');
            if (bf) bf.addEventListener('click', async e => {
                e.stopPropagation();
                try {
                    const path = await Bridge.pickFile(this._opts.ext);
                    if (path) this.setPath(path);
                } catch (err) { console.error(err); }
            });
            if (bd) bd.addEventListener('click', async e => {
                e.stopPropagation();
                try {
                    const path = await Bridge.pickFolder();
                    if (path) this.setPath(path);
                } catch (err) { console.error(err); }
            });
        } else {
            this._el.addEventListener('click', () => this._pick());
        }

        this._el.addEventListener('dragover', e => {
            e.preventDefault();
            this._el.classList.add('drag-over');
        });

        this._el.addEventListener('dragleave', () => {
            this._el.classList.remove('drag-over');
        });

        this._el.addEventListener('drop', e => {
            e.preventDefault();
            this._el.classList.remove('drag-over');
            const items = e.dataTransfer?.items;
            if (items) {
                for (const item of items) {
                    const entry = item.webkitGetAsEntry?.();
                    if (entry) {
                        if ((this._opts.accept === 'folder' || this._opts.accept === 'any') && entry.isDirectory) {
                            this.setPath(entry.fullPath || e.dataTransfer.files[0]?.path || '');
                            return;
                        }
                        if ((this._opts.accept === 'file' || this._opts.accept === 'any') && entry.isFile) {
                            this.setPath(entry.fullPath || e.dataTransfer.files[0]?.path || '');
                            return;
                        }
                    }
                }
            }
            // Fallback: use file path directly (Tauri exposes this)
            const f = e.dataTransfer?.files?.[0];
            if (f?.path) this.setPath(f.path);
        });
    }

    async _pick() {
        try {
            let path;
            if (this._opts.accept === 'folder') {
                path = await Bridge.pickFolder();
            } else {
                path = await Bridge.pickFile(this._opts.ext);
            }
            if (path) this.setPath(path);
        } catch (err) {
            console.error('DropZone pick error:', err);
        }
    }

    setPath(path) {
        this._path = path;
        this._renderFilled(path);
        this._bind();
        this._opts.onPick(path);
    }

    getPath() { return this._path; }

    reset() {
        this._path = null;
        this._render();
        this._bind();
    }
}
