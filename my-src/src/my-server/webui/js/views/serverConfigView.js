import { escapeHtml } from '../utils.js';

// Define global functions for the view
window.openAddServerModal = () => {
    document.getElementById('modalTitle').innerText = '添加服务器';
    document.getElementById('addServerForm').reset();
    document.getElementById('serverId').value = '';
    document.getElementById('addServerModal').classList.remove('hidden');
};

window.editServer = (btn) => {
    const dataset = btn.dataset;
    document.getElementById('modalTitle').innerText = '编辑服务器';
    document.getElementById('serverId').value = dataset.id;
    document.getElementById('serverName').value = dataset.name;
    document.getElementById('serverIp').value = dataset.ip;
    document.getElementById('serverWebPort').value = dataset.port;
    document.getElementById('serverDescription').value = dataset.desc;
    document.getElementById('addServerModal').classList.remove('hidden');
};

window.closeAddServerModal = () => {
    document.getElementById('addServerModal').classList.add('hidden');
    document.getElementById('addServerForm').reset();
};

window.submitAddServer = async (e) => {
    e.preventDefault();
    const id = document.getElementById('serverId').value;
    const name = document.getElementById('serverName').value;
    const ip = document.getElementById('serverIp').value;
    const web_port = parseInt(document.getElementById('serverWebPort').value);
    const description = document.getElementById('serverDescription').value;
    
    try {
        let url = '/api/v1/servers';
        let method = 'POST';
        if (id) {
            url = `/api/v1/servers/${id}`;
            method = 'PUT';
        }

        const res = await fetch(url, {
            method: method,
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, ip, web_port, description })
        });

        if (res.ok) {
            window.closeAddServerModal();
            renderServerConfigView();
        } else {
            throw new Error('Server returned not OK');
        }
    } catch (err) {
        alert('Failed to save server: ' + err.message);
    }
};

window.deleteServer = async (id) => {
    if (!confirm('Are you sure you want to delete this server?')) return;
    try {
        const res = await fetch(`/api/v1/servers/${id}`, {
            method: 'DELETE'
        });
        if (res.ok) {
            renderServerConfigView();
        } else {
            throw new Error('Server returned not OK');
        }
    } catch (err) {
        alert('Failed to delete server: ' + err.message);
    }
};

export async function renderServerConfigView() {
    const app = document.getElementById('app');
    app.innerHTML = '<div class="p-8 text-center text-gray-500">Loading Server Config...</div>';

    try {
        let servers = [];
        try {
            const response = await fetch('/api/v1/servers');
            if (!response.ok) throw new Error('Not OK');
            servers = await response.json();
        } catch (e) {
            console.error(e);
        }

        let tableRows = '';
        if (servers.length === 0) {
            tableRows = '<tr><td colspan="6" class="px-6 py-4 text-center text-gray-500">No servers configured.</td></tr>';
        } else {
            servers.forEach(server => {
                tableRows += `
                    <tr class="hover:bg-gray-50 border-b border-gray-100">
                        <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">${escapeHtml(server.name)}</td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">${escapeHtml(server.ip)}</td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">${server.web_port}</td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">${escapeHtml(server.description || '')}</td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                            <span class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-gray-100 text-gray-800">
                                未探测
                            </span>
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
                            <button data-id="${server.id}" data-name="${escapeHtml(server.name)}" data-ip="${escapeHtml(server.ip)}" data-port="${server.web_port}" data-desc="${escapeHtml(server.description || '')}" onclick="window.editServer(this)" class="text-indigo-600 hover:text-indigo-900 mr-3">编辑</button>
                            <button onclick="window.deleteServer(${server.id})" class="text-red-600 hover:text-red-900">删除</button>
                        </td>
                    </tr>
                `;
            });
        }

        app.innerHTML = `
            <div class="max-w-6xl mx-auto">
                <div class="flex justify-between items-center mb-6">
                    <h3 class="text-xl font-medium text-gray-800">服务器配置</h3>
                    <button onclick="window.openAddServerModal()" class="bg-indigo-600 hover:bg-indigo-700 text-white px-4 py-2 rounded shadow-sm text-sm font-medium transition-colors">
                        + 添加服务器
                    </button>
                </div>
                <div class="bg-white rounded-lg shadow-sm overflow-hidden border border-gray-200">
                    <table class="min-w-full divide-y divide-gray-200">
                        <thead class="bg-gray-50">
                            <tr>
                                <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">名称</th>
                                <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">服务器地址(IP)</th>
                                <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Web 端口</th>
                                <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">描述</th>
                                <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">状态</th>
                                <th scope="col" class="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">操作</th>
                            </tr>
                        </thead>
                        <tbody class="bg-white divide-y divide-gray-200">
                            ${tableRows}
                        </tbody>
                    </table>
                </div>
            </div>

            <!-- Add/Edit Server Modal -->
            <div id="addServerModal" class="hidden fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center">
                <div class="bg-white p-6 rounded-lg w-full max-w-md">
                    <h3 class="text-lg font-medium mb-4" id="modalTitle">添加服务器</h3>
                    <p class="text-xs text-gray-500 mb-4">无需 SSH 用户/密码，仅用于打开远端 sashiko 与拉取统计数据</p>
                    <form id="addServerForm" onsubmit="window.submitAddServer(event)">
                        <input type="hidden" id="serverId">
                        <div class="mb-4">
                            <label class="block text-sm font-medium text-gray-700 mb-1">名称</label>
                            <input type="text" id="serverName" required class="w-full border border-gray-300 rounded px-3 py-2">
                        </div>
                        <div class="mb-4">
                            <label class="block text-sm font-medium text-gray-700 mb-1">服务器地址(IP)</label>
                            <input type="text" id="serverIp" required class="w-full border border-gray-300 rounded px-3 py-2">
                        </div>
                        <div class="mb-4">
                            <label class="block text-sm font-medium text-gray-700 mb-1">Web 端口</label>
                            <input type="number" id="serverWebPort" value="8080" required class="w-full border border-gray-300 rounded px-3 py-2">
                        </div>
                        <div class="mb-6">
                            <label class="block text-sm font-medium text-gray-700 mb-1">描述</label>
                            <input type="text" id="serverDescription" required class="w-full border border-gray-300 rounded px-3 py-2">
                        </div>
                        <div class="flex justify-end gap-3">
                            <button type="button" onclick="window.closeAddServerModal()" class="px-4 py-2 border border-gray-300 rounded text-gray-700 hover:bg-gray-50">取消</button>
                            <button type="submit" class="px-4 py-2 bg-indigo-600 text-white rounded hover:bg-indigo-700">保存</button>
                        </div>
                    </form>
                </div>
            </div>
        `;

    } catch (err) {
        app.innerHTML = `<div class="p-8 text-center text-red-500">Error loading server config: ${escapeHtml(err.message)}</div>`;
    }
}