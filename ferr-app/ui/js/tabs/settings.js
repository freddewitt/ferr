// settings.js — Settings tab: toggles, hash, PAR2 slider, video mode, profiles, language

const SettingsTab = (() => {
    function render(container) {
        const s = Settings.getAll();
        container.innerHTML = `
            <!-- General -->
            <div class="settings-section">
                <div class="settings-section-title">${t('general')}</div>
                ${_toggle('ejectAfterCopy', t('eject_after_copy'), t('eject_desc'), s.ejectAfterCopy)}
                ${_toggle('preserveMetadata', t('preserve_meta'), t('preserve_desc'), s.preserveMetadata)}
                ${_toggle('notifications', t('notifications'), t('notif_desc'), s.notifications)}
            </div>

            <!-- Hash -->
            <div class="settings-section">
                <div class="settings-section-title">${t('hash_algorithm')}</div>
                <div class="settings-row">
                    <div class="settings-row-label">
                        <div>${t('algorithm')}</div>
                        <div class="settings-row-sub">${t('algo_desc')}</div>
                    </div>
                    <div class="segmented" id="hash-segmented">
                        <button class="segmented-item ${s.hashAlgorithm === 'xxhash' ? 'active' : ''}" data-val="xxhash">XXH64</button>
                        <button class="segmented-item ${s.hashAlgorithm === 'sha256' ? 'active' : ''}" data-val="sha256">SHA-256</button>
                    </div>
                </div>
            </div>

            <!-- PAR2 -->
            <div class="settings-section">
                <div class="settings-section-title">${t('par2')}</div>
                ${_toggle('par2Enabled', t('par2_enable'), t('par2_desc'), s.par2Enabled)}
                <div class="settings-row" id="par2-slider-row" ${!s.par2Enabled ? 'style="opacity:0.4;pointer-events:none;"' : ''}>
                    <div class="settings-row-label">${t('recovery_percent')}</div>
                    <div class="slider-row" style="width:180px;">
                        <input type="range" id="par2-slider" min="1" max="30" value="${s.par2Percent}" step="1">
                        <span class="slider-value" id="par2-value">${s.par2Percent}%</span>
                    </div>
                </div>
            </div>

            <!-- Video mode -->
            <div class="settings-section">
                <div class="settings-section-title">${t('video_mode')}</div>
                ${_toggle('videoMode', t('video_enable'), t('video_desc'), s.videoMode)}
                <div id="video-conditional" class="settings-conditional ${!s.videoMode ? 'hidden' : ''}">
                    <div class="settings-row">
                        <div class="settings-row-label">
                            <div>${t('rename_template')}</div>
                            <div class="settings-row-sub">{camera} {date} {clip} {reel}</div>
                        </div>
                        <input type="text" id="rename-template" value="${s.renameTemplate}" style="
                            border:1px solid var(--border); border-radius:6px; padding:5px 8px;
                            font-size:12px; font-family:var(--font-mono); background:var(--bg-secondary);
                            color:var(--text-primary); width:180px;
                        ">
                    </div>
                    ${_toggle('historyDedup', t('dedup'), t('dedup_desc'), s.historyDedup)}
                </div>
            </div>

            <!-- Reports -->
            <div class="settings-section">
                <div class="settings-section-title">${t('reports')}</div>
                ${_toggle('pdfReport', t('pdf_report'), t('pdf_desc'), s.pdfReport)}
            </div>

            <!-- Profiles -->
            <div class="settings-section">
                <div class="settings-section-title">${t('profiles')}</div>
                <div id="profile-list" class="profile-list"></div>
                <div style="display:flex;gap:8px;align-items:center;">
                    <input type="text" id="profile-name" placeholder="${t('profile_name')}" style="
                        flex:1; border:1px solid var(--border); border-radius:6px;
                        padding:6px 10px; font-size:13px; background:var(--bg-secondary);
                        color:var(--text-primary); font-family:var(--font);
                    ">
                    <button class="btn" id="profile-save">${t('save_profile')}</button>
                </div>
            </div>

            <!-- Language -->
            <div class="settings-section">
                <div class="settings-section-title">${t('language')}</div>
                <div class="settings-row">
                    <div class="settings-row-label">Interface language</div>
                    <div class="segmented" id="lang-segmented">
                        <button class="segmented-item ${s.language === 'en' ? 'active' : ''}" data-val="en">English</button>
                        <button class="segmented-item ${s.language === 'fr' ? 'active' : ''}" data-val="fr">Français</button>
                    </div>
                </div>
            </div>
        `;

        _bindEvents();
        _loadProfiles();
    }

    function _toggle(key, label, desc, checked) {
        return `
            <div class="settings-row">
                <div class="settings-row-label">
                    <div>${label}</div>
                    <div class="settings-row-sub">${desc}</div>
                </div>
                <label class="toggle-switch">
                    <input type="checkbox" data-key="${key}" ${checked ? 'checked' : ''}>
                    <span class="toggle-track"></span>
                </label>
            </div>
        `;
    }

    function _bindEvents() {
        // Checkboxes
        document.querySelectorAll('.toggle-switch input[type=checkbox]').forEach(cb => {
            cb.addEventListener('change', async () => {
                await Settings.set(cb.dataset.key, cb.checked);
                App.refreshPills();
                // Conditional reveals
                if (cb.dataset.key === 'par2Enabled') {
                    const row = document.getElementById('par2-slider-row');
                    if (row) { row.style.opacity = cb.checked ? '1' : '0.4'; row.style.pointerEvents = cb.checked ? '' : 'none'; }
                }
                if (cb.dataset.key === 'videoMode') {
                    document.getElementById('video-conditional')?.classList.toggle('hidden', !cb.checked);
                }
            });
        });

        // Hash segmented
        document.getElementById('hash-segmented')?.querySelectorAll('.segmented-item').forEach(btn => {
            btn.addEventListener('click', async () => {
                document.getElementById('hash-segmented').querySelectorAll('.segmented-item').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                await Settings.set('hashAlgorithm', btn.dataset.val);
                App.refreshPills();
            });
        });

        // PAR2 slider
        document.getElementById('par2-slider')?.addEventListener('input', async e => {
            const v = +e.target.value;
            document.getElementById('par2-value').textContent = v + '%';
            await Settings.set('par2Percent', v);
            App.refreshPills();
        });

        // Rename template
        document.getElementById('rename-template')?.addEventListener('change', async e => {
            await Settings.set('renameTemplate', e.target.value);
        });

        // Language
        document.getElementById('lang-segmented')?.querySelectorAll('.segmented-item').forEach(btn => {
            btn.addEventListener('click', async () => {
                document.getElementById('lang-segmented').querySelectorAll('.segmented-item').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                await Settings.set('language', btn.dataset.val);
                await I18n.load(btn.dataset.val);
            });
        });

        document.getElementById('profile-save')?.addEventListener('click', async () => {
            const name = document.getElementById('profile-name').value.trim();
            if (!name) return App.flash(t('enter_profile_name'));
            await Bridge.saveProfile(name);
            document.getElementById('profile-name').value = '';
            _loadProfiles();
        });
    }

    async function _loadProfiles() {
        const listEl = document.getElementById('profile-list');
        if (!listEl) return;
        try {
            const profiles = await Bridge.getProfiles();
            if (!profiles.length) {
                listEl.innerHTML = `<div style="font-size:12px;color:var(--text-secondary);padding:4px 0;">${t('no_profiles')}</div>`;
                return;
            }
            listEl.innerHTML = profiles.map(p => `
                <div class="profile-item">
                    <span>${p.name}</span>
                    <div class="profile-item-actions">
                        <button class="btn" style="height:26px;padding:0 10px;font-size:11px;" data-profile="${p.name}">${t('load')}</button>
                    </div>
                </div>
            `).join('');
        } catch {
            listEl.innerHTML = '';
        }
    }

    return { render };
})();
