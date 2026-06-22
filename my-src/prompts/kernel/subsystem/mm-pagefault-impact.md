# Page Fault 性能影响检视指南

涉及内存回收、页缓存驻留、页表填充或缺页处理链路的补丁，可能在不引入传统正确性缺陷的情况下，改变 **缺页发生频率** 与 **单次缺页耗时**。此类性能回退常在内存压力、混合负载或大映射区域下才显现，常规功能检视容易遗漏。

**每个变更块必须回答的两个核心问题：**
1. 该变更是否使合法访问更容易在页表或页缓存中未命中（提高缺页 **次数**）？
2. 该变更是否在缺页 **路径** 上增加工作、阻塞、分配或锁竞争（提高单次缺页 **时延**）？

将发现分类为 **缺页频率（fault-rate）** 或 **缺页时延（fault-latency）**，标明方向（增加/减少/不确定），并说明机制（换出、解除映射、PTE 状态、swap-in、锁、分配等）。

---

## 缺页类型分类（统计与时延分析的基础）

在判断频率或时延影响前，必须先识别缺页 **类型**。

| 类别 | 典型触发 | 主要开销 | 关键路径 |
|------|----------|----------|----------|
| 轻微缺页（按需零页 / COW 断开） | 首次访问、对共享/干净页的写缺页 | PTE 安装、anon/文件页分配、`folio_get` | `do_anonymous_page()`、`do_cow_fault()`（`mm/memory.c`） |
| 文件后备缺页 | `PROT_NONE` / 缺失的文件 PTE | `filemap_fault()`、预读、`i_rwsem` | `filemap_fault()`（`mm/filemap.c`）、`address_space_operations` 的 `->fault` |
| Swap-in 缺页 | 存在 swap PTE | 块 IO、swap cache、`get_swap_device` | `do_swap_page()`、`swapin_readahead()`（`mm/memory.c`、`mm/swap_state.c`） |
| 写缺页 / mkwrite | 可写映射、脏页跟踪 | `page_mkwrite`、文件系统 freeze、额外 PTE 工作 | `do_wp_page()`、`do_page_mkwrite()` |
| NUMA 提示缺页 | `CONFIG_NUMA_BALANCING` | 迁移准备、远端内存 | `do_numa_page()`、`change_pte_range()`（`mm/mprotect.c`） |
| THP 缺页 / 折叠 | PMD 空 vs PTE 已填充 | 分裂/折叠、`khugepaged`、回退 | `create_huge_pmd()`、`__collapse_huge_page_*`（`mm/huge_memory.c`） |
| 设备 / 迁移 / uffd | 非 present 的 swap 型 PTE | 额外分支、阻塞交接 | `handle_pte_fault()` 分发（`mm/memory.c`） |

- **频率（rate）**：更多访问落入上述某一类路径（尤其 major / swap-in / 文件后备缺页）。
- **时延（latency）**：路径类型不变，但单次 `handle_mm_fault()` 调用做更多工作。

统一入口：`handle_mm_fault()`（`mm/memory.c`）→ `__handle_mm_fault()` → `handle_pte_fault()`。

---

## 缺页频率：页缓存换出与驻留

提高回收积极性、缩短缓存驻留时间、或解除已缓存页的映射，会增加后续访问时 **major** 与 **文件后备** 缺页的概率。此类变更可能是有意为之，但必须说明理由并评估边界。

**会提高缺页频率的典型机制：**

| 变更类别 | 缺页增多的原因 | 需追踪的文件 / 符号 |
|----------|----------------|---------------------|
| 更激进的 LRU 回收 | 热 file/anon folio 更早被换出 | `shrink_lruvec()`、`evict_folios()`、`scan_folios()`（`mm/vmscan.c`） |
| 有效缓存容量变小 | watermark / cgroup 压力更早触发回收 | `balance_pgdat()`、`try_to_wake_kswapd()`、`mem_cgroup_reclaim()` |
| swappiness / anon 与 file 平衡 | 更多 anon 换出 → 更多 swap 缺页 | `get_swappiness()`、`shrink_anon()` |
| 回写 / 脏页节流 | 脏页驻留更久，或刷盘后从 cache 丢弃 | `balance_dirty_pages()`、`wb_writeback()`（`mm/page-writeback.c`） |
| truncate / punch / fadvise | PTE 被 zap、cache 页移除 | `truncate_inode_pages_range()`、`madvise_dontneed()`、`zap_page_range()` |
| MADV_FREE / lazyfree | 干净+可写 PTE 被回收且无需 writeback；写时 refault | `madvise_free_pte_range()`（`mm/madvise.c`） |
| 预读减少或关闭 | 顺序访问变冷 cache | `page_cache_sync_readahead()`、`filemap_read()` |
| shmem / swap cache 策略 | folio 进入 swap 侧 → swap-in 缺页 | `shmem_swapout()`、`add_to_swap()`（`mm/swapfile.c`） |
| MGLRU 代际 / tier 调整 | 错误驱逐顺序：冷页留下、热页换出 | `lru_gen_*`、`folio_inc_gen()`（`mm/vmscan.c`） |

