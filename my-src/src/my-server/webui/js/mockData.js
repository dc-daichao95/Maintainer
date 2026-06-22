export const mockFindings = [
    { id: 'ISSUE-001', server_id: 1, source_server: 'Server A', rule_id: 'NULL_DEREF', file_path: 'net/ipv4/tcp.c', line_number: 125, subsystem: 'net', severity: 'High', status: 'TP', comments: 'Confirmed null dereference in tcp_v4_rcv.' },
    { id: 'ISSUE-002', server_id: 2, source_server: 'Server B', rule_id: 'MEM_LEAK', file_path: 'drivers/usb/core/devio.c', line_number: 450, subsystem: 'usb', severity: 'Medium', status: 'Pending', comments: '' },
    { id: 'ISSUE-003', server_id: 1, source_server: 'Server A', rule_id: 'LOCK_ORDER', file_path: 'fs/ext4/inode.c', line_number: 890, subsystem: 'ext4', severity: 'High', status: 'FP', comments: 'Lock is acquired in caller.' },
    { id: 'ISSUE-004', server_id: 3, source_server: 'Server C', rule_id: 'USE_AFTER_FREE', file_path: 'net/core/dev.c', line_number: 310, subsystem: 'net', severity: 'Critical', status: 'TP', comments: 'UAF in dev_queue_xmit.' },
    { id: 'ISSUE-005', server_id: 1, source_server: 'Server A', rule_id: 'NULL_DEREF', file_path: 'drivers/net/ethernet/intel/e1000e/netdev.c', line_number: 1102, subsystem: 'net', severity: 'High', status: 'Pending', comments: '' },
    { id: 'ISSUE-006', server_id: 2, source_server: 'Server B', rule_id: 'MEM_LEAK', file_path: 'kernel/bpf/syscall.c', line_number: 201, subsystem: 'bpf', severity: 'Medium', status: 'TP', comments: 'Map memory leak on error path.' },
    { id: 'ISSUE-007', server_id: 3, source_server: 'Server C', rule_id: 'RACE_COND', file_path: 'mm/page_alloc.c', line_number: 4400, subsystem: 'mm', severity: 'High', status: 'FP', comments: 'Protected by zone lock.' },
    { id: 'ISSUE-008', server_id: 1, source_server: 'Server A', rule_id: 'NULL_DEREF', file_path: 'drivers/gpu/drm/i915/i915_drv.c', line_number: 550, subsystem: 'drm', severity: 'High', status: 'TP', comments: 'Missing check before deref.' },
    { id: 'ISSUE-009', server_id: 2, source_server: 'Server B', rule_id: 'LOCK_ORDER', file_path: 'net/ipv6/udp.c', line_number: 88, subsystem: 'net', severity: 'Medium', status: 'Pending', comments: '' },
    { id: 'ISSUE-010', server_id: 1, source_server: 'Server A', rule_id: 'USE_AFTER_FREE', file_path: 'fs/btrfs/extents.c', line_number: 1200, subsystem: 'btrfs', severity: 'Critical', status: 'TP', comments: 'Confirmed UAF.' },
    { id: 'ISSUE-011', server_id: 3, source_server: 'Server C', rule_id: 'MEM_LEAK', file_path: 'drivers/usb/host/xhci.c', line_number: 300, subsystem: 'usb', severity: 'Low', status: 'FP', comments: 'Freed in cleanup function.' },
    { id: 'ISSUE-012', server_id: 2, source_server: 'Server B', rule_id: 'NULL_DEREF', file_path: 'net/bluetooth/hci_core.c', line_number: 770, subsystem: 'bluetooth', severity: 'High', status: 'Pending', comments: '' },
];

mockFindings.forEach((f, idx) => {
    const weekNum = 20 + (idx % 4);
    f.week = `2026-W${weekNum}`;
});

export function getMockStats() {
    return {
        overall: { tp: 5, fp: 3, accuracy: 62.5 },
        weekly: [
            { week: '2026-W20', tp: 2, fp: 0, accuracy: 100 },
            { week: '2026-W21', tp: 1, fp: 1, accuracy: 50 },
            { week: '2026-W22', tp: 1, fp: 1, accuracy: 50 },
            { week: '2026-W23', tp: 1, fp: 1, accuracy: 50 },
        ],
        subsystems: [
            { subsystem: 'net', tp: 2, fp: 0, accuracy: 100 },
            { subsystem: 'usb', tp: 0, fp: 1, accuracy: 0 },
            { subsystem: 'ext4', tp: 0, fp: 1, accuracy: 0 },
            { subsystem: 'bpf', tp: 1, fp: 0, accuracy: 100 },
            { subsystem: 'mm', tp: 0, fp: 1, accuracy: 0 },
            { subsystem: 'drm', tp: 1, fp: 0, accuracy: 100 },
            { subsystem: 'btrfs', tp: 1, fp: 0, accuracy: 100 },
        ]
    };
}
