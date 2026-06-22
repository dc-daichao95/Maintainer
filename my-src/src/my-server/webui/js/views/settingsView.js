export async function renderSettingsView() {
    const app = document.getElementById('app');
    app.innerHTML = `
        <div class="mb-6">
            <h1 class="text-2xl font-bold text-gray-900">设置</h1>
            <p class="text-gray-600">系统偏好设置与配置。</p>
        </div>
        <div class="bg-white rounded-lg shadow p-6 border border-gray-200">
            <p class="text-gray-500">当前没有可用的设置选项 (Mock UI)。</p>
        </div>
    `;
}
