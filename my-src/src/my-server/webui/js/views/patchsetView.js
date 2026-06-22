import { escapeHtml, formatDate, parseAuthor, renderFindingsBadges, toggleCollapsible } from '../utils.js';
import { state, navigate } from '../router.js';

export async function renderPatchsetView(id, query = {}) {
    const page = parseInt(query.page) || 1;
    const limit = parseInt(query.limit) || 50;
    const res = await fetch(`/api/patchset?id=${encodeURIComponent(id)}&page=${page}&limit=${limit}`);
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    const data = await res.json();
    
    state.patchsetId = id;
    state.patchsetPage = data.page || page;
    state.patchsetLimit = data.limit || limit;
    state.patchsetTotalPages = data.total_patches_in_db > 0 ? Math.ceil(data.total_patches_in_db / state.patchsetLimit) : 1;

    const date = formatDate(data.date, true);
    const patchMap = new Map();
    if (data.patches) data.patches.forEach(p => patchMap.set(p.id, p));

    const reviewsByPatch = new Map();
    if (data.patches) data.patches.forEach(p => reviewsByPatch.set(p.id, []));
    reviewsByPatch.set(null, []);
    if (data.reviews) {
        data.reviews.forEach(r => {
            const pid = r.patch_id || null;
            if (!reviewsByPatch.has(pid)) reviewsByPatch.set(pid, []);
            reviewsByPatch.get(pid).push(r);
        });
    }

    let reviewsHtml = '';
    const renderReviewList = (reviews, emailHtml = '', patchStatus = null) => {
        if (!reviews || reviews.length === 0) {
            const statusToCheck = patchStatus || data.status;
            if (statusToCheck === 'Embargoed' && data.embargo_until) {
                return `<p >Embargoed until ${formatDate(data.embargo_until, true)}</p>`;
            }
            return `<p >No reviews yet.</p>`;
        }
        return reviews.map((r, i) => renderReviewCard(r, i === reviews.length - 1 ? emailHtml : '')).join('');
    };

    const generalReviews = reviewsByPatch.get(null) || [];
    if (generalReviews.length > 0) {
        reviewsHtml += `<h3>General Reviews</h3>${renderReviewList(generalReviews)}`;
    }

    const sortedPatchIds = Array.from(reviewsByPatch.keys())
        .filter(id !== null)
        .sort((a, b) => {
            const pa = patchMap.get(a);
            const pb = patchMap.get(b);
            return (pa?.part_index || 0) - (pb?.part_index || 0);
        });

    let summaryHtml = '';
    if (sortedPatchIds.length > 1) {
        summaryHtml = '<table ><thead><tr><th>Patch</th><th>Potential Regressions</th></tr></thead><tbody>';
    }

    sortedPatchIds.forEach((pid, index) => {
        const patch = patchMap.get(pid);
        const reviews = reviewsByPatch.get(pid);
        let header = patch
            ? `Patch ${patch.part_index}: <a href="#/message/${encodeURIComponent(patch.message_id)}">${escapeHtml(patch.subject || '(no subject)')}</a>`
            : `Patch ID ${pid}`;
        const patchLink = window.location.origin + window.location.pathname + `#/patchset/${encodeURIComponent(state.patchsetId)}?${patch ? 'part=' + patch.part_index : 'patch=' + pid}`;
        header += `<a href="${patchLink}" class="permalink-icon" onclick="copyToClipboard('${patchLink}'); return false;" title="Copy permalink to this patch">
            <svg viewBox="0 0 16 16" width="16" height="16" fill="currentColor"><path d="m7.775 3.275 1.25-1.25a3.5 3.5 0 1 1 4.95 4.95l-2.5 2.5a3.5 3.5 0 0 1-4.95 0 .751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018 1.998 1.998 0 0 0 2.83 0l2.5-2.5a2.002 2.002 0 0 0-2.83-2.83l-1.25 1.25a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042Zm-4.69 9.64a1.998 1.998 0 0 0 2.83 0l1.25-1.25a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042l-1.25 1.25a3.5 3.5 0 1 1-4.95-4.95l2.5-2.5a3.5 3.5 0 0 1 4.95 0 .751.751 0 0 1-.018 1.042.751.751 0 0 1-1.042.018 1.998 1.998 0 0 0-2.83 0l-2.5 2.5a1.998 1.998 0 0 0 0 2.83Z"></path></svg>
        </a>`;

        const borderStyle = '';

        let statusHtml = '';
        if (patch && (patch.status === 'error' || patch.status === 'failed')) {
             statusHtml = `<div ><strong>Patch Application Failed:</strong>\n${escapeHtml(patch.apply_error || 'Unknown error')}</div>`;
        }

        let emailHtml = '';
        if (patch) {
            let toAddrs = patch.email_to || '';
            let ccAddrs = patch.email_cc || '';
            let eStatus = patch.email_status;

            if (!data.smtp_enabled) {
                eStatus = 'Disabled (SMTP not configured)';
            } else if (data.dry_run) {
                eStatus = eStatus ? `Dry-run (${eStatus})` : 'Dry-run (Not sent)';
            } else {
                eStatus = eStatus || 'None';
            }

            try { if (toAddrs.startsWith('[')) toAddrs = JSON.parse(toAddrs).join(', '); } catch(e) {}
            try { if (ccAddrs.startsWith('[')) ccAddrs = JSON.parse(ccAddrs).join(', '); } catch(e) {}

            emailHtml = `
                <div >
                    <strong>To:</strong> ${escapeHtml(toAddrs || 'None')} &nbsp;&nbsp; 
                    <strong>Cc:</strong> ${escapeHtml(ccAddrs || 'None')} &nbsp;&nbsp; 
                    <strong>Status:</strong> ${escapeHtml(eStatus)}
                </div>
            `;
        }

        let listHtml = '';
        if (patch && patch.status === 'Skipped') {
            listHtml = `<p >Skipped.</p>`;
        } else if (reviews && reviews.length > 0 && reviews.every(r => r.status === 'Skipped')) {
            const reason = reviews[0].result || 'Skipped.';
            listHtml = `<p >${escapeHtml(reason)}</p>`;
        } else {
            listHtml = renderReviewList(reviews, emailHtml, patch ? patch.status : null);
        }

        reviewsHtml += `
            <div id="patch-${pid}" ${patch ? `data-part="${patch.part_index}"` : ''} >
                <h3 >
                    <span>${header}</span>
                </h3>
                ${statusHtml}
                ${listHtml}
            </div>
        `;

        if (sortedPatchIds.length > 1) {
            let summaryStatus = '';

            if (patch && (patch.status === 'error' || patch.status === 'failed')) {
                summaryStatus = `<span >Apply Failed</span>`;
            } else if (patch && patch.status === 'Skipped') {
                summaryStatus = '<span >Skipped</span>';
            } else if (!reviews || reviews.length === 0) {
                summaryStatus = '<span >No reviews</span>';
            } else {
                let maxCounts = { low: 0, medium: 0, high: 0, critical: 0 };
                let hasSuccessful = false;
                let hasSkipped = false;
                let hasInProgress = false;
                let skippedReason = '';
                let errors = [];

                let validRunCount = 0;
                reviews.forEach(r => {
                   if (r.status !== 'Failed To Apply') {
                       validRunCount++;
                   }

                   if (r.status === 'Reviewed') {
                       hasSuccessful = true;
                       try {
                           if (r.output) {
                               const out = typeof r.output === 'string' ? JSON.parse(r.output) : r.output;
                               if (out && out.findings && Array.isArray(out.findings)) {
                                   let current = { low: 0, medium: 0, high: 0, critical: 0 };
                                   out.findings.forEach(f => {
                                       const s = String(f.severity || '').toLowerCase();
                                       if (s === 'critical') current.critical++;
                                       else if (s === 'high') current.high++;
                                       else if (s === 'medium') current.medium++;
                                       else current.low++;
                                   });

                                   maxCounts.critical = Math.max(maxCounts.critical, current.critical);
                                   maxCounts.high = Math.max(maxCounts.high, current.high);
                                   maxCounts.medium = Math.max(maxCounts.medium, current.medium);
                                   maxCounts.low = Math.max(maxCounts.low, current.low);
                               }
                           }
                       } catch (e) {}
                   } else if (r.status === 'Skipped') {
                       hasSkipped = true;
                       skippedReason = r.result || 'Skipped';
                   } else if (r.status && (r.status.toLowerCase().includes('failed') || r.status.toLowerCase().includes('error'))) {
                       errors.push(r.status);
                   } else if (r.status === 'Pending' || r.status === 'In Review') {
                       hasInProgress = true;
                   }
                });

                if (hasSuccessful) {
                   summaryStatus = renderFindingsBadges(maxCounts, 'Reviewed');
                   if (validRunCount > 1) {
                       summaryStatus += ` <span >(max of ${validRunCount} runs)</span>`;
                   }
                } else if (errors.length > 0) {
                   summaryStatus = `<span >${escapeHtml(errors[0])}</span>`;
                } else if (hasSkipped && !hasInProgress) {
                   summaryStatus = `<span  title="${escapeHtml(skippedReason)}">Skipped</span>`;
                } else {
                   summaryStatus = '<span >In progress...</span>';
                }
            }
            const patchLabel = patch ? `Patch ${patch.part_index}` : `Patch ${pid}`;
            const patchSubject = patch ? escapeHtml(patch.subject || '(no subject)') : '';
            summaryHtml += `<tr onclick="document.getElementById('patch-${pid}').scrollIntoView({behavior: 'smooth'})" ><td><strong>${patchLabel}</strong>: ${patchSubject}</td><td>${summaryStatus}</td></tr>`;
        }
    });

    if (sortedPatchIds.length > 1) {
        summaryHtml += '</tbody></table>';
    }

    if (!reviewsHtml) reviewsHtml = '<p>No AI reviews yet.</p>';

    let paginationHtml = '';
    if (data.total_patches_in_db > data.limit) {
        const totalPages = Math.ceil(data.total_patches_in_db / data.limit);
        const basePath = `#/patchset/${encodeURIComponent(data.id || data.message_id)}`;
        
        paginationHtml = `
            <div class="pagination">
                <button onclick="navigate('${basePath}?page=1&limit=${data.limit}')" ${data.page <= 1 ? 'disabled' : ''}>First</button>
                <button onclick="navigate('${basePath}?page=${data.page - 1}&limit=${data.limit}')" ${data.page <= 1 ? 'disabled' : ''}>Prev</button>
                <span>Page ${data.page} of ${totalPages} (${data.total_patches_in_db} total)</span>
                <button onclick="navigate('${basePath}?page=${data.page + 1}&limit=${data.limit}')" ${data.page >= totalPages ? 'disabled' : ''}>Next</button>
                <button onclick="navigate('${basePath}?page=${totalPages}&limit=${data.limit}')" ${data.page >= totalPages ? 'disabled' : ''}>Last</button>
            </div>
        `;
    }

    const isGitSubmission = (data.message_id && (/^[0-9a-f]{40}$/.test(data.message_id) || data.message_id.includes('@sashiko.local'))) || (data.subject && data.subject.startsWith('Git Import:'));

    const threadHtml = isGitSubmission ? '' : renderThreadTree(data.thread);
    const subsystemsHtml = data.subsystems?.length > 0
        ? data.subsystems.map(s => `<span class="tag">${escapeHtml(s)}</span>`).join('')
        : '-';

    let failureHtml = '';
    if (data.failed_reason) {
        failureHtml = `
            <div class="section" >
                <h3 >Ingestion Failed</h3>
                <div >${escapeHtml(data.failed_reason)}</div>
            </div>
        `;
    }

    const modelName = data.model_name || '-';
    const providerModel = data.provider ? `${data.provider}/${modelName}` : modelName;
    const promptsHash = data.prompts_git_hash ? data.prompts_git_hash.substring(0, 8) : '-';
    
    let baselineStr = '-';
    if (data.baseline) {
         const parts = [];
         if (data.baseline.branch) parts.push(data.baseline.branch);
         if (data.baseline.commit) parts.push(data.baseline.commit.substring(0, 8));
         if (parts.length > 0) baselineStr = parts.join(' / ');
    }

    let baselineLogsHtml = '';
    if (data.baseline_logs && !isGitSubmission) {
         const logId = 'baseline-logs-' + Math.random().toString(36).substr(2, 9);
         let contentHtml = '';
         let isJson = false;
         
         try {
             const logs = JSON.parse(data.baseline_logs);
             if (Array.isArray(logs)) {
                 isJson = true;
                 contentHtml = `
                    <table >
                        <thead>
                            <tr >
                                <th >Baseline</th>
                                <th >Status</th>
                                <th >Log</th>
                            </tr>
                        </thead>
                        <tbody>
                 `;
                 
                 logs.forEach((item, idx) => {
                     let statusBadge = '';
                     if (item.status === 'Applied') {
                         statusBadge = '<span class="status-badge status-Applied">Applied</span>';
                     } else {
                         statusBadge = '<span class="status-badge status-Failed">Failed</span>';
                     }
                     
                     contentHtml += `
                        <tr>
                            <td >${escapeHtml(item.baseline)}</td>
                            <td >${statusBadge}</td>
                            <td >
                                <a href="#/log/baseline/${encodeURIComponent(data.id || data.message_id)}/${idx}"  onmouseover="this.style.color='#007bff'" onmouseout="this.style.color='#666'">View Log</a>
                            </td>
                        </tr>
                     `;
                 });
                 contentHtml += '</tbody></table>';
             }
         } catch (e) {
         }

         if (!isJson) {
             const formattedLogs = escapeHtml(data.baseline_logs)
                .replace(/ - passed/g, ' - <span >passed</span>')
                .replace(/ - failed/g, ' - <span >failed</span>')
                .replace(/Application successful./g, '<span >Application successful.</span>')
                .replace(/Application failed./g, '<span >Application failed.</span>');
             
             contentHtml = `<div class="log-block" >${formattedLogs}</div>`;
         }

         baselineLogsHtml = `
            <div class="section">
                <h2 >
                    Baseline
                    <button class="toggle-btn" onclick="toggleCollapsible('${logId}', this)" >▶ Show Details</button>
                </h2>
                <div class="kv"><div class="label">Selected:</div><div>${baselineStr}</div></div>
                <div id="${logId}" class="collapsible">
                    ${contentHtml}
                </div>
            </div>
         `;
    }

    let currentStatus = data.status || 'Pending';

    document.getElementById('app').innerHTML = `
        <div class="nav"><a href="#/" onclick="event.preventDefault(); goBack();">← Back</a></div>
        <h1>
            <span>${escapeHtml(data.subject) || '(no subject)'}</span>
            <span class="status-badge status-${currentStatus.replace(/ /g, '')}">${currentStatus}</span>
        </h1>
        
        ${failureHtml}

        <div class="section">
            <div class="kv"><div class="label">Author:</div><div>${(() => {
                const a = parseAuthor(data.author);
                return escapeHtml(a.name ? `${a.name} <${a.email}>` : a.email);
            })()}</div></div>
            <div class="kv"><div class="label">Date:</div><div>${date}</div></div>
            <div class="kv"><div class="label">Subsystems:</div><div>${subsystemsHtml}</div></div>
            <div class="kv"><div class="label">Patches:</div><div>${data.received_parts || 0} / ${data.total_parts || 0}</div></div>
            <div class="kv"><div class="label">Model:</div><div>${escapeHtml(providerModel)}</div></div>
            <div class="kv"><div class="label">Sashiko ver.:</div><div>${escapeHtml(promptsHash)}</div></div>
        </div>
        
        ${threadHtml ? `<div class="section">
            <h2>Thread</h2>
            ${threadHtml}
        </div>` : ''}

        ${baselineLogsHtml}
        
        <div class="section">
            <h2 id="ai-reviews">AI Reviews</h2>
            ${summaryHtml}
            ${reviewsHtml}
            ${paginationHtml}
        </div>
    `;

    if (query.patch || query.part) {
        setTimeout(() => {
            let el = null;
            if (query.patch) {
                el = document.getElementById(`patch-${query.patch}`);
            }
            if (!el && query.part) {
                el = document.querySelector(`[data-part="${query.part}"]`);
            }
            if (el) {
                el.scrollIntoView({behavior: 'smooth', block: 'center'});
                const origBg = el.style.backgroundColor;
                el.style.backgroundColor = 'var(--selected-bg)';
                el.style.transition = 'background-color 1.5s ease-out';
                setTimeout(() => {
                    el.style.backgroundColor = origBg;
                }, 2000);
            }
        }, 100);
    }
}

