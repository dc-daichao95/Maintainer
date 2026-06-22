export function openAbout() {
    document.getElementById('aboutModal').classList.add('open');
}

export function closeAbout() {
    document.getElementById('aboutModal').classList.remove('open');
}

export function parseAuthor(authorStr) {
    if (!authorStr) return { name: '', email: 'unknown' };
    authorStr = authorStr.trim();
    
    const match = authorStr.match(/^(?:"?([^"]*)"?\s+)?<([^>]+)>$/);
    if (match) {
        let name = match[1] ? match[1].trim() : '';
        const email = match[2].trim();
        
        if (name.toLowerCase() === 'unknown') {
            name = '';
        }
        
        return { name, email };
    }
    
    if (authorStr.includes('@')) {
        return { name: '', email: authorStr };
    }
    
    if (authorStr.toLowerCase() === 'unknown') {
        return { name: '', email: 'unknown' };
    }
    
    return { name: authorStr, email: '' };
}

export function formatAuthorList(str) {
    if (!str) return '-';
    const parts = str.split(',');
    return parts.map(p => {
        const a = parseAuthor(p.trim());
        return a.name ? `${a.name} <${a.email}>` : a.email;
    }).join(', ');
}

export function escapeHtml(text, highlightDiff = false, renderAiComments = false) {
    if (!text) return '';
    const escaped = String(text)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");

    if (highlightDiff) {
        const lines = escaped.split('\n');
        const hasQuotes = lines.some(l => l.trimStart().startsWith('&gt;')); // Escaped >
        let inDiffBlock = false;
        let currentBlockIsQuoted = false;
        let seenDiffOrQuote = false;
        
        // Pass 1: Classify
        const classified = lines.map(line => {
            const trimmed = line.trimStart();
            const isQuoted = trimmed.startsWith('&gt;');
            
            let content = line;
            if (isQuoted) {
                content = trimmed.substring(4); 
                if (content.startsWith(' ')) content = content.substring(1);
            }
            
            const isHeader = content.startsWith('diff --git') || 
                             content.startsWith('Index: ') || 
                             content.startsWith('--- ') || 
                             content.startsWith('+++ ') || 
                             content.startsWith('@@ ');

            const isExtendedHeader = /^(new|old|deleted) file mode|similarity index|copy (from|to)|rename (from|to)/.test(content);
            
            if (isHeader || isExtendedHeader || isQuoted) {
                seenDiffOrQuote = true;
            }

            const isMeta = content.match(/^(commit [0-9a-f]+|Author:|Date:|Subject:|From:|To:|Cc:|Signed-off-by:)/i) && !renderAiComments;

            let type = 'TEXT'; // Default to comment candidate
            let isDefiniteDiff = false;

            if (isHeader || isExtendedHeader) {
                isDefiniteDiff = true;
                inDiffBlock = true;
                currentBlockIsQuoted = isQuoted;
            } else if (hasQuotes) {
                if (isQuoted) {
                    isDefiniteDiff = true;
                } else {
                    if (isMeta) isDefiniteDiff = true;
                    else if (inDiffBlock) {
                        if (currentBlockIsQuoted) {
                            // Breaking out of a quoted block
                            inDiffBlock = false;
                        } else {
                            if (content.startsWith('+') && !content.startsWith('+++')) isDefiniteDiff = true;
                            else if (content.startsWith('-') && !content.startsWith('---') && content.trim() !== '--') isDefiniteDiff = true;
                            else if (content.startsWith(' ') || content.startsWith('\\') || content.startsWith('\t')) isDefiniteDiff = true;
                        }
                    }
                }
            } else {
                if (isMeta) isDefiniteDiff = true;
                else if (inDiffBlock) {
                     if (content.startsWith('+') && !content.startsWith('+++')) isDefiniteDiff = true;
                     else if (content.startsWith('-') && !content.startsWith('---')) isDefiniteDiff = true;
                     else if (content.startsWith(' ') || content.startsWith('\\') || content.startsWith('\t')) isDefiniteDiff = true;
                }
            }

            if (isDefiniteDiff) {
                type = 'DIFF';
            } else if (line.trim() === '') {
                type = 'EMPTY';
            }

            // Store metadata for rendering
            let cls = 'diff-line';
            if (isHeader) cls += ' diff-header';
            else if (isDefiniteDiff) {
                 if (content.startsWith('+') && !content.startsWith('+++')) cls += ' diff-added';
                 else if (content.startsWith('-') && !content.startsWith('---') && content.trim() !== '--') cls += ' diff-removed';
            }

            return { line, type, cls };
        });

        // Pass 2: Render with lookahead/buffering
        let resultHtml = '';
        let buffer = [];
        let lastType = 'DIFF'; // Initial state: DIFF (so leading empty lines are not highlighted)

        for (let i = 0; i < classified.length; i++) {
            const item = classified[i];
            
            if (item.type === 'EMPTY') {
                buffer.push(item);
            } else {
                // Flush buffer
                if (buffer.length > 0) {
                    // If bridging TEXT -> TEXT, highlight buffer
                    const highlightBuffer = (lastType === 'TEXT' && item.type === 'TEXT');
                    buffer.forEach(b => {
                        let c = b.cls;
                        if (highlightBuffer && renderAiComments) c += ' ai-comment';
                        resultHtml += `<div class="${c}">${b.line}</div>`;
                    });
                    buffer = [];
                }
                
                // Output current
                let c = item.cls;
                if (item.type === 'TEXT' && renderAiComments) c += ' ai-comment';
                resultHtml += `<div class="${c}">${item.line}</div>`;
                
                lastType = item.type;
            }
        }
        
        // Flush remaining buffer (trailing empty lines)
        if (buffer.length > 0) {
             // Don't render trailing blank lines that bloat the end of the block
             while (buffer.length > 0 && buffer[buffer.length - 1].line.trim() === '') {
                 buffer.pop();
             }
             buffer.forEach(b => {
                resultHtml += `<div class="${b.cls}">${b.line}</div>`;
            });
        }
        
        return resultHtml;
    }
    return escaped;
}

