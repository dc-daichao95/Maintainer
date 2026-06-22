import { IssueStore } from '../issueStore.js';
import { mockFindings } from '../mockData.js';

const store = new IssueStore(mockFindings);
store.setPageSize(10);
let currentIssuePage = 1;

export async function renderIssuesView() {
    const app = document.getElementById('app');
    
    const renderTable = () => {
        const pageFindings = store.getPage(currentIssuePage);
        const totalPages = store.getTotalPages() || 1;

        const tableHtml = pageFindings.map(f => {
            let severityStyle = 'bg-green-100 text-green-800';
            let severityText = '低危 (Low)';
            if (f.severity === 'Critical') {
                severityStyle = 'bg-red-100 text-red-800';
                severityText = '极危 (Critical)';
            } else if (f.severity === 'High') {
                severityStyle = 'bg-red-100 text-red-800';
                severityText = '高危 (High)';
            } else if (f.severity === 'Medium') {
                severityStyle = 'bg-yellow-100 text-yellow-800';
                severityText = '中危 (Medium)';
            }

            // Using rule_id as the suggestion text for mock data purposes
            const suggestion = `[${f.subsystem}] ${f.rule_id} 检测到潜在风险`;

            return `
            <tr class="hover:bg-gray-50">
                <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-indigo-600">
                    <a href="#/patchset/${f.id}" class="flex items-center gap-2 hover:underline">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"></path></svg>
                        ${f.file_path}:${f.line_number}
                    </a>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm">
                    <span class="px-3 py-1 inline-flex text-xs leading-5 font-semibold rounded-full ${severityStyle}">
                        ${severityText}
                    </span>
                </td>
                <td class="px-6 py-4 text-sm text-gray-600">
                    ${suggestion}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                    ${f.status === 'TP' ? '已处理 / 有效' : f.status === 'FP' ? '非问题 / 误报' : '待处理'}
                </td>
                <td class="px-6 py-4 text-sm text-gray-500">
                    <input type="text" class="comment-input block w-full border-gray-300 rounded-md shadow-sm focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm" data-id="${f.id}" value="${f.comments || ''}" placeholder="填写您的分析原因...">
                </td>
            </tr>
        `}).join('');

        return `
            <div class="bg-white shadow overflow-hidden sm:rounded-lg border border-gray-200">
                <div class="px-6 py-5 border-b border-gray-200">
                    <h3 class="text-lg font-semibold text-gray-900">问题管理列表</h3>
                </div>
                <table class="min-w-full divide-y divide-gray-200">
                    <thead class="bg-gray-50">
                        <tr>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider w-1/5">
                                源代码链接
                            </th>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider w-32">
                                <div class="flex items-center gap-2">
                                    严重等级
                                    <select id="filter-severity" class="border-0 bg-transparent text-gray-500 text-xs focus:ring-0 cursor-pointer">
                                        <option value="All" ${store.filters.severity === 'All' ? 'selected' : ''}>全部</option>
                                        <option value="Critical" ${store.filters.severity === 'Critical' ? 'selected' : ''}>极危</option>
                                        <option value="High" ${store.filters.severity === 'High' ? 'selected' : ''}>高危</option>
                                        <option value="Medium" ${store.filters.severity === 'Medium' ? 'selected' : ''}>中危</option>
                                        <option value="Low" ${store.filters.severity === 'Low' ? 'selected' : ''}>低危</option>
                                    </select>
                                </div>
                            </th>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider w-1/4">
                                <div class="flex items-center gap-2">
                                    AI审查建议 (子系统)
                                    <select id="filter-subsystem" class="border-0 bg-transparent text-gray-500 text-xs focus:ring-0 cursor-pointer">
                                        <option value="All" ${store.filters.subsystem === 'All' ? 'selected' : ''}>全部</option>
                                        <option value="net" ${store.filters.subsystem === 'net' ? 'selected' : ''}>net</option>
                                        <option value="usb" ${store.filters.subsystem === 'usb' ? 'selected' : ''}>usb</option>
                                        <option value="ext4" ${store.filters.subsystem === 'ext4' ? 'selected' : ''}>ext4</option>
                                        <option value="bpf" ${store.filters.subsystem === 'bpf' ? 'selected' : ''}>bpf</option>
                                        <option value="mm" ${store.filters.subsystem === 'mm' ? 'selected' : ''}>mm</option>
                                        <option value="drm" ${store.filters.subsystem === 'drm' ? 'selected' : ''}>drm</option>
                                        <option value="btrfs" ${store.filters.subsystem === 'btrfs' ? 'selected' : ''}>btrfs</option>
                                        <option value="bluetooth" ${store.filters.subsystem === 'bluetooth' ? 'selected' : ''}>bluetooth</option>
                                    </select>
                                </div>
                            </th>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider w-40">
                                <div class="flex items-center gap-2">
                                    处理意见
                                    <select id="filter-status" class="border-0 bg-transparent text-gray-500 text-xs focus:ring-0 cursor-pointer">
                                        <option value="All" ${store.filters.status === 'All' ? 'selected' : ''}>全部</option>
                                        <option value="Pending" ${store.filters.status === 'Pending' ? 'selected' : ''}>待处理</option>
                                        <option value="TP" ${store.filters.status === 'TP' ? 'selected' : ''}>已处理</option>
                                        <option value="FP" ${store.filters.status === 'FP' ? 'selected' : ''}>非问题</option>
                                    </select>
                                </div>
                            </th>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider">
                                误报分析 / 备注
                            </th>
                        </tr>
                    </thead>
                    <tbody class="bg-white divide-y divide-gray-200">
                        ${tableHtml.length > 0 ? tableHtml : '<tr><td colspan="5" class="px-6 py-8 text-center text-sm text-gray-500">暂无符合条件的问题。</td></tr>'}
                    </tbody>
                </table>
            </div>
            
            <div class="mt-6 flex items-center justify-between">
                <div class="text-sm text-gray-600">
                    当前第 <span class="font-medium text-gray-900">${currentIssuePage}</span> 页，共 <span class="font-medium text-gray-900">${totalPages}</span> 页
                </div>
                <div class="flex gap-2">
                    <button id="prevPage" class="px-4 py-2 border border-gray-300 rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed" ${currentIssuePage <= 1 ? 'disabled' : ''}>上一页</button>
                    <button id="nextPage" class="px-4 py-2 border border-gray-300 rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed" ${currentIssuePage >= totalPages ? 'disabled' : ''}>下一页</button>
                </div>
            </div>
        `;
    };

    const renderFullView = () => {
        app.innerHTML = `
            <div class="mb-8 flex justify-between items-end">
                <div>
                    <h1 class="text-2xl font-bold text-gray-900 mb-2">所有代码检视问题</h1>
                    <p class="text-gray-500 text-sm">过滤并浏览所有 Sashiko 代码审查给出的问题，进行研判并记录反馈。</p>
                </div>
                <div class="text-gray-400">
                    <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"></path></svg>
                </div>
            </div>

            <div id="table-container">
                ${renderTable()}
            </div>
        `;

        attachFiltersAndListeners();
    };

    const attachFiltersAndListeners = () => {
        // Table listeners
        document.querySelectorAll('.comment-input').forEach(inp => {
            inp.addEventListener('change', (e) => {
                const id = e.target.getAttribute('data-id');
                store.updateFeedback(id, { comments: e.target.value });
            });
        });

        // Filter listeners
        const subFilter = document.getElementById('filter-subsystem');
        if (subFilter) {
            subFilter.addEventListener('change', (e) => {
                store.setFilter('subsystem', e.target.value);
                currentIssuePage = 1;
                document.getElementById('table-container').innerHTML = renderTable();
                attachFiltersAndListeners();
            });
        }

        const sevFilter = document.getElementById('filter-severity');
        if (sevFilter) {
            sevFilter.addEventListener('change', (e) => {
                store.setFilter('severity', e.target.value);
                currentIssuePage = 1;
                document.getElementById('table-container').innerHTML = renderTable();
                attachFiltersAndListeners();
            });
        }

        const statFilter = document.getElementById('filter-status');
        if (statFilter) {
            statFilter.addEventListener('change', (e) => {
                store.setFilter('status', e.target.value);
                currentIssuePage = 1;
                document.getElementById('table-container').innerHTML = renderTable();
                attachFiltersAndListeners();
            });
        }

        // Pagination
        const prevBtn = document.getElementById('prevPage');
        if (prevBtn) {
            prevBtn.addEventListener('click', () => {
                if (currentIssuePage > 1) {
                    currentIssuePage--;
                    document.getElementById('table-container').innerHTML = renderTable();
                    attachFiltersAndListeners();
                }
            });
        }

        const nextBtn = document.getElementById('nextPage');
        if (nextBtn) {
            nextBtn.addEventListener('click', () => {
                if (currentIssuePage < store.getTotalPages()) {
                    currentIssuePage++;
                    document.getElementById('table-container').innerHTML = renderTable();
                    attachFiltersAndListeners();
                }
            });
        }
    };

    renderFullView();
}