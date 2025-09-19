export enum BackupLogType {
    PLAN_LOG = "plan",
    TARGET_LOG = "target",
    TASK_LOG = "task",
    FILE_LOG = "file",
    CHUNK_LOG = "chunk",
}

export interface BackupLogBase {
    seq: number;
    timestamp: number; // unix timestamp in milliseconds(UTC)
    type: BackupLogType;
}
export interface PlanLog extends BackupLogBase {
    id: string;
    name: string;
    action: "add" | "update" | "delete";
    update?: Array<{
        field: string;
        from: any;
        to: any;
    }>;
}

export interface TargetLog extends BackupLogBase {
    id: string;
    name: string;
    logs: string[];
}

export interface TaskLog extends BackupLogBase {
    id: string;
    name: string;
    action:
        | "add"
        | "start"
        | "pause"
        | "resume"
        | "delete"
        | "complete"
        | "fail";
    fail_msg?: string;
}

export interface FileLog extends BackupLogBase {
    task_id: string;
    file_path: string;
    action: "scan" | "transfer" | "complete" | "fail" | "skip" | "cover";
    file_id?: string;
    fail_msg?: string;
}

export interface ChunkLog extends BackupLogBase {
    task_id: string;
    file_path: string;
    chunk_seq: number;
    action: "transfer" | "complete" | "fail";
    chunk_id: string;
}

export type LogRecord = PlanLog | TargetLog | TaskLog | FileLog | ChunkLog;

export interface LogTypeFilter {
    type: BackupLogType;
    plan_id?: string;
    task_id?: string;
    target_id?: string;
    file_path?: string;
    chunk_id?: string;
}

export interface ReadLogBegin {
    by: "last_seq" | "timestamp";
    last_seq?: number;
    timestamp?: number; // unix timestamp in milliseconds(UTC)
}

export class BackupLogReader {
    constructor() {}

    public async read(
        typeFilter: LogTypeFilter[],
        begin: ReadLogBegin,
        limit: number,
        order: "asc" | "desc" = "desc"
    ): Promise<LogRecord[]> {
        // todo: implement reading log file and parsing logs
        return [];
    }
}
