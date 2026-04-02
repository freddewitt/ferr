// format.js — file size, speed, time formatting helpers

const Fmt = {
    bytes(n) {
        if (n == null) return '—';
        if (n >= 1e12) return (n / 1e12).toFixed(2) + ' TB';
        if (n >= 1e9)  return (n / 1e9).toFixed(1)  + ' GB';
        if (n >= 1e6)  return (n / 1e6).toFixed(1)  + ' MB';
        if (n >= 1e3)  return (n / 1e3).toFixed(0)  + ' KB';
        return n + ' B';
    },

    speed(bps) {
        if (typeof bps === 'string') return bps; // already formatted
        return Fmt.bytes(bps) + '/s';
    },

    duration(seconds) {
        if (seconds < 60) return `${Math.round(seconds)}s`;
        const m = Math.floor(seconds / 60);
        const s = Math.round(seconds % 60);
        if (m < 60) return `${m}m ${s}s`;
        const h = Math.floor(m / 60);
        return `${h}h ${m % 60}m`;
    },

    date(iso) {
        if (!iso) return '—';
        const d = new Date(iso);
        return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })
            + ' '
            + d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    },

    dateShort(iso) {
        if (!iso) return '—';
        const d = new Date(iso);
        return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
    },

    percent(n, total) {
        if (!total) return '0%';
        return Math.round((n / total) * 100) + '%';
    },

    diskBar(used, total) {
        if (!total) return 0;
        return Math.min(100, Math.round((used / total) * 100));
    },

    basename(path) {
        if (!path) return '';
        return path.replace(/\\/g, '/').split('/').pop() || path;
    },
};
