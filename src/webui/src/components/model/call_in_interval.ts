export function callInInterval(
    callback: () => Promise<void>,
    interval: number,
    setIntervalHandle: (intervalHandle: number | null) => boolean
) {
    let isStop = false;
    const tick = async () => {
        if (isStop) return;
        await callback();
        const intervalHandle = window.setInterval(tick, interval);
        isStop = setIntervalHandle(intervalHandle);
    };
    tick();
}
