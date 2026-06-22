import { getMockStats, mockFindings } from '../mockData.js';
import { IssueStore } from '../issueStore.js';

const store = new IssueStore(mockFindings);
store.setPageSize(10);
let currentIssuePage = 1;

export async function renderStatsView() {
    const app = document.getElementById('app');
    const stats = getMockStats();

    const renderTable = () => {
        // Just show 5 recent findings
        const pageFindings = store.getPage(1).slice(0, 5);

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
                <table class="min-w-full divide-y divide-gray-200">
                    <thead class="bg-gray-50">
                        <tr>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider w-1/5">
                                源代码链接
                            </th>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider w-32">
                                严重等级
                            </th>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider w-1/4">
                                AI审查建议 (子系统)
                            </th>
                            <th scope="col" class="px-6 py-3 text-left text-xs font-semibold text-gray-600 uppercase tracking-wider w-40">
                                处理意见
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
        `;
    };

    app.innerHTML = `
        <div class="mb-6">
            <h1 class="text-2xl font-bold text-gray-900">问题处理统计</h1>
            <p class="text-gray-600">问题处理准确率趋势与问题清单概览。</p>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
            <div class="bg-white rounded-lg shadow p-6 border border-gray-200">
                <h3 class="text-sm font-medium text-gray-500 uppercase tracking-wider">总体准确率</h3>
                <div class="mt-2 flex items-baseline gap-2">
                    <span class="text-3xl font-bold text-indigo-600">${stats.overall.accuracy}%</span>
                </div>
                <p class="text-sm text-gray-500 mt-1">TP: ${stats.overall.tp} | FP: ${stats.overall.fp}</p>
            </div>
            <div class="bg-white rounded-lg shadow p-6 border border-gray-200">
                <h3 class="text-sm font-medium text-gray-500 uppercase tracking-wider">总有效(TP)</h3>
                <div class="mt-2 flex items-baseline gap-2">
                    <span class="text-3xl font-bold text-green-600">${stats.overall.tp}</span>
                </div>
            </div>
            <div class="bg-white rounded-lg shadow p-6 border border-gray-200">
                <h3 class="text-sm font-medium text-gray-500 uppercase tracking-wider">总误报(FP)</h3>
                <div class="mt-2 flex items-baseline gap-2">
                    <span class="text-3xl font-bold text-red-600">${stats.overall.fp}</span>
                </div>
            </div>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
            <div class="bg-white rounded-lg shadow p-6 border border-gray-200">
                <h3 class="text-lg font-medium text-gray-900 mb-4">按周准确率趋势</h3>
                <div id="weeklyChart" class="h-80 w-full"></div>
            </div>
            <div class="bg-white rounded-lg shadow p-6 border border-gray-200">
                <h3 class="text-lg font-medium text-gray-900 mb-4">按子系统准确率</h3>
                <div id="subsystemChart" class="h-80 w-full"></div>
            </div>
        </div>

        <div class="mb-6 flex justify-between items-center">
            <div>
                <h2 class="text-xl font-bold text-gray-900">最近问题清单概览</h2>
                <p class="text-gray-600">查看并跟进最近问题状态。</p>
            </div>
            <a href="#/issues" class="text-indigo-600 hover:text-indigo-800 text-sm font-medium">查看全部问题 &rarr;</a>
        </div>

        <div id="table-container">
            ${renderTable()}
        </div>
    `;

    const attachTableListeners = () => {
        document.querySelectorAll('.comment-input').forEach(inp => {
            inp.addEventListener('change', (e) => {
                const id = e.target.getAttribute('data-id');
                store.updateFeedback(id, { comments: e.target.value });
            });
        });
    };

    attachTableListeners();

    // Initialize ECharts
    setTimeout(() => {
        const weeklyChart = echarts.init(document.getElementById('weeklyChart'));
        weeklyChart.setOption({
            tooltip: { trigger: 'axis' },
            xAxis: { type: 'category', data: stats.weekly.map(w => w.week) },
            yAxis: { type: 'value', max: 100, name: 'Accuracy (%)' },
            series: [{
                data: stats.weekly.map(w => w.accuracy),
                type: 'line',
                smooth: true,
                itemStyle: { color: '#4f46e5' },
                areaStyle: {
                    color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
                        { offset: 0, color: 'rgba(79, 70, 229, 0.5)' },
                        { offset: 1, color: 'rgba(79, 70, 229, 0)' }
                    ])
                }
            }]
        });

        const subsystemChart = echarts.init(document.getElementById('subsystemChart'));
        subsystemChart.setOption({
            tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
            xAxis: { type: 'value', max: 100, name: 'Accuracy (%)' },
            yAxis: { type: 'category', data: stats.subsystems.map(s => s.subsystem) },
            series: [{
                type: 'bar',
                data: stats.subsystems.map(s => s.accuracy),
                itemStyle: { color: '#4f46e5' }
            }]
        });

        const handleResize = () => {
            if (!document.getElementById('weeklyChart')) {
                window.removeEventListener('resize', handleResize);
                return;
            }
            weeklyChart.resize();
            subsystemChart.resize();
        };
        window.addEventListener('resize', handleResize);
    }, 0);
}
