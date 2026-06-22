import { escapeHtml } from '../utils.js';

export async function renderDashboardView() {
    const app = document.getElementById('app');
    
    app.innerHTML = '<div class="p-8 text-center text-gray-500">Loading Dashboard...</div>';

    try {
        let stats;
        try {
            const statsRes = await fetch('/api/v1/dashboard/stats');
            if (!statsRes.ok) throw new Error('Not OK');
            stats = await statsRes.json();
        } catch (e) {
            console.error(e);
            app.innerHTML = `<div class="p-8 text-center text-red-500">Error loading dashboard: ${escapeHtml(e.message)}</div>`;
            return;
        }

        let serverCardsHtml = '';
        if (stats.servers && stats.servers.length > 0) {
            stats.servers.forEach(server => {
                const statusBadge = server.online 
                    ? '<div class="px-3 py-1 rounded-full bg-emerald-50 text-emerald-600 text-xs font-semibold">在线</div>'
                    : '<div class="px-3 py-1 rounded-full bg-red-50 text-red-600 text-xs font-semibold">离线</div>';
                
                serverCardsHtml += `
                    <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-6 flex flex-col gap-4 hover:shadow-md transition-shadow cursor-pointer" onclick="window.open('http://${encodeURIComponent(server.ip)}:${server.web_port}', '_blank')">
                        <div class="flex justify-between items-center w-full">
                            <div class="text-base font-semibold text-gray-900">${escapeHtml(server.name)}</div>
                            ${statusBadge}
                        </div>
                        <div class="flex flex-col gap-2">
                            <div class="flex items-center gap-2 text-indigo-600">
                                <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01"></path></svg>
                                <span class="text-sm font-medium">${escapeHtml(server.ip)}:${server.web_port}</span>
                            </div>
                            <div class="flex items-center gap-2 text-gray-500">
                                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                                <span class="text-sm w-full">${escapeHtml(server.description || '')}</span>
                            </div>
                        </div>
                        <div class="flex gap-4 pt-4 mt-2 border-t border-gray-200 w-full">
                            <span class="text-sm font-medium text-gray-400">点击卡片打开 sashiko ↗</span>
                        </div>
                    </div>
                `;
            });
        } else {
            serverCardsHtml = '<div class="col-span-full text-center text-gray-500 py-8">暂无配置服务器，请前往服务器配置添加。</div>';
        }

        let accuracyStr = '0.0%';
        if (typeof stats.avg_accuracy === 'number' && !isNaN(stats.avg_accuracy)) {
            accuracyStr = (stats.avg_accuracy * 100).toFixed(1) + '%';
        }

        app.innerHTML = `
            <div class="max-w-6xl mx-auto space-y-8 pb-12">
                <div class="flex justify-between items-center bg-white p-6 rounded-lg shadow-sm border border-gray-200">
                    <div class="flex flex-col gap-1">
                        <h1 class="text-2xl font-semibold text-gray-900">概览</h1>
                        <p class="text-sm text-gray-500">监控所有服务器节点的运行状态、IP及描述信息。</p>
                    </div>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                    ${serverCardsHtml}
                </div>

                <div class="grid grid-cols-1 md:grid-cols-4 gap-6">
                    <div class="bg-white rounded-lg shadow-sm p-6 border border-gray-200">
                        <div class="text-sm font-medium text-gray-500 mb-1">总检视 Patch 数</div>
                        <div class="text-3xl font-bold text-gray-900">${typeof stats.total_patchsets === 'number' ? stats.total_patchsets : 0}</div>
                    </div>
                    <div class="bg-white rounded-lg shadow-sm p-6 border border-gray-200">
                        <div class="text-sm font-medium text-gray-500 mb-1">总告警数</div>
                        <div class="text-3xl font-bold text-gray-900">${stats.total_issues}</div>
                    </div>
                    <div class="bg-white rounded-lg shadow-sm p-6 border border-gray-200">
                        <div class="text-sm font-medium text-gray-500 mb-1">平均准确率</div>
                        <div class="text-3xl font-bold text-gray-900">${accuracyStr}</div>
                    </div>
                    <div class="bg-white rounded-lg shadow-sm p-6 border border-gray-200">
                        <div class="text-sm font-medium text-gray-500 mb-1">在线服务器</div>
                        <div class="text-3xl font-bold text-gray-900">${stats.online_servers}</div>
                    </div>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                    <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                        <h3 class="text-lg font-medium text-gray-800 mb-4">各服务器检视趋势</h3>
                        <div id="trendChart" style="height: 300px;"></div>
                    </div>
                    <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                        <h3 class="text-lg font-medium text-gray-800 mb-4">各服务器告警分布</h3>
                        <div id="serverChart" style="height: 300px;"></div>
                    </div>
                </div>
            </div>
        `;

        // Dispose old charts if they exist
        if (window.__dashboardTrendChart) {
            window.__dashboardTrendChart.dispose();
        }
        if (window.__dashboardServerChart) {
            window.__dashboardServerChart.dispose();
        }

        window.__dashboardTrendChart = echarts.init(document.getElementById('trendChart'));
        const days = stats.trend_days || [];
        // One stacked-area series per server, matching the per-server overview design.
        const trendSeries = (stats.trend_series || []).map(s => ({
            name: s.name,
            type: 'line',
            stack: 'Total',
            smooth: true,
            areaStyle: {},
            emphasis: { focus: 'series' },
            data: s.counts || []
        }));

        window.__dashboardTrendChart.setOption({
            tooltip: { 
                trigger: 'axis',
                axisPointer: { type: 'cross', label: { backgroundColor: '#6a7985' } }
            },
            legend: { bottom: 0, data: trendSeries.map(s => s.name) },
            grid: { left: '3%', right: '4%', bottom: '15%', containLabel: true },
            xAxis: [
                {
                    type: 'category',
                    boundaryGap: false,
                    data: days
                }
            ],
            yAxis: [ { type: 'value' } ],
            series: trendSeries
        });

        window.__dashboardServerChart = echarts.init(document.getElementById('serverChart'));
        const pieData = stats.pie_chart_data ? stats.pie_chart_data.map(d => ({ name: d.name, value: d.value })) : [];
        
        window.__dashboardServerChart.setOption({
            tooltip: { trigger: 'item' },
            legend: { top: '5%', left: 'center' },
            series: [{
                name: 'Alerts',
                type: 'pie',
                radius: ['40%', '70%'],
                avoidLabelOverlap: false,
                itemStyle: {
                    borderRadius: 10,
                    borderColor: '#fff',
                    borderWidth: 2
                },
                label: { show: false, position: 'center' },
                emphasis: {
                    label: { show: true, fontSize: 20, fontWeight: 'bold' }
                },
                labelLine: { show: false },
                data: pieData
            }]
        });

        if (!window.__dashboardResizeBound) {
            window.addEventListener('resize', () => {
                if (window.__dashboardTrendChart) window.__dashboardTrendChart.resize();
                if (window.__dashboardServerChart) window.__dashboardServerChart.resize();
            });
            window.__dashboardResizeBound = true;
        }

    } catch (err) {
        app.innerHTML = `<div class="p-8 text-center text-red-500">Error loading dashboard: ${escapeHtml(err.message)}</div>`;
    }
}