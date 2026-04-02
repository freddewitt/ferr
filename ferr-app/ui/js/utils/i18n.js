// i18n.js — localization helper (English default, French supported)

const I18n = (() => {
    let _strings = {};
    let _lang = 'en';

    async function load(lang) {
        try {
            const res = await fetch(`locales/${lang}.json`);
            _strings = await res.json();
            _lang = lang;
        } catch {
            if (lang !== 'en') await load('en');
        }
    }

    function t(key, vars) {
        let str = _strings[key] ?? key;
        if (vars) {
            for (const [k, v] of Object.entries(vars)) {
                str = str.replace(new RegExp(`\\{${k}\\}`, 'g'), v);
            }
        }
        return str;
    }

    function getLang() { return _lang; }

    return { load, t, getLang };
})();

// Shorthand
const t = (key, vars) => I18n.t(key, vars);
