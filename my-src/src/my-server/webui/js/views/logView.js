import { escapeHtml, formatJsonPretty, formatSize } from '../utils.js';
import { goBack } from '../router.js';

export async function renderLogView(id) {
    const res = await fetch(`/api/review?id=${id}`);
    if (!res.ok) throw new Error('Review not found');
    const data = await res.json();

    const totalTokens = (data.tokens_in || 0) + (data.tokens_out || 0) + (data.tokens_cached || 0);
    let logs = [];
    try { logs = JSON.parse(data.logs || '[]'); } catch (e) { }

    let logsHtml = '';
    if (!logs.length) {
        logsHtml = '<div >No logs recorded.</div>';
    } else {
        let jsonCounter = 0;
        logsHtml = logs.map((entry, idx) => {
            const role = (entry.role || 'unknown').toLowerCase();
            const parts = entry.parts || [];
            let rowClass = 'log-entry';
            if (role === 'user') rowClass += ' role-user';
            else if (role === 'model' || role === 'assistant') rowClass += ' role-model';
            else if (role === 'tool' || role === 'function') rowClass += ' role-tool';
            else rowClass += ' role-system';

            let label = (role === 'model' || role === 'assistant') ? 'LLM' : (role === 'tool' || role === 'function') ? 'TOOL' : role.toUpperCase();

            let content = '';

            if (entry.thought && typeof entry.thought === 'string') {
                content += `<div >${escapeHtml(entry.thought)}</div>`;
            }
            
            if (entry.content && typeof entry.content === 'string') {
                if (role === 'tool') {
                    content += formatJsonExpandable(entry.content, jsonCounter++);
                } else {
                    content += `<div>${escapeHtml(entry.content)}</div>`;
                }
            }
            if (entry.tool_calls && Array.isArray(entry.tool_calls)) {
                entry.tool_calls.forEach(call => {
                    const callId = jsonCounter++;
                    content += formatFuncCallExpandable(call.function_name, call.arguments, callId);
                });
            }

            parts.forEach(part => {
                if (part.text) {
                    if (part.thought) {
                        content += `<div >${escapeHtml(part.text)}</div>`;
                    } else {
                        content += `<div>${escapeHtml(part.text)}</div>`;
                    }
                } else if (part.functionCall || part.function_call) {
                    const fc = part.functionCall || part.function_call;
                    const callId = jsonCounter++;
                    const callHtml = formatFuncCallExpandable(fc.name, fc.args, callId);
                    content += callHtml;
                } else if (part.functionResponse || part.function_response) {
                    const fr = part.functionResponse || part.function_response;
                    const respHtml = formatJsonExpandable(fr.response, jsonCounter++);
                    content += `<div><span >→ ${escapeHtml(fr.name)}:</span> ${respHtml}</div>`;
                }
            });

            return `<div class="${rowClass}"><div class="log-gutter">${label}</div><div class="log-content">${content}</div></div>`;
        }).join('');
    }

    const app = document.getElementById('app');
    if (!app) return;
    app.innerHTML = `
        <div class="nav"><a href="#/" onclick="event.preventDefault(); goBack();">← Back</a></div>
        <h1>
            <span>Interaction Log</span>
            <span >
                ${data.model || '?'} · ${data.prompts_hash ? 'Sashiko ver.: ' + data.prompts_hash.substring(0, 8) + ' · ' : ''}
                <span class="status-badge status-${(data.status || '').replace(/ /g, '')}">${data.status}</span>
            </span>
        </h1>
        <div>${logsHtml}</div>
    `;
}

