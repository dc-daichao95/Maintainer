import { escapeHtml, parseAuthor, renderFindingsBadges, copyToClipboard } from '../utils.js';
import { state, navigate } from '../router.js?v=20260607_1';

export function updateUrlWithList() {
    let url = new URL(window.location.href);
    let searchParams = new URLSearchParams(window.location.search);
    
    if (searchParams.has('list') || window.location.search.startsWith('?list=')) {
        if (state.selectedList) {
            searchParams.set('list', state.selectedList);
        } else {
            searchParams.delete('list');
        }
        let searchStr = searchParams.toString() ? '?' + searchParams.toString() : '';
        window.history.replaceState(null, null, window.location.pathname + searchStr + window.location.hash);
        return;
    }

    let hash = url.hash;
    let queryPart = '';
    
    const qIndex = hash.indexOf('?');
    if (qIndex !== -1) {
        hash = hash.substring(0, qIndex);
    }
    if (!hash) {
        hash = '#/';
    }
    
    const params = new URLSearchParams();
    if (state.selectedList) {
        params.set('list', state.selectedList);
    }
    
    if (params.toString()) {
        queryPart = '?' + params.toString();
    }
    
    window.history.replaceState(null, null, window.location.pathname + window.location.search + hash + queryPart);
}

export function setMailingList(list) {
    state.selectedList = list;
    state.currentPage = 1;
    updateUrlWithList();
    fetchListData();
}

export async function renderListView() {
    const app = document.getElementById('app');

    const listOptions = state.mailingLists.map(l => 
        `<option value="${escapeHtml(l.group)}" ${state.selectedList === l.group ? 'selected' : ''}>${escapeHtml(l.name)}</option>`
    ).join('');

    app.innerHTML = `
        <h1>
            <span  onclick="openAbout()">
                <img src="logo.png" >
                Kernel Maintainer
            </span>
        </h1>
        <div class="flex flex-wrap gap-3 mb-6 items-center">
            <select id="listSelector" onchange="setMailingList(this.value)" >
                <option value="">All Lists</option>
                ${listOptions}
            </select>
            
            <input type="text" id="searchInput" placeholder="Search (subject, author:, date:)" value="${escapeHtml(state.searchQuery)}">

            <button onclick="triggerSearch()">Search</button>
            <button onclick="clearSearch()">Clear</button>
            <div ></div>
            <button id="btn-patchsets" onclick="setListMode('patchsets')" class="${state.listMode === 'patchsets' ? 'active' : ''}">Patchsets</button>
            <button id="btn-messages" onclick="setListMode('messages')" class="${state.listMode === 'messages' ? 'active' : ''}">Messages</button>
        </div>
        <div class="bg-white shadow rounded-lg overflow-hidden border border-gray-200"><table id="dataTable" class="min-w-full divide-y divide-gray-200">
            <thead id="tableHead" class="bg-gray-50 text-gray-500 text-xs uppercase tracking-wider"></thead>
<tbody id="tableBody" class="bg-white divide-y divide-gray-200 text-sm"></tbody>
</table></div>
        <div class="flex justify-center items-center gap-2 mt-6 text-sm text-gray-600" id="pagination"></div>
        <div class="stats" id="stats">Loading stats...</div>
    `;

    updateTableHeader();
    await fetchListData();
    fetchStats();

    document.getElementById('searchInput').addEventListener('keypress', e => {
        if (e.key === 'Enter') triggerSearch();
    });
}

function updateTableHeader() {
    const thead = document.getElementById('tableHead');
    if (!thead) return;

    if (state.listMode === 'patchsets') {
        thead.innerHTML = `<tr><th>Subject</th><th >Author</th><th >Date</th><th >Parts</th><th >Status</th><th >Findings</th></tr>`;
    } else {
        thead.innerHTML = `<tr><th>Subject</th><th >Author</th><th >Date</th><th ></th></tr>`;
    }
}

