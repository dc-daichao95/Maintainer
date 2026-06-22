const fs = require('fs');
const path = require('path');

const indexPath = path.resolve('index.html');
let content = fs.readFileSync(indexPath, 'utf-8');

// 1. Add <script type="module" src="js/app.js"></script> before the main script
content = content.replace(/<script>/, '<script type="module" src="js/app.js"></script>\n    <script>');

// 2. Add 'issues' route to router
const routerCaseRegex = /case 'stats':\s*await renderStatsView\(\);\s*break;/;
if (routerCaseRegex.test(content)) {
    content = content.replace(routerCaseRegex, `case 'stats':\n                        await renderStatsView();\n                        break;\n                    case 'issues':\n                        if (window.renderIssuesViewMock) await window.renderIssuesViewMock();\n                        break;`);
}

// 3. Parse hash for 'issues'
const parseHashRegex = /if \(hash === '#\/stats'\) \{\s*return \{ view: 'stats', query \};\s*\}/;
content = content.replace(parseHashRegex, `if (hash === '#/stats') {\n                return { view: 'stats', query };\n            }\n            if (hash === '#/issues') {\n                return { view: 'issues', query };\n            }`);

// 4. Overwrite renderStatsView to use the mock version if available
const renderStatsViewRegex = /async function renderStatsView\(\) \{[\s\S]*?\}\s*router\(\);/m;
content = content.replace(renderStatsViewRegex, `async function renderStatsView() {
            if (window.renderStatsViewMock) {
                return window.renderStatsViewMock();
            }
            const app = document.getElementById('app');
            app.innerHTML = '<div class="p-8 text-center text-gray-500">Stats view not implemented</div>';
        }

        router();`);

fs.writeFileSync(indexPath, content);
console.log("Injected hooks into index.html");
