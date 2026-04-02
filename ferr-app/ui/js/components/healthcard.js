// healthcard.js — selectable health action card component

class HealthCard {
    constructor(container, opts = {}) {
        this._container = container;
        this._id     = opts.id     ?? 'card';
        this._icon   = opts.icon   ?? '';
        this._title  = opts.title  ?? '';
        this._desc   = opts.desc   ?? '';
        this._onClick = opts.onClick ?? (() => {});
        this._render();
    }

    _render() {
        const el = document.createElement('div');
        el.className = 'health-card';
        el.dataset.id = this._id;
        el.innerHTML = `
            <div class="health-card-header">
                <div class="health-card-icon">${this._icon}</div>
                <div>
                    <div class="health-card-title">${this._title}</div>
                    <div class="health-card-desc">${this._desc}</div>
                </div>
            </div>
        `;
        el.addEventListener('click', () => this._onClick(this._id));
        this._el = el;
        this._container.appendChild(el);
    }

    setSelected(sel) {
        this._el.classList.toggle('selected', sel);
    }
}
