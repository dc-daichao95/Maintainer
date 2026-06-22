import { router, registerView } from './router.js?v=20260607_1';

// Import Views
import { renderListView } from './views/listView.js?v=20260607_1';
import { renderPatchsetView } from './views/patchsetView.js';
import { renderMessageView } from './views/messageView.js';
import { renderLogView } from './views/logView.js';
import { renderBaselineLogView } from './views/baselineLogView.js';
import { renderStatsView } from './views/statsView.js';
import { renderIssuesView } from './views/issuesView.js?v=20260607_1';
import { renderSettingsView } from './views/settingsView.js';
import { renderServerConfigView } from './views/serverConfigView.js?v=20260607_1';
import { renderDashboardView } from './views/dashboardView.js?v=20260607_5';

// Register views
registerView('list', renderListView);
registerView('patchset', renderPatchsetView);
registerView('message', renderMessageView);
registerView('log', renderLogView);
registerView('baseline_log', renderBaselineLogView);
registerView('stats', renderStatsView);
registerView('issues', renderIssuesView);
registerView('settings', renderSettingsView);
registerView('server-config', renderServerConfigView);
registerView('dashboard', renderDashboardView);

// Initialize router
router();
