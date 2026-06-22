import { escapeHtml } from './utils.js';

export const state = {
    view: 'list',
    listMode: 'patchsets',
    currentPage: 1,
    perPage: 50,
    totalPages: 1,
    searchQuery: '',
    selectedIndex: -1,
    stats: null,
    mailingLists: [],
    selectedList: '',
    patchsetId: null,
    patchsetPage: 1,
    patchsetTotalPages: 1,
    patchsetLimit: 50
};

export function parseHash() {
    let hash = window.location.hash || '#/';
    let search = window.location.search || '';
    let query = {};

    if (search.startsWith('?')) {
        const qStr = search.substring(1);
        qStr.split('&').forEach(part => {
            const [k, v] = part.split('=');
            if (k) query[decodeURIComponent(k)] = decodeURIComponent(v || '');
        });
    }

    const qIndex = hash.indexOf('?');
    if (qIndex !== -1) {
        const qStr = hash.substring(qIndex + 1);
        hash = hash.substring(0, qIndex);
        qStr.split('&').forEach(part => {
            const [k, v] = part.split('=');
            if (k) query[decodeURIComponent(k)] = decodeURIComponent(v || '');
        });
    }

    if (hash === '#/' || hash === '#') {
        return { view: 'dashboard', query };
    }
    if (hash === '#/list') {
        return { view: 'list', query };
    }
    const patchMatch = hash.match(/^#\/patchset\/(.+)$/);
    if (patchMatch) {
        return { view: 'patchset', id: decodeURIComponent(patchMatch[1]), query };
    }
    const msgMatch = hash.match(/^#\/message\/(.+)$/);
    if (msgMatch) {
        return { view: 'message', id: decodeURIComponent(msgMatch[1]), query };
    }
    const baselineLogMatch = hash.match(/^#\/log\/baseline\/(.+)\/(\d+)$/);
    if (baselineLogMatch) {
        return { view: 'baseline_log', id: baselineLogMatch[1], idx: parseInt(baselineLogMatch[2]), query };
    }
    const logMatch = hash.match(/^#\/log\/(\d+)$/);
    if (logMatch) {
        return { view: 'log', id: logMatch[1], query };
    }
    if (hash === '#/stats') {
        return { view: 'stats', query };
    }
    if (hash === '#/issues') {
        return { view: 'issues', query };
    }
    if (hash === '#/settings') {
        return { view: 'settings', query };
    }
    if (hash === '#/server-config') {
        return { view: 'server-config', query };
    }
    if (hash === '#/dashboard') {
        return { view: 'dashboard', query };
    }
    return { view: 'dashboard', query };
}

export let hasNavigatedInternally = false;

export function navigate(hash) {
    hasNavigatedInternally = true;
    window.location.hash = hash;
}

export function goBack() {
    if (hasNavigatedInternally) {
        window.history.back();
    } else {
        navigate('#/');
    }
}

// Router needs a way to register view renderers to avoid circular dependencies
const viewRenderers = {
    list: null,
    patchset: null,
    message: null,
    log: null,
    baseline_log: null,
    stats: null,
    issues: null,
    settings: null,
    'server-config': null,
    dashboard: null
};

export function registerView(name, renderer) {
    viewRenderers[name] = renderer;
}

export async function fetchMailingLists() {
    try {
        const res = await fetch('/api/lists');
        if (res.ok) {
            const lists = await res.json();
            lists.sort((a, b) => a.name.localeCompare(b.name));
            state.mailingLists = lists;
        } else {
            state.mailingLists = [];
        }
    } catch (e) {
        console.warn("Failed to fetch mailing lists (Mock Env)", e);
        state.mailingLists = [];
    }
}

export async function router() {
    const route = parseHash();
    state.view = route.view;
    state.selectedIndex = -1;

    if (route.query && typeof route.query.list !== 'undefined') {
         state.selectedList = route.query.list;
    }

    document.querySelectorAll('.nav-link').forEach(el => {
        el.classList.remove('bg-indigo-50', 'text-indigo-700', 'font-semibold');
        const icon = el.querySelector('.nav-icon');
        if (icon) icon.classList.remove('text-indigo-600');
        
        if (el.getAttribute('href') === window.location.hash || 
           (window.location.hash === '' && el.getAttribute('href') === '#/')) {
            el.classList.add('bg-indigo-50', 'text-indigo-700', 'font-semibold');
            if (icon) icon.classList.add('text-indigo-600');
        }
    });

    const pageTitle = document.getElementById('pageTitle');
    if (pageTitle) {
        if (route.view === 'dashboard') pageTitle.innerText = '概览';
        else if (route.view === 'list') pageTitle.innerText = '概览 (旧)';
        else if (route.view === 'stats') pageTitle.innerText = '问题处理统计';
        else if (route.view === 'issues') pageTitle.innerText = '问题列表';
        else if (route.view === 'settings') pageTitle.innerText = '设置';
        else if (route.view === 'server-config') pageTitle.innerText = '服务器配置';
        else pageTitle.innerText = '详情';
    }

    const app = document.getElementById('app');
    if (!app) return;
    
    app.innerHTML = '<div class="p-8 text-center text-gray-500">Loading...</div>';

    try {
        switch (route.view) {
            case 'list':
                if (viewRenderers.list) await viewRenderers.list();
                break;
            case 'patchset':
                if (viewRenderers.patchset) await viewRenderers.patchset(route.id, route.query || {});
                break;
            case 'message':
                if (viewRenderers.message) await viewRenderers.message(route.id);
                break;
            case 'log':
                if (viewRenderers.log) await viewRenderers.log(route.id);
                break;
            case 'baseline_log':
                if (viewRenderers.baseline_log) await viewRenderers.baseline_log(route.id, route.idx);
                break;
            case 'stats':
                if (viewRenderers.stats) {
                    await viewRenderers.stats();
                } else if (window.renderStatsViewMock) {
                    await window.renderStatsViewMock();
                }
                break;
            case 'issues':
                if (viewRenderers.issues) {
                    await viewRenderers.issues();
                }
                break;
            case 'settings':
                if (viewRenderers.settings) {
                    await viewRenderers.settings();
                }
                break;
            case 'server-config':
                if (viewRenderers['server-config']) {
                    await viewRenderers['server-config']();
                }
                break;
            case 'dashboard':
                if (viewRenderers.dashboard) {
                    await viewRenderers.dashboard();
                }
                break;
            default:
                if (viewRenderers.list) await viewRenderers.list();
        }
    } catch (e) {
        app.innerHTML = `<div class="error">Error: ${escapeHtml(e.message)}</div>`;
    }
}

window.addEventListener('hashchange', () => {
    hasNavigatedInternally = true;
    router();
});

// Expose state and navigation globally for inline handlers
window.navigate = navigate;
window.goBack = goBack;
window.state = state;
window.router = router;