async function fetchListData() {
    const endpoint = state.listMode === 'patchsets' ? '/api/patchsets' : '/api/messages';
    let url = `${endpoint}?page=${state.currentPage}&per_page=${state.perPage}`;
    if (state.searchQuery) {
        url += `&q=${encodeURIComponent(state.searchQuery)}`;
    }
    if (state.selectedList) {
        url += `&mailing_list=${encodeURIComponent(state.selectedList)}`;
    }

    const res = await fetch(url);
    let data;
    try {
        if (!res.ok) throw new Error("API error");
        data = await res.json();
    } catch (e) {
        console.warn("API not available, using empty data:", e);
        data = { items: [], total: 0 };
    }

    const items = data.items || [];
    const total = data.total || 0;

    state.totalPages = Math.ceil(total / state.perPage) || 1;

    const tbody = document.getElementById('tableBody');
    if (!tbody) return;
    tbody.innerHTML = '';

    items.forEach((p, idx) => {
        const row = document.createElement('tr');
        const d = new Date(p.date * 1000);
        const dateStr = d.toISOString().split('T')[0];
        const timeStr = d.toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'});

        const dateHtml = `
            <div>${dateStr}</div>
            <div >${timeStr}</div>
        `;

        const author = parseAuthor(p.author);
        const nameToDisplay = author.name || author.email;
        const emailToDisplay = author.name ? author.email : '';

        const authorHtml = `
            <div>${escapeHtml(nameToDisplay)}</div>
            ${emailToDisplay ? `<div >${escapeHtml(emailToDisplay)}</div>` : ''}
        `;

        if (state.listMode === 'patchsets') {
            const parts = (p.received_parts === p.total_parts && p.total_parts > 0)
                ? `${p.total_parts}`
                : `${p.received_parts || 0}/${p.total_parts || 0}`;

            const clickHandler = (e) => {
                if (e.target.closest('.copy-btn')) return;
                const target = `#/patchset/${encodeURIComponent(p.message_id || p.id)}`;
                if (e.button === 1 || e.ctrlKey || e.metaKey) {
                    window.open(target, '_blank');
                } else if (e.button === 0) {
                    navigate(target);
                }
            };
            row.onclick = clickHandler;
            row.onauxclick = clickHandler;

            let tagsHtml = '';
            if (p.subsystems && Array.isArray(p.subsystems) && p.subsystems.length > 0) {
                tagsHtml = '<div >' + p.subsystems.map(s => `<span class="tag">${escapeHtml(s)}</span>`).join('') + '</div>';
            }

            let statusLabel = p.status || 'Pending';
            let findingsHtml = '-';
            if (p.findings_low !== undefined || p.findings_medium !== undefined || p.findings_high !== undefined || p.findings_critical !== undefined) {
                const counts = {
                    low: p.findings_low || 0,
                    medium: p.findings_medium || 0,
                    high: p.findings_high || 0,
                    critical: p.findings_critical || 0
                };
                findingsHtml = renderFindingsBadges(counts, statusLabel);
            }

            row.innerHTML = `
                <td><div>${escapeHtml(p.subject) || '(no subject)'}</div>${tagsHtml}</td>
                <td >${authorHtml}</td>
                <td >${dateHtml}</td>
                <td>${parts}</td>
                <td><span class="status-badge status-${statusLabel.replace(/ /g, '')}">${statusLabel}</span></td>
                <td>${findingsHtml}</td>
            `;
        } else {
            const clickHandler = (e) => {
                if (e.target.closest('.copy-btn')) return;
                const target = `#/message/${encodeURIComponent(p.message_id)}`;
                if (e.button === 1 || e.ctrlKey || e.metaKey) {
                    window.open(target, '_blank');
                } else if (e.button === 0) {
                    navigate(target);
                }
            };
            row.onclick = clickHandler;
            row.onauxclick = clickHandler;

            row.innerHTML = `
                <td>${escapeHtml(p.subject) || '(no subject)'}</td>
                <td >${authorHtml}</td>
                <td >${dateHtml}</td>
                <td><button class="copy-btn" onclick="event.stopPropagation(); copyToClipboard('${escapeHtml(p.message_id)}')">📋</button></td>
            `;
        }
        tbody.appendChild(row);
    });

    updatePagination(total);
}