export function formatDate(unix, full = false) {
    const d = new Date(unix * 1000);
    return full ? d.toLocaleString() : d.toISOString().split('T')[0];
}

export function copyToClipboard(text) {
    navigator.clipboard.writeText(text).then(() => {
        console.log('Copied:', text);
    });
}

export function renderStatCard(title, value) {
    return `
        <div class="stat-card">
            <h3>${escapeHtml(title)}</h3>
            <div class="value">${escapeHtml(String(value))}</div>
        </div>
    `;
}

export function formatSize(bytes) {
    if (bytes === 0) return '0 B';
    if (bytes < 1024) return bytes + 'B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

export function toggleCollapsible(id, btn) {
    const el = document.getElementById(id);
    if (el.classList.contains('open')) {
        el.classList.remove('open');
        if (btn) btn.innerText = btn.innerText.replace('▼', '▶');
    } else {
        el.classList.add('open');
        if (btn) btn.innerText = btn.innerText.replace('▶', '▼');
    }
}

export function renderFindingsBadges(counts, status) {
    const { low = 0, medium = 0, high = 0, critical = 0 } = counts;
    if (low + medium + high + critical === 0) {
        if (status === 'Reviewed' || status === 'Finished') {
            return '<span >✓</span>';
        }
        return '<span >-</span>';
    }
    return `
        <div >
            <span  title="Critical">${critical}</span>
            <span  title="High">${high}</span>
            <span  title="Medium">${medium}</span>
            <span  title="Low">${low}</span>
        </div>
    `;
}

export function sortToolArgs(entries) {
    const priorityKeys = ['file_path', 'path', 'dir_path', 'filename', 'pattern', 'command', 'instruction', 'old_string', 'new_string', 'content', 'objective', 'fact'];

    function getKeyKey(k) {
        const pIdx = priorityKeys.indexOf(k);
        if (pIdx !== -1) return String.fromCharCode(32 + pIdx);
        if (k.startsWith('start_')) return k.substring(6) + '_0';
        if (k.startsWith('end_')) return k.substring(4) + '_1';
        return 'z' + k;
    }

    return entries.sort((a, b) => {
        const ka = getKeyKey(a[0]);
        const kb = getKeyKey(b[0]);
        return ka.localeCompare(kb);
    });
}

export function formatJsonPretty(obj) {
    const formatted = JSON.stringify(obj, null, 2);
    return escapeHtml(formatted).replace(/\\n/g, '\n').replace(/\\t/g, '    ');
}

export function toggleJsonExpand(id) {
    const el = document.getElementById(id);
    el.style.display = el.style.display === 'none' ? 'block' : 'none';
}

// Bind utilities to window for inline HTML handlers
window.openAbout = openAbout;
window.closeAbout = closeAbout;
window.copyToClipboard = copyToClipboard;
window.toggleCollapsible = toggleCollapsible;
window.toggleJsonExpand = toggleJsonExpand;

document.addEventListener('keydown', e => {
    if (e.key === 'Escape') closeAbout();
});
