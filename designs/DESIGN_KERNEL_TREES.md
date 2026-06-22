# Design: Linux Kernel Tree Hierarchy & Sashiko Integration

## Objective
To map the complex landscape of Linux kernel repositories hosted on `kernel.org`. This document serves as the reference for `sashiko` to determine the correct **Git Baseline** for applying and reviewing patches. Unlike a standard software project with a single `main` branch, the Linux kernel operates on a distributed hierarchy of trees.

## 1. The Tree Landscape

### 1.1 Mainline (Linus's Tree)
*   **URL**: `git://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git`
*   **Role**: The ultimate source of truth. Releases (e.g., `v6.12`, `v6.13-rc1`) are cut from here.
*   **Content**:
    *   **Merge Window (2 weeks)**: Ingests changes from *Subsystem Trees*.
    *   **Stabilization Phase (~7-8 weeks)**: Only accepts bug fixes (mostly).
*   **Sashiko Relevance**:
    *   Default fallback for almost all patches if specific subsystem trees are ambiguous.
    *   Primary target for patches labeled "Fixes" during the stabilization phase.

### 1.2 Linux-Next
*   **URL**: `git://git.kernel.org/pub/scm/linux/kernel/git/next/linux-next.git`
*   **Role**: The integration testing ground. It is rebuilt fresh every day by merging ~100+ subsystem trees onto the latest Mainline.
*   **Characteristics**:
    *   **Transient**: History is rewritten daily. Tags are preserved (e.g., `next-20250105`).
    *   **Conflict Resolution**: This is where merge conflicts between different subsystems are detected.
*   **Sashiko Relevance**:
    *   Ideal for checking if a patch conflicts with *upcoming* changes.
    *   **Warning**: Patches usually target a specific subsystem tree, *not* linux-next directly. Applying a patch to `linux-next` is good for "future-proofing" checks but might fail if the patch depends on a stable baseline.

### 1.3 Subsystem Trees
The kernel is divided into subsystems (Networking, BPF, USB, Scheduler, etc.), each with its own maintainer and repository.

#### Types of Subsystem Branches:
1.  **`*-next` (e.g., `net-next`, `bpf-next`)**:
    *   Queues new features for the *next* merge window.
    *   *Sashiko Action*: Use this baseline for patches adding new features or non-critical refactoring.
2.  **Standard/Fixes (e.g., `net`, `bpf`)**:
    *   Queues urgent bug fixes for the *current* release cycle.
    *   *Sashiko Action*: Use this baseline for patches tagged `[PATCH net]` or clearly identified as fixes.

#### Key Examples:
*   **Networking**: `git://git.kernel.org/pub/scm/linux/kernel/git/netdev/net.git` / `net-next.git`
*   **BPF**: `git://git.kernel.org/pub/scm/linux/kernel/git/bpf/bpf.git` / `bpf-next.git`
*   **Tip (x86, Sched, Locking)**: `git://git.kernel.org/pub/scm/linux/kernel/git/tip/tip.git`
*   **Block**: `git://git.kernel.org/pub/scm/linux/kernel/git/axboe/linux-block.git`
*   **GregKH (USB, Driver Core, Staging)**: `git://git.kernel.org/pub/scm/linux/kernel/git/gregkh/usb.git`, etc.

### 1.4 Stable / Longterm
*   **URL**: `git://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git`
*   **Role**: Maintenance of older kernels (6.1.y, 5.15.y, etc.).
*   **Sashiko Relevance**:
    *   Target for patches explicitly sent to `stable@vger.kernel.org`.
    *   Target for "Backport" requests.

### 1.5 Linux-Next History
*   **URL**: `git://git.kernel.org/pub/scm/linux/kernel/git/next/linux-next-history.git`
*   **Role**: An archive preserving the state of `linux-next` releases over time (since `linux-next` itself rewrites history).
*   **Sashiko Relevance**: Primarily for archeology or training data; unlikely to be a direct baseline for active reviews.

## 2. Baseline Resolution Strategy

To accurately review a patch, Sashiko must guess which tree the author intended. Using the wrong tree leads to `git apply` failures.

### Logic Flow
1.  **Explicit Tag Detection**:
    *   Subject: `[PATCH bpf-next]` -> `bpf-next` repo.
    *   Subject: `[PATCH net]` -> `net` repo.
    *   Subject: `[PATCH 6.1]` -> Stable 6.1 branch.

2.  **Heuristic Detection (The "Get Maintainer" approach)**:
    *   Analyze `To:` and `Cc:` headers.
    *   Map mailing lists to subsystem trees (e.g., `netdev@vger.kernel.org` -> `net-next` by default, unless strict "Fixes" logic applies).
    *   *Implementation Note*: We need a mapping table (DB or Config) linking Mailing Lists -> Git URLs.

3.  **File Path Analysis**:
    *   If `To/Cc` is ambiguous, check touched files against `MAINTAINERS` file rules to find the custodian tree.

4.  **Fallback**:
    *   `torvalds/linux.git` (Mainline).

## 3. Storage Optimization (Git Alternates)

Cloning 50 subsystems is expensive (disk/network).

**Architecture**:
1.  **Reference Repo**: Maintain a bare clone of `torvalds/linux.git`.
2.  **Subsystem Remotes**: Instead of separate clones, add subsystems as *remotes* to the Reference Repo or use `git clone --reference` / `git alternates`.
3.  **Fetch Strategy**:
    *   Sashiko should maintain one "Mega-Repo" with remotes for `net`, `bpf`, `tip`, `gregkh`, etc.
    *   When a review is requested for `bpf-next`, fetch that remote and checkout a worktree.
    *   This reduces storage from ~5GB * N to ~5GB + Deltas.

## 4. Summary of URLs

| Tree | URL (git.kernel.org/...) | Purpose |
| :--- | :--- | :--- |
| **Mainline** | `pub/scm/linux/kernel/git/torvalds/linux.git` | Universal Baseline |
| **Next** | `pub/scm/linux/kernel/git/next/linux-next.git` | Integration Checks |
| **Stable** | `pub/scm/linux/kernel/git/stable/linux.git` | Backports |
| **Net** | `pub/scm/linux/kernel/git/netdev/net.git` | Networking Fixes |
| **Net-Next** | `pub/scm/linux/kernel/git/netdev/net-next.git` | Networking Features |
| **BPF** | `pub/scm/linux/kernel/git/bpf/bpf.git` | BPF Fixes |
| **BPF-Next** | `pub/scm/linux/kernel/git/bpf/bpf-next.git` | BPF Features |
| **Tip** | `pub/scm/linux/kernel/git/tip/tip.git` | x86, Sched, Core |
