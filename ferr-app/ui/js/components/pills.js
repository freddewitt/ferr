// pills.js — status pills rendered in the topbar

const Pills = (() => {
    function render(pills) {
        return pills.map(p => `
            <span class="pill ${p.cls ?? ''}">
                ${p.dot ? '<span class="pill-dot"></span>' : ''}
                ${p.label}
            </span>
        `).join('');
    }

    function hashPill(algo) {
        return { label: algo === 'sha256' ? 'SHA-256' : 'XXH64', cls: '' };
    }

    function par2Pill(enabled, pct) {
        if (!enabled) return null;
        return { label: `PAR2 ${pct}%`, cls: 'active' };
    }

    function watchPill(active) {
        if (!active) return null;
        return { label: 'Watching', cls: 'active', dot: true };
    }

    function videoPill(enabled) {
        if (!enabled) return null;
        return { label: 'Video mode', cls: '' };
    }

    function buildFromSettings(s, watchActive) {
        const items = [];
        items.push(hashPill(s.hashAlgorithm));
        const p2 = par2Pill(s.par2Enabled, s.par2Percent);
        if (p2) items.push(p2);
        const vp = videoPill(s.videoMode);
        if (vp) items.push(vp);
        const wp = watchPill(watchActive);
        if (wp) items.push(wp);
        return items;
    }

    return { render, buildFromSettings };
})();