**应作为性能问题报告（REPORT）** 的情形：
- 在未说明工作负载前提下，降低驱逐阈值、扫描优先级，或削弱 active/file LRU 保护。
- 在回收循环（`shrink_folio_list`）内增加 per-folio 工作，降低有效回收吞吐 → **内存压力持续时间变长** → 缺页频率持续偏高。
- 改变 `folio_clear_referenced()` / young 位清除逻辑，使热页被误判为冷页（错误换出）。
- 在用户态可频繁触发的路径（ioctl、sysfs、缺页重试）上引入无条件 `zap_*` / `invalidate_*`。

**双路径一致性（MGLRU vs 经典回收）：** `shrink_inactive_list()` 的变更须在 `evict_folios()` 中对称体现（参见上游 `mm-reclaim.md`）。仅改一侧时，缺页频率变化可能只在某一种回收模式下出现。

---

## 缺页频率：PTE 状态与映射生命周期

PTE 与 VMA 变更可在不释放物理内存的情况下强制 refault。

- 清除 **young** 或 **accessed** 位（`pte_mkold`、`clear_young_dirty_ptes`）会为 writenotify 或回收记账带来 refault；在 MADV_FREE 等路径上合理，在通用路径上危险。
- **`pte_protnone()` / PROT_NONE**：映射仍存在但访问必缺页；若 NUMA balancing 同时作用可能 double-fault（`change_pte_range()` 会跳过已有 protnone）。
- **THP 分裂** 或强制 PTE 级映射（相对 PMD 映射）会增加每次访问的缺页次数，直至再次折叠（`split_huge_pmd()`、khugepaged）。
- **COW / fork**：打破共享会在写时增加每进程缺页。
- **换出且无 cache**：anon 进 swap；shmem 从 page cache 进 swap cache（xarray 模型不同——判断 refault 路径须用 `folio_test_swapcache()`，勿用 `folio_test_swapbacked()` 代替）。

**应作为性能问题报告**：原先 present PTE 或驻留 page cache folio 即可满足的 **只读热路径**，补丁后变为必须缺页。

---

## 缺页时延：处理路径与锁

在 `handle_mm_fault()` 及其子调用中增加工作，直接提高 **单次缺页时延**（微秒级到 swap/IO 的毫秒级）。

**缺页路径上的高时延操作（应避免或严格限定）：**

| 操作 | 风险 | 常见位置 |
|------|------|----------|
| `mmap_write_lock` 升级或长时间持有 | 阻塞其他缺页与 mmap 操作 | `mm/rmap.c` 中缺页与 `mmap_lock` 规则 |
| `folio_lock` + 路径内 IO | 阻塞并行访问者 | `filemap_fault`、swap-in |
| 缺页上下文中直接回收 | `__alloc_pages` 带 `__GFP_DIRECT_RECLAIM` | `do_anonymous_page`、THP 分配 |
| `cond_resched()` / 睡眠 | 仅允许在缺页路径明确允许处；原子上下文禁止 | `filemap_fault`、大 folio migrate 回调 |
| 额外 `flush_tlb_*` | TLB shootdown IPI 风暴 | unmap 竞态后的 PTE 安装 |
| `i_rwsem` / `sb_start_write` | 每次缺页的文件系统争用 | `filemap_page_mkwrite`、`generic_perform_write` |
| userfaultfd / migration entry | 额外分支与等待 | `handle_userfault`、`do_swap_page` |
| 热路径追踪 / 统计 | 无条件开销 | `trace_mm_*`、`perf_sw_event` |

**缺页路径锁约束**（参见上游 `mm-pagetable.md`）：`->fault` / `->page_mkwrite` 在 `mmap_read_lock` 下运行，嵌套于 `i_rwsem`、`sb_start_write` 之下。缺页处理程序不得等待 freeze 保护（ABBA）。须在锁外重试处使用 `copy_folio_from_iter_atomic()` 等模式。

**应作为缺陷 / 性能回退报告**：
- 在用户态可触达的 `handle_pte_fault()` 分支中新增阻塞、`GFP_KERNEL` 分配或 mutex。
- `migrate_folio` / `filemap_migrate_folio` 回调在持 spinlock 时跨越 `folio_mc_copy()`（大 folio 会睡眠）。
- 每次缺页新增未批处理的 `find_get_entry` / xarray 遍历。
- `VM_FAULT_RETRY` 重试环在无进展上限时，争用下出现时延尖峰。

---

## 缺页时延：Swap-in 与块 IO

Swap 与文件后备缺页常主导尾时延。

- **`do_swap_page()`**：`read_swap_cache_async()`、swap 预读、设备查找（`get_swap_device()` / `put_swap_device()` 须配对）。
- **跨设备预读**：须分别 pin 各设备的 `swap_info_struct`；错误引用 → 跳过预读或额外缺页（`swap_vma_readahead()`，`mm/swap_state.c`）。
- **Swap entry ABA**：未在加锁后重新校验 folio 身份时，快速路径失败与重试增多。
- **块层**：队列深度、plug、ioprio 变更间接影响缺页驱动的读（不在 mm/ 内，由缺页触发）。