function renderReviewCard(r, emailHtml = '') {
    const uniqueId = Math.random().toString(36).substr(2, 9);

    let displayResult = '';
    
    let parsedFindings = false;
    try {
        if (r.output) {
            let outputJson = null;
            if (typeof r.output === 'string') {
                outputJson = JSON.parse(r.output);
            } else if (typeof r.output === 'object') {
                outputJson = r.output;
            }

            if (outputJson && outputJson.findings && Array.isArray(outputJson.findings)) {
                let counts = { low: 0, medium: 0, high: 0, critical: 0 };
                outputJson.findings.forEach(f => {
                    const s = (f.severity || '').toLowerCase();
                    if (s === 'critical') counts.critical++;
                    else if (s === 'high') counts.high++;
                    else if (s === 'medium') counts.medium++;
                    else counts.low++;
                });
                
                const total = counts.low + counts.medium + counts.high + counts.critical;
                if (total === 0) {
                    displayResult = '<span >No regressions</span>';
                } else {
                    const parts = [];
                    parts.push(`Critical: <span >${counts.critical}</span>`);
                    parts.push(`High: <span >${counts.high}</span>`);
                    parts.push(`Medium: <span >${counts.medium}</span>`);
                    parts.push(`Low: <span >${counts.low}</span>`);
                    displayResult = parts.join(' · ');
                }
                parsedFindings = true;
            }
        }
    } catch (e) { }

    if (!parsedFindings) {
        const rawResult = r.result || 'Finished';
        if (rawResult === "Review completed successfully.") {
             displayResult = '<span >No regressions</span>';
        } else {
            if (rawResult.trim().startsWith('{') || rawResult.length > 60) {
                 displayResult = `<span  title="${escapeHtml(rawResult)}">See log for details</span>`;
            } else {
                 displayResult = `<span >${escapeHtml(rawResult)}</span>`;
            }
        }
    }

    const tokensOut = (r.tokens_out || 0);
    const tokensCached = (r.tokens_cached || 0);
    const tokensIn = Math.max(0, (r.tokens_in || 0) - tokensCached);
    const tokenStr = `In: ${tokensIn.toLocaleString()} &middot; Cached: ${tokensCached.toLocaleString()} &middot; Out: ${tokensOut.toLocaleString()}`;

    let inlineHtml = '';
    if (r.inline_review) {
        const hasIssues = parsedFindings && !displayResult.includes('No regressions');
        const isOpen = hasIssues ? 'open' : '';
        const icon = hasIssues ? '▼' : '▶';
        inlineHtml = `
            <div >
                <button class="toggle-btn" onclick="toggleCollapsible('inline-${uniqueId}', this)">${icon} Inline Review</button>
                <div id="inline-${uniqueId}" class="collapsible ${isOpen}" >
                    <button class="copy-btn"  onclick="copyToClipboard(this.nextElementSibling.innerText)">Copy</button>
                    <div class="log-inline-block">${escapeHtml(r.inline_review, true, true)}</div>
                </div>
            </div>
        `;
    }

    const statusRaw = r.status || 'Finished';
    const s = statusRaw.toLowerCase();
    const showDetails = s === 'reviewed' || s === 'finished' || s === 'failed' || s === 'skipped';

    return `
        <div class="review-card">
            <div class="review-meta">
                ${showDetails ? `
                <div class="review-meta-label">Result:</div>
                <div >
                    <span>${displayResult}</span>
                    <span class="status-badge status-${(statusRaw || '').replace(/ /g, '')}">${statusRaw}</span>
                </div>
                ` : `
                <div class="review-meta-label">Status:</div>
                <div><span class="status-badge status-${(statusRaw || '').replace(/ /g, '')}">${statusRaw}</span></div>
                `}
            </div>
            ${showDetails ? `
            ${inlineHtml}
            <div >
                ${emailHtml}
                <div >
                    <strong>Tokens used:</strong> ${tokenStr}
                </div>
            </div>
            <div >
                ${r.id ? `<a href="#/log/${r.id}"  onmouseover="this.style.textDecoration='underline'" onmouseout="this.style.textDecoration='none'">View Raw Log</a>` : '<span></span>'}
                <a href="javascript:void(0)" onclick="document.getElementById('ai-reviews').scrollIntoView({behavior: 'smooth'})" >↑ Back to Summary</a>
            </div>
            ` : ''}
        </div>
    `;
}

export function renderThreadTree(thread) {
    if (!thread || thread.length === 0) return '<p >No thread history.</p>';

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
        const author = parseAuthor(m.author);
        const authorStr = author.name ? `${author.name} <${author.email}>` : author.email;
        html += `<li>${arrow}<span class="date">[${mDate}]</span> ${escapeHtml(authorStr)}: <a href="#/message/${encodeURIComponent(m.message_id)}">${escapeHtml(m.subject || '(no subject)')}</a>`;
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