function updatePagination(total) {
    const el = document.getElementById('pagination');
    if (!el) return;
    el.innerHTML = `
        <button onclick="goToPage(1)" ${state.currentPage <= 1 ? 'disabled' : ''}>First</button>
        <button onclick="changePage(-1)" ${state.currentPage <= 1 ? 'disabled' : ''}>Prev</button>
        <span>Page ${state.currentPage} of ${state.totalPages} (${total} total)</span>
        <button onclick="changePage(1)" ${state.currentPage >= state.totalPages ? 'disabled' : ''}>Next</button>
        <button onclick="goToPage(${state.totalPages})" ${state.currentPage >= state.totalPages ? 'disabled' : ''}>Last</button>
    `;
}

async function fetchStats() {
    try {
        const res = await fetch('/api/stats');
        if (!res.ok) throw new Error("API error");
        const data = await res.json();
        state.stats = data;

        const el = document.getElementById('stats');
        if (!el) return;
        el.outerHTML = `
            <div class="stats-container" id="stats">
                <div class="stats-group">
                    <span class="stats-label">Sashiko</span> v${data.version}
                    <span class="stats-sep"></span>
                    <a href="#" onclick="event.preventDefault(); openAbout();" >About</a>
                    <span class="stats-sep"></span>
                    <a href="#/stats" >Stats</a>
                </div>
                <div class="stats-sep"></div>
                <div class="stats-group">
                    <span class="stats-val">${data.patchsets}</span> Patchsets
                    <span class="stats-sub">(${data.messages} msgs)</span>
                </div>
                <div class="stats-sep"></div>
                <div class="stats-group" title="Pending patches to review">
                    <span class="status-dot status-Pending"></span> ${data.pending || 0} Pending
                </div>
                <div class="stats-group" title="Patches currently in review">
                    <span class="status-dot status-InReview"></span> ${data.reviewing || 0} In Review
                </div>
            </div>
        `;
    } catch (e) {
        console.error(e);
        const el = document.getElementById('stats');
        if (el) el.innerHTML = 'Stats unavailable';
    }
}

export function triggerSearch() {
    const input = document.getElementById('searchInput');
    if (!input) return;
    state.searchQuery = input.value.trim();
    state.currentPage = 1;
    fetchListData();
}

export function clearSearch() {
    const input = document.getElementById('searchInput');
    if (input) input.value = '';
    state.searchQuery = '';
    state.currentPage = 1;
    fetchListData();
}

export function setListMode(mode) {
    if (state.listMode === mode) return;
    state.listMode = mode;
    state.currentPage = 1;
    const bp = document.getElementById('btn-patchsets');
    const bm = document.getElementById('btn-messages');
    if (bp) bp.classList.toggle('active', mode === 'patchsets');
    if (bm) bm.classList.toggle('active', mode === 'messages');
    updateTableHeader();
    fetchListData();
}

export async function changePage(delta) {
    const newPage = state.currentPage + delta;
    if (newPage < 1 || newPage > state.totalPages) return;
    state.currentPage = newPage;
    await fetchListData();
}

export async function goToPage(page) {
    if (page < 1 || page > state.totalPages) return;
    state.currentPage = page;
    await fetchListData();
}

window.setMailingList = setMailingList;
window.triggerSearch = triggerSearch;
window.clearSearch = clearSearch;
window.setListMode = setListMode;
window.changePage = changePage;
window.goToPage = goToPage;
window.updateRowSelection = (rows) => {
    rows.forEach((r, i) => {
        if (i === state.selectedIndex) {
            r.classList.add('selected');
            r.scrollIntoView({ block: 'nearest' });
        } else {
            r.classList.remove('selected');
        }
    });
};
