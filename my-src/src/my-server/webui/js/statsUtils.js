export function calculateAccuracy(tp, fp) {
    if (tp === 0 && fp === 0) return 0;
    const accuracy = (tp / (tp + fp)) * 100;
    return Number(accuracy.toFixed(2));
}

export function aggregateByWeek(findings) {
    const agg = {};
    findings.forEach(f => {
        if (!agg[f.week]) {
            agg[f.week] = { tp: 0, fp: 0 };
        }
        if (f.status === 'TP') agg[f.week].tp += 1;
        if (f.status === 'FP') agg[f.week].fp += 1;
    });

    for (const week in agg) {
        agg[week].accuracy = calculateAccuracy(agg[week].tp, agg[week].fp);
    }
    return agg;
}

export function aggregateBySubsystem(findings) {
    const agg = {};
    findings.forEach(f => {
        if (!agg[f.subsystem]) {
            agg[f.subsystem] = { tp: 0, fp: 0 };
        }
        if (f.status === 'TP') agg[f.subsystem].tp += 1;
        if (f.status === 'FP') agg[f.subsystem].fp += 1;
    });

    for (const sub in agg) {
        agg[sub].accuracy = calculateAccuracy(agg[sub].tp, agg[sub].fp);
    }
    return agg;
}