function getToolPreview(name, args) {
    try {
        switch (name) {
            case 'read_files': {
                const files = args.files || [];
                const mode = args.mode || 'raw';
                const fileStrs = files.map(f => {
                    let s = `"${f.path}"`;
                    if (f.start_line || f.end_line) {
                        s += `::${f.start_line || ''}..${f.end_line || ''}`;
                    }
                    return s;
                });
                const filesPreview = fileStrs.length > 3 ? fileStrs.slice(0, 3).join(', ') + `, +${fileStrs.length - 3}` : fileStrs.join(', ');
                return mode !== 'raw' ? `${filesPreview}, mode=${mode}` : filesPreview;
            }
            case 'git_blame': {
                let s = `"${args.path}"`;
                if (args.start_line || args.end_line) {
                    s += `::${args.start_line || ''}..${args.end_line || ''}`;
                }
                return s;
            }
            case 'git_diff': {
                const diffArgs = args.args || [];
                return diffArgs.map(a => `"${a}"`).join(' ');
            }
            case 'git_show': {
                let s = `"${args.object}"`;
                if (args.suppress_diff) s += ', no-diff';
                if (args.start_line || args.end_line) {
                    s += `::${args.start_line || ''}..${args.end_line || ''}`;
                }
                return s;
            }
            case 'list_dir':
                return `"${args.path}"`;
            case 'write_file': {
                const contentLen = (args.content || '').length;
                return `"${args.path}", ${formatSize(contentLen)} content`;
            }
            case 'search_file_content': {
                let s = `"${args.pattern}"`;
                if (args.path && args.path !== '.') s += ` in "${args.path}"`;
                if (args.context_lines) s += ` ±${args.context_lines}`;
                return s;
            }
            case 'find_files': {
                let s = `"${args.pattern}"`;
                if (args.path && args.path !== '.') s += ` in "${args.path}"`;
                return s;
            }
            default:
                return null;
        }
    } catch (e) {
        return null;
    }
}

function formatFuncCallExpandable(name, args, id) {
    if (typeof args === 'string') { try { args = JSON.parse(args); } catch (e) { } }
    if (!args || typeof args !== 'object') {
        return `<div><span class="fn-name">${escapeHtml(name)}</span>()</div>`;
    }

    let entries = Object.entries(args);
    if (entries.length === 0) {
        return `<div><span class="fn-name">${escapeHtml(name)}</span>()</div>`;
    }

    let preview = getToolPreview(name, args);
    if (!preview) {
        preview = entries.map(([k, v]) => {
            let valStr = JSON.stringify(v);
            if (valStr?.length > 40) valStr = valStr.substring(0, 37) + '...';
            return `${k}=${valStr}`;
        }).join(', ');
    }

    const fullJson = JSON.stringify(args, null, 2);
    const argsSize = formatSize(fullJson.length);

    const expandId = `json-call-${id}`;
    const shortPreview = preview.length > 80 ? preview.substring(0, 77) + '...' : preview;
    return `<div class="json-expandable" onclick="toggleJsonExpand('${expandId}')"><span class="fn-name">${escapeHtml(name)}</span>(<span class="json-preview">${escapeHtml(shortPreview)}</span>) <span class="json-size">(${argsSize})</span><pre id="${expandId}" class="json-formatted" style="display:none;">${formatJsonPretty(args)}</pre></div>`;
}

function formatJsonExpandable(obj, id) {
    if (typeof obj === 'string') { try { obj = JSON.parse(obj); } catch (e) { } }
    const jsonStr = JSON.stringify(obj);
    const size = formatSize(jsonStr.length);
    const sizeHtml = `<span class="json-size">(${size})</span>`;
    const preview = jsonStr.length > 100 ? jsonStr.substring(0, 97) + '...' : jsonStr;

    if (jsonStr.length <= 100 && !jsonStr.includes('\\n') && !jsonStr.includes('\\t')) {
        return `${sizeHtml}${escapeHtml(preview)}`;
    }

    const expandId = `json-resp-${id}`;
    return `${sizeHtml}<span class="json-expandable" onclick="toggleJsonExpand('${expandId}')"><span class="json-preview">${escapeHtml(preview)}</span><pre id="${expandId}" class="json-formatted" style="display:none;">${formatJsonPretty(obj)}</pre></span>`;
}
