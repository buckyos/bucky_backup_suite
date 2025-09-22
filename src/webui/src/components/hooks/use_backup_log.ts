import { useState, useEffect, useRef } from "react";
import {
    BackupLogReader,
    LogRecord,
    LogTypeFilter,
    ReadLogBegin,
} from "../model/backup_log_reader";
import { callInInterval } from "../model/call_in_interval";

export function useLogReader(
    typeFilter: LogTypeFilter[],
    begin: ReadLogBegin,
    limit: number,
    order: "asc" | "desc" = "desc"
) {
    const [nextBegin, setNextBegin] = useState<ReadLogBegin>(begin);
    const [logs, setLogs] = useState<LogRecord[]>([]);
    const timerRef = useRef<number | null>(null);
    const [isStopped, setIsStopped] = useState(false);

    useEffect(() => {
        const reader = new BackupLogReader();
        callInInterval(
            async () => {
                try {
                    const appendLogs = await reader.read(
                        typeFilter,
                        nextBegin,
                        limit,
                        order
                    );
                    if (appendLogs.length > 0) {
                        setLogs([...logs, ...appendLogs]);
                        setNextBegin({
                            by: "last_seq",
                            last_seq:
                                appendLogs[appendLogs.length - 1].seq
                        });
                    }
                } catch (error) {
                    console.error("Error reading logs:", error);
                }
            },
            1000,
            (intervalHandle) => {
                timerRef.current = intervalHandle;
                return isStopped;
            }
        );

        return () => {
            if (timerRef.current) {
                window.clearInterval(timerRef.current);
                timerRef.current = null;
            }
            setIsStopped(true);
        };
    }, []);
}
