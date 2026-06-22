export class IssueStore {
    constructor(initialFindings = []) {
        this.findings = JSON.parse(JSON.stringify(initialFindings));
        this.filters = {
            subsystem: 'All',
            severity: 'All',
            status: 'All',
            server_id: 'All'
        };
        this.pageSize = 10;
    }

    setFilter(key, value) {
        if (this.filters.hasOwnProperty(key)) {
            this.filters[key] = value;
        }
    }

    getFindings() {
        return this.findings.filter(f => {
            if (this.filters.subsystem !== 'All' && f.subsystem !== this.filters.subsystem) return false;
            if (this.filters.severity !== 'All' && f.severity !== this.filters.severity) return false;
            if (this.filters.status !== 'All' && f.status !== this.filters.status) return false;
            if (this.filters.server_id !== 'All' && f.server_id !== parseInt(this.filters.server_id)) return false;
            return true;
        });
    }

    updateFeedback(id, updates) {
        const finding = this.findings.find(f => f.id === id);
        if (finding) {
            if (updates.status !== undefined) finding.status = updates.status;
            if (updates.comments !== undefined) finding.comments = updates.comments;
        }
    }

    setPageSize(size) {
        this.pageSize = size;
    }

    getPage(pageNumber) {
        const filtered = this.getFindings();
        const start = (pageNumber - 1) * this.pageSize;
        return filtered.slice(start, start + this.pageSize);
    }

    getTotalPages() {
        const filtered = this.getFindings();
        return Math.ceil(filtered.length / this.pageSize);
    }
}
