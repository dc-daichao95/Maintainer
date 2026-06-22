# 自定义检视 Prompt（my-src）

本目录存放 Kernel-Maintainer 项目专用的检视知识库，与上游 `third_party/prompts` 隔离，便于与开源 Sashiko 同步。

## 文件

| 路径 | 说明 |
|------|------|
| `kernel/subsystem/mm-pagefault-impact.md` | Page fault 次数与时延性能影响检视指南（中文） |

## 使用方式

1. **复制到 Sashiko prompt 目录**（若检视引擎读取 `third_party/prompts`）：
   ```text
   third_party/prompts/kernel/subsystem/mm-pagefault-impact.md
   ```
   并在 `subsystem.md` 索引表中增加对应一行（触发词见该文件末尾）。

2. **单次检视附加焦点**：将 `mm-pagefault-impact.md` 末尾「检视焦点」段落作为 `--custom-prompt` 传入（需运行环境已接线 `custom_prompt`）。

3. **与上游 MM 指南叠加**：常与 `mm-reclaim.md`、`mm-pagetable.md`、`mm-folio.md` 一并加载。

## 格式约定

遵循上游 `subsystem-template.md`：按概念分节、段首说明违反后果、表格与 **REPORT** 标记、文末 Quick Checks；不包含 TodoWrite 工作流步骤。