**应报告**：补丁在原先由预读摊销的路径上，改为每次缺页同步 IO 或元数据查找。

---

## 缺页时延：分配与回收交互

缺页处理会分配 folio、PTE 页、anon 槽；分配失败会在 **缺页上下文** 触发回收，成倍放大时延。

- 缺页路径上 `GFP_KERNEL` vs `GFP_ATOMIC` / `GFP_NOWAIT`（参见上游 `mm.md` GFP 表）。
- **`mem_cgroup_charge`** 失败 → 回收循环；错误 memcg → 错误域被节流（参见 `mm-reclaim.md` cgroup writeback 域规则）。
- **THP 分配回退**：PMD 缺页失败 → PTE 级安装 → 后续更多缺页（`VM_FAULT_FALLBACK`、`finish_fault()`）。
- **Mempool / slab**：缺页路径 kmalloc 须匹配上下文。

**应报告**：原先使用预分配、缓存或 zero page 快速路径的缺页路径，补丁后新增分配或 charge。

---

## 工作负载敏感的检视方法

对每个变更函数回答：

1. **热路径还是冷路径？** 每 CPU 中断、系统调用缺页、mmap 热循环 vs 一次性管理 ioctl。
2. **缺页类型是否迁移？** 是否将工作从后台（kswapd、writeback、khugepaged）挪到 **同步缺页**？
3. **是否放大？** 每次缺页 O(n) 扫描、无 cluster 上界的 per-PTE 循环、新的重试环。
4. **是否有度量依据？** 若声称性能改进，应见 ftrace、`perf stat`（page-faults、major-faults）、vmstat（`pgfault`、`pgmajfault`、`pgscan_*`、`workingset_*`）或 commit message 中的 benchmark 说明。

**每条发现的输出格式：**
- **类型**：fault-rate | fault-latency
- **机制**：一句话（例：「memcg 压力下 file LRU 换出更积极」）
- **证据**：函数/符号与 hunk 行为
- **严重程度**：用户可见时延 | 吞吐 | 仅内存压力场景 | 不确定
- **建议**：回退保护、批处理、挪到异步、增加 sysctl、文档化权衡

禁止无代码路径依据的臆测；结论须能对应到具体变更。

---

## 关联子系统指南

补丁触及下列领域时，应与本指南一并加载（上游 `third_party/prompts/kernel/subsystem/`）：

| 领域 | 文件 |
|------|------|
| 回收、swap、writeback | mm-reclaim.md |
| PTE 安装、zap、THP 缺页 | mm-pagetable.md |
| 页缓存、folio、预读 | mm-folio.md |
| mmap_lock、VMA 标志 | mm-vma.md |
| THP / hugetlb 缺页锁 | mm-largepage.md |

---

## 快速检查清单（Quick Checks）

- **换出无驻留说明**：加快回收或收紧 cgroup 限额，但未说明谁承担 refault 成本。
- **热路径上的 zap**：`zap_page_range`、`unmap_mapping_range`、`truncate` 可从缺页重试或高频 syscall 触达。
- **PTE young/dirty 操纵**：为 writenotify 或 LRU 带来更多 refault。
- **缺页路径睡眠**：在 `FAULT_FLAG_*` 禁止睡眠的上下文中加锁、文件系统或回收睡眠。
- **MGLRU/经典回收不一致**：只改一侧回收路径。
- **Swap-in 串行化**：原先有预读，现为每缺页 `get_swap_device` 或同步读。
- **THP 折叠/分裂抖动**：PMD ↔ PTE 振荡提高缺页次数。
- **对已缺页 VMA 做 NUMA balancing**：每次访问额外 minor fault。
- **`handle_mm_fault` 内无条件 trace/stat**：热路径常驻开销。
- **重试风暴**：`VM_FAULT_RETRY` + 新争用源（新锁或 IO）。

---

## 检视焦点（可选：作为 `--custom-prompt` 前言）

你必须优先评估缺页 **性能影响**，而非风格类意见。对每个变更判断：是否提高缺页 **频率**（尤其换出后的 major/文件/swap 缺页），或提高缺页处理 **时延**（锁、分配、IO、额外 PTE 工作）。仅报告有补丁代码依据、且能说明机制与受影响缺页路径的发现。除非同时改变缺页行为，否则将一般正确性问题延后处理。

---

## Sashiko 索引触发词（供 `subsystem.md` 或 Phase 0 选用）

`handle_mm_fault`、`__handle_mm_fault`、`handle_pte_fault`、`do_page_fault`、`filemap_fault`、`do_swap_page`、`do_anonymous_page`、`VM_FAULT_`、`pgfault`、`pgmajfault`、`shrink_*`、`evict_folios`、`balance_dirty`、`try_to_unmap`、`zap_pte`、`madvise_`、`readahead`、`mm/memory.c`、`mm/filemap.c`、`mm/vmscan.c`
