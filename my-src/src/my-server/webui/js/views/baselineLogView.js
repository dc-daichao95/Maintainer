import { escapeHtml } from '../utils.js';
import { goBack } from '../router.js';

export async function renderBaselineLogView(id, idx) {
    const res = await fetch(`/api/patchset?id=${encodeURIComponent(id)}`);
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    const data = await res.json();

    let logContent = 'Log not found';
    let baselineName = '-';
    let status = '-';

    try {
        const logs = JSON.parse(data.baseline_logs);
        if (Array.isArray(logs) && logs[idx]) {
            logContent = logs[idx].log || '(empty log)';
            baselineName = logs[idx].baseline || '-';
            status = logs[idx].status || '-';
        }
    } catch (e) {
        logContent = 'Failed to parse baseline logs';
    }

    const app = document.getElementById('app');
    if (!app) return;
    
    app.innerHTML = `
        <div class="nav"><a href="#/" onclick="event.preventDefault(); goBack();">← Back</a></div>
        <h1>
            <span>Baseline Application Log</span>
            <span >
                ${escapeHtml(baselineName)} · 
                <span class="status-badge status-${(status || '').replace(/ /g, '')}">${status}</span>
            </span>
        </h1>
        <div class="section">
            <div class="log-block" >${escapeHtml(logContent)}</div>
        </div>
    `;
}
