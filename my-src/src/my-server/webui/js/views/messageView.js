import { escapeHtml, formatDate, parseAuthor, formatAuthorList } from '../utils.js';
import { goBack } from '../router.js';

export async function renderMessageView(id) {
    const res = await fetch(`/api/message?id=${encodeURIComponent(id)}`);
    if (!res.ok) throw new Error('Message not found');
    const data = await res.json();

    document.title = `Sashiko - ${data.subject}`;
    const date = formatDate(data.date, true);
    
    const isGitSubmission = data.message_id && (/^[0-9a-f]{40}$/.test(data.message_id) || data.message_id.includes('@sashiko.local'));
    const threadHtml = (!isGitSubmission && data.thread?.length > 0) ? renderThreadTreeWithCurrent(data.thread, data.message_id) : '';

    const app = document.getElementById('app');
    if (!app) return;
    
    app.innerHTML = `
        <div class="nav"><a href="#/" onclick="event.preventDefault(); goBack();">← Back</a></div>
        <h1>${escapeHtml(data.subject) || '(no subject)'}</h1>
        
        <div class="message-meta">
            <div><span class="label">From:</span> ${(() => {
                const a = parseAuthor(data.author);
                return escapeHtml(a.name ? `${a.name} <${a.email}>` : a.email);
            })()}</div>
            <div><span class="label">To:</span> ${escapeHtml(formatAuthorList(data.to))}</div>
            <div><span class="label">Cc:</span> ${escapeHtml(formatAuthorList(data.cc))}</div>
            <div><span class="label">Date:</span> ${date}</div>
            <div><span class="label">ID:</span> ${escapeHtml(data.message_id)} <button class="copy-btn" onclick="copyToClipboard('${escapeHtml(data.message_id)}')">Copy</button></div>
        </div>
        
        ${threadHtml ? `<div class="section"><h2>Thread</h2>${threadHtml}</div>` : ''}
        
        <pre class="body"><div >${escapeHtml(data.subject || '(no subject)')}</div>
${escapeHtml((data.body || '(no body)').replace(/\n+$/, ''), true, false)}${data.diff ? '\n\n' + escapeHtml(data.diff, true) : ''}</pre>
    `;
}

function renderThreadTreeWithCurrent(thread, currentMsgId) {
    if (!thread || thread.length === 0) return '';

    const msgMap = new Map();
    const childrenMap = new Map();
    thread.forEach(m => {
        msgMap.set(m.message_id, m);
        childrenMap.set(m.message_id, []);
    });

    const roots = [];
    thread.forEach(m => {
        const parentId = m.in_reply_to;
        if (parentId && msgMap.has(parentId)) {
            childrenMap.get(parentId).push(m);
        } else {
            roots.push(m);
        }
    });

    let html = '<div class="thread-tree"><ul>';
    function renderNode(m, depth) {
        const mDate = m.date ? formatDate(m.date) : '-';
        const arrow = depth > 0 ? '↳ ' : '';
        const isCurrent = m.message_id === currentMsgId;
        const style = isCurrent ? 'class="current"' : '';
        const author = parseAuthor(m.author);
        const authorStr = author.name ? `${author.name} <${author.email}>` : author.email;
        html += `<li>${arrow}<span class="date">[${mDate}]</span> ${escapeHtml(authorStr)}: <a href="#/message/${encodeURIComponent(m.message_id)}" ${style}>${escapeHtml(m.subject || '(no subject)')}</a>`;
        const children = childrenMap.get(m.message_id) || [];
        if (children.length > 0) {
            html += '<ul>';
            children.forEach(c => renderNode(c, depth + 1));
            html += '</ul>';
        }
        html += '</li>';
    }
    roots.forEach(r => renderNode(r, 0));
    html += '</ul></div>';
    return html;
}
