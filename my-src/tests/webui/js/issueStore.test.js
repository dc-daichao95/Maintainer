import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { IssueStore } from '../../../src/my-server/webui/js/issueStore.js';

describe('Issue Store', () => {
    let store;
    const mockFindings = [
        { id: 'f1', subsystem: 'net', severity: 'High', status: 'Pending', comments: '' },
        { id: 'f2', subsystem: 'usb', severity: 'Medium', status: 'TP', comments: 'Confirmed' },
        { id: 'f3', subsystem: 'net', severity: 'Low', status: 'FP', comments: 'False alarm' },
    ];

    beforeEach(() => {
        store = new IssueStore(mockFindings);
    });

    it('should initialize with all findings', () => {
        assert.strictEqual(store.getFindings().length, 3);
    });

    it('should filter by subsystem', () => {
        store.setFilter('subsystem', 'net');
        const filtered = store.getFindings();
        assert.strictEqual(filtered.length, 2);
        assert.strictEqual(filtered[0].id, 'f1');
        assert.strictEqual(filtered[1].id, 'f3');
    });

    it('should filter by multiple criteria', () => {
        store.setFilter('subsystem', 'net');
        store.setFilter('status', 'Pending');
        const filtered = store.getFindings();
        assert.strictEqual(filtered.length, 1);
        assert.strictEqual(filtered[0].id, 'f1');
    });

    it('should update feedback status and comments', () => {
        store.updateFeedback('f1', { status: 'TP', comments: 'Fixing now' });
        const finding = store.getFindings().find(f => f.id === 'f1');
        assert.strictEqual(finding.status, 'TP');
        assert.strictEqual(finding.comments, 'Fixing now');
    });

    it('should paginate correctly', () => {
        store = new IssueStore([...mockFindings, { id: 'f4', subsystem: 'net', severity: 'High', status: 'Pending', comments: '' }]);
        store.setPageSize(2);
        
        let page1 = store.getPage(1);
        assert.strictEqual(page1.length, 2);
        assert.strictEqual(page1[0].id, 'f1');
        
        let page2 = store.getPage(2);
        assert.strictEqual(page2.length, 2);
        assert.strictEqual(page2[0].id, 'f3');
        
        assert.strictEqual(store.getTotalPages(), 2);
    });
});
