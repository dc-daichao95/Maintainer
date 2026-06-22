import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { calculateAccuracy, aggregateByWeek, aggregateBySubsystem } from '../../../src/my-server/webui/js/statsUtils.js';

describe('Stats Utilities', () => {
    describe('calculateAccuracy', () => {
        it('should return 0 when both tp and fp are 0', () => {
            assert.strictEqual(calculateAccuracy(0, 0), 0);
        });

        it('should calculate correct accuracy', () => {
            assert.strictEqual(calculateAccuracy(80, 20), 80);
            assert.strictEqual(calculateAccuracy(100, 0), 100);
            assert.strictEqual(calculateAccuracy(0, 100), 0);
            assert.strictEqual(calculateAccuracy(1, 2), 33.33);
        });
    });

    describe('aggregateByWeek', () => {
        it('should aggregate findings by week correctly', () => {
            const findings = [
                { id: 1, week: '2026-W20', status: 'TP' },
                { id: 2, week: '2026-W20', status: 'FP' },
                { id: 3, week: '2026-W21', status: 'TP' },
                { id: 4, week: '2026-W21', status: 'TP' },
            ];
            const result = aggregateByWeek(findings);
            assert.deepStrictEqual(result, {
                '2026-W20': { tp: 1, fp: 1, accuracy: 50 },
                '2026-W21': { tp: 2, fp: 0, accuracy: 100 },
            });
        });
    });

    describe('aggregateBySubsystem', () => {
        it('should aggregate findings by subsystem correctly', () => {
            const findings = [
                { id: 1, subsystem: 'net', status: 'TP' },
                { id: 2, subsystem: 'net', status: 'FP' },
                { id: 3, subsystem: 'usb', status: 'TP' },
                { id: 4, subsystem: 'usb', status: 'TP' },
            ];
            const result = aggregateBySubsystem(findings);
            assert.deepStrictEqual(result, {
                'net': { tp: 1, fp: 1, accuracy: 50 },
                'usb': { tp: 2, fp: 0, accuracy: 100 },
            });
        });
    });
});
