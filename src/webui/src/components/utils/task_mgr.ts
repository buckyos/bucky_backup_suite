import { buckyos } from "buckyos";

export enum TaskState {
    RUNNING = "RUNNING",
    PENDING = "PENDING",
    PAUSED = "PAUSED",
    DONE = "DONE",
    FAILED = "FAILED",
}

export enum TaskType {
    BACKUP = "BACKUP",
    RESTORE = "RESTORE",
}

export interface TaskInfo {
    taskid: string;
    task_type: TaskType;
    owner_plan_id: string;
    checkpoint_id: string;
    total_size: number;
    completed_size: number;
    state: TaskState;
    error?: string;
    create_time: number; //unix timestamp
    update_time: number; //unix timestamp
    item_count: number;
    completed_item_count: number;
    wait_transfer_item_count: number;
    last_log_content: string | null;
    name: string; // todo
    speed: number; // B/s todo
}

export interface RestoreTaskInfo extends TaskInfo {
    restore_location_url: string;
    is_clean_restore: boolean;
}

export type PlanPolicyPeriod = {
    minutes: number; // 0-60*24
} & ({} | { week: number } | { date: number });

export interface PlanPolicyEvent {
    update_delay: number; // seconds
}

export type PlanPolicy = PlanPolicyPeriod | PlanPolicyEvent;

export enum SourceType {
    DIRECTORY = "DIRECTORY",
}

export enum BackupPlanType {
    C2C = "c2c"
}

export interface BackupPlanInfo {
    plan_id: string;
    title: string;
    description: string;
    type_str: BackupPlanType;
    last_checkpoint_index: number;
    source_type: SourceType;
    source: string;
    target: string;
    target_type: TargetType;
    target_name?: string;
    target_url?: string;
    last_run_time?: number; //unix timestamp (UTC)
    policy_disabled?: boolean;
    policy: PlanPolicy[];
    priority: number; // 0-10
    reserved_versions: number; // 0 means unlimited

    create_time: number; //unix timestamp
    update_time: number; //unix timestamp
    total_backup: number;
    total_size: number;
}

export enum TargetState {
    ONLINE = "ONLINE",
    OFFLINE = "OFFLINE",
    ERROR = "ERROR",
    UNKNOWN = "UNKNOWN",
}

export enum TargetType {
    LOCAL = "LOCAL",
    NDN = "NDN",
}

export interface BackupTargetInfo {
    target_id: string;
    target_type: TargetType;
    name: string;
    url: string;
    description: string;
    state: TargetState;
    used: number;
    total: number;
    last_error: string;
}

export interface TaskFilter {
    state?: TaskState[];
    type?: TaskType[];
    owner_plan_id?: string[];
    owner_plan_title?: string[];
}

export enum ListTaskOrderBy {
    CREATE_TIME = "create_time",
    UPDATE_TIME = "update_time",
    COMPLETE_TIME = "complete_time",
}

export enum ListOrder {
    ASC = "asc",
    DESC = "desc",
}

export enum TaskEventType {
    CREATE_PLAN = "create_plan",
    REMOVE_PLAN = "remove_plan",
    UPDATE_PLAN = "update_plan",

    CREATE_TARGET = "create_target",
    UPDATE_TARGET = "update_target",
    REMOVE_TARGET = "remove_target",
    CHANGE_TARGET_STATE = "change_target_state",

    CREATE_TASK = "create_task",
    UPDATE_TASK = "update_task",
    COMPLETE_TASK = "complete_task",
    FAIL_TASK = "fail_task",
    PAUSE_TASK = "pause_task",
    RESUME_TASK = "resume_task",
    REMOVE_TASK = "remove_task",
}

export interface DirectoryNode {
    name: string;
    isDirectory: boolean;
}

export enum DirectoryPurpose {
    BACKUP_SOURCE = "backup_source",
    RESTORE_TARGET = "restore_target",
    BACKUP_TARGET = "backup_target",
}

type BackupSubject =
    | { kind: "plan"; plan_id: string; plan_title: string }
    | { kind: "target"; target_id: string; name: string; target_url: string }
    | { kind: "task"; task_id: string; task_name: string; task_type: TaskType };

// 每种日志类型对应的参数表（扩展时只需在这里加键）
type BackupLogTypeMap = {
    create_plan: {};
    update_plan: {};
    run_plan: { task_id: string; task_name: string };
    run_success: { task_id: string; task_name: string };
    run_fail: { task_id: string; task_name: string; reason: string };
    remove_plan: {};
    create_target: {};
    update_target: {};
    remove_target: {};
    check_target: { old_state: TargetState; new_state: TargetState };
    create_task: {
        plan_id: string;
        plan_title: string;
        backup_task?: { id: string; task_name: string };
    };
    update_task: {
        plan_id: string;
        plan_title: string;
        backup_task?: { id: string; task_name: string };
    };
    pause_task: {
        plan_id: string;
        plan_title: string;
        backup_task?: { id: string; task_name: string };
    };
    resume_task: {
        plan_id: string;
        plan_title: string;
        backup_task?: { id: string; task_name: string };
    };
    task_success: {
        plan_id: string;
        plan_title: string;
        consume_size: number;
        backup_task?: { id: string; task_name: string };
    };
    task_fail: {
        plan_id: string;
        plan_title: string;
        reason: string;
        backup_task?: { id: string; task_name: string };
    };
    remove_task: {
        plan_id: string;
        plan_title: string;
        backup_task?: { id: string; task_name: string };
    };
    restore_backup: {
        plan_id: string;
        plan_title: string;
        restore_task: { id: string; task_name: string };
    };
    restore_success: {
        plan_id: string;
        plan_title: string;
        restore_task: { id: string; task_name: string };
    };
    restore_fail: {
        plan_id: string;
        plan_title: string;
        restore_task: { id: string; task_name: string };
        reason: string;
    };
    find_file: { path: string; size: number };
    hash_file: { path: string; hash: string };
    transfer_start: { path: string; size: number };
    transfer_success: { path: string; size: number; duration: number };
    transfer_fail: { path: string; reason: string };
};

// 通用日志结构（用泛型将 type 与 params 绑定）
type BackupLogEntry<T extends keyof BackupLogTypeMap = keyof BackupLogTypeMap> =
    {
        seq: number;
        timestamp: number;
        subject: BackupSubject;
        type: T;
        params: BackupLogTypeMap[T];
    };

// 全部日志的联合类型（用于变量、数组等）
export type BackupLog = {
    [K in keyof BackupLogTypeMap]: BackupLogEntry<K>;
}[keyof BackupLogTypeMap];

export class BackupTaskManager {
    private rpc_client: any;
    //可以关注task事件(全部task)
    private task_event_listeners: ((
        event: TaskEventType,
        data: any
    ) => void | Promise<void>)[] = [];

    private next_timer_id = 1;
    protected uncomplete_tasks: Map<
        string,
        TaskInfo & { last_query_time: number }
    > = new Map();
    private uncomplete_task_timer = {
        is_stop: true,
        listener_timers: new Set<number>(),
    };
    private targets: Map<string, BackupTargetInfo> = new Map();
    private target_timer = {
        is_stop: true,
        listener_timers: new Set<number>(),
    };

    constructor() {
        // Initialize RPC client for backup control service
        console.log("BackupTaskManager initialized");
        this.rpc_client = new buckyos.kRPCClient("/kapi/backup_control");
        this.task_event_listeners = [];
    }

    addTaskEventListener(
        listener: (event: TaskEventType, data: any) => void | Promise<void>
    ) {
        this.task_event_listeners.push(listener);
    }

    removeTaskEventListener(
        listener: (event: TaskEventType, data: any) => void | Promise<void>
    ) {
        this.task_event_listeners = this.task_event_listeners.filter(
            (l) => l !== listener
        );
    }

    async emitTaskEvent(event: TaskEventType, data: any) {
        // 使用 Promise.all 等待所有监听器执行完成
        await Promise.all(
            this.task_event_listeners.map((listener) => {
                try {
                    return Promise.resolve(listener(event, data));
                } catch (error) {
                    console.error("Error in task event listener:", error);
                    return Promise.resolve();
                }
            })
        );
    }

    async createBackupPlan(params: {
        type_str: BackupPlanType;
        source_type: SourceType;
        source: string;
        target_type: TargetType;
        target: string;
        title: string;
        description: string;
        policy: PlanPolicy[];
        priority: number;
    }): Promise<string> {
        const result = await this.rpc_client.call("create_backup_plan", params);
        result.type_str = params.type_str;
        result.source = params.source;
        result.target = params.target;
        result.title = params.title;
        result.description = params.description;
        await this.emitTaskEvent(TaskEventType.CREATE_PLAN, result);
        return result.plan_id;
    }

    async listBackupPlans(): Promise<string[]> {
        const result = await this.rpc_client.call("list_backup_plan", {});
        console.log("listBackupPlans: ", result);
        return result.backup_plans;
    }

    async getBackupPlan(planId: string): Promise<BackupPlanInfo> {
        const result = await this.rpc_client.call("get_backup_plan", {
            plan_id: planId,
        });
        result.plan_id = planId;
        console.log("getBackupPlan: ", result);
        return result;
    }

    async updateBackupPlan(planInfo: BackupPlanInfo): Promise<boolean> {
        const result = await this.rpc_client.call(
            "update_backup_plan",
            planInfo
        );
        await this.emitTaskEvent(TaskEventType.UPDATE_PLAN, result);
        return result.result === "success";
    }

    async removeBackupPlan(planId: string): Promise<boolean> {
        const result = await this.rpc_client.call("remove_backup_plan", {
            plan_id: planId,
        });
        await this.emitTaskEvent(TaskEventType.REMOVE_PLAN, result);
        return result.result === "success";
    }

    async createBackupTask(planId: string, parentCheckpointId?: string) {
        const params: any = { plan_id: planId };
        if (parentCheckpointId) {
            params.parent_checkpoint_id = parentCheckpointId;
        }
        const result = await this.rpc_client.call("create_backup_task", params);
        await this.emitTaskEvent(TaskEventType.CREATE_TASK, result);
        return result;
    }

    async createRestoreTask(
        planId: string,
        checkpointId: string,
        targetLocationUrl: string,
        is_clean_folder?: boolean
    ) {
        const params: any = {
            plan_id: planId,
            checkpoint_id: checkpointId,
            cfg: {
                restore_location_url: targetLocationUrl,
                is_clean_restore: is_clean_folder,
            },
        };

        const result = await this.rpc_client.call(
            "create_restore_task",
            params
        );
        await this.emitTaskEvent(TaskEventType.CREATE_TASK, result);
        return result;
    }

    async listBackupTasks(
        filter: TaskFilter = {},
        offset: number = 0,
        limit?: number,
        orderBy?: Array<[ListTaskOrderBy, ListOrder]>
    ): Promise<{ task_ids: string[]; total: number }> {
        const result = await this.rpc_client.call("list_backup_task", {
            filter: filter,
            offset: offset,
            limit: limit,
            order_by: orderBy,
        });
        return { task_ids: result.task_list, total: result.total };
    }

    async getTaskInfo(taskId: string): Promise<TaskInfo> {
        const result = await this.rpc_client.call("get_task_info", {
            taskid: taskId,
        });
        if (result) {
            if (result.state === TaskState.DONE) {
                if (this.uncomplete_tasks.has(taskId)) {
                    this.uncomplete_tasks.delete(taskId);
                    await this.emitTaskEvent(
                        TaskEventType.COMPLETE_TASK,
                        result
                    );
                }
            } else {
                const old_task = this.uncomplete_tasks.get(taskId);
                if (result.state === TaskState.FAILED) {
                    if (old_task && old_task.state !== TaskState.FAILED) {
                        await this.emitTaskEvent(
                            TaskEventType.FAIL_TASK,
                            result
                        );
                    }
                }
                const now = Date.now();
                let speed_im = old_task
                    ? ((result.completed_size - old_task.completed_size) *
                          1000) /
                      (now - old_task.last_query_time)
                    : 0;
                if (speed_im < 0) speed_im = 0;
                const speed_avg =
                    (old_task ? old_task.speed * 0.7 : 0) + speed_im * 0.3;
                result.speed = speed_avg;
                result.last_query_time = now;
                this.uncomplete_tasks.set(taskId, result);
            }
        }
        return result;
    }

    async resumeBackupTask(taskId: string) {
        const result = await this.rpc_client.call("resume_backup_task", {
            taskid: taskId,
        });
        await this.emitTaskEvent(TaskEventType.RESUME_TASK, taskId);
        return result.result === "success";
    }

    async pauseBackupTask(taskId: string) {
        const result = await this.rpc_client.call("pause_backup_task", {
            taskid: taskId,
        });
        return result.result === "success";
    }

    async removeBackupTask(taskId: string) {
        const result = await this.rpc_client.call("remove_backup_task", {
            taskid: taskId,
        });
        await this.emitTaskEvent(TaskEventType.REMOVE_TASK, taskId);
        return result.result === "success";
    }

    async validatePath(path: string) {
        const result = await this.rpc_client.call("validate_path", {
            path: path,
        });
        console.log(result);
        return result.path_exist;
    }

    async resume_last_working_task() {
        let taskid_list = await this.listBackupTasks({
            state: [TaskState.PAUSED],
        });
        if (taskid_list.task_ids.length > 0) {
            let last_task = taskid_list.task_ids[0];
            console.log("resume last task:", last_task);
            this.resumeBackupTask(last_task);
            await this.emitTaskEvent(TaskEventType.RESUME_TASK, last_task);
        }
    }

    async pause_all_tasks() {
        let taskid_list = await this.listBackupTasks({
            state: [TaskState.RUNNING, TaskState.PENDING],
        });
        for (let taskid of taskid_list.task_ids) {
            this.pauseBackupTask(taskid);
            await this.emitTaskEvent(TaskEventType.PAUSE_TASK, taskid);
        }
    }

    async listFilesInTask(
        taskId: string,
        subDir: string | null
    ): Promise<
        Array<{
            name: string;
            len: number;
            create_time: number;
            update_time: number;
            is_dir: boolean;
        }>
    > {
        const result = await this.rpc_client.call("list_files_in_task", {
            taskid: taskId,
            subdir: subDir,
        });
        return result.files;
    }

    async listChunksInFile(
        taskId: string,
        filePath: string
    ): Promise<
        Array<{
            chunkid: string;
            seq: string;
            size: number;
            status: string;
        }>
    > {
        const result = await this.rpc_client.call("list_chunks_in_file", {
            taskid: taskId,
            filepath: filePath,
        });
        return result.chunks;
    }

    async createBackupTarget(
        target_type: TargetType,
        target_url: string,
        name: string,
        config: any
    ): Promise<string> {
        const result = await this.rpc_client.call("create_backup_target", {
            target_type,
            url: target_url,
            name,
            config: config,
        });
        await this.emitTaskEvent(TaskEventType.CREATE_TARGET, result);
        return result.target_id;
    }

    async listBackupTargets(): Promise<string[]> {
        const result = await this.rpc_client.call("list_backup_target", {});
        return result.targets;
    }

    async getBackupTarget(targetId: string): Promise<BackupTargetInfo> {
        const result = await this.rpc_client.call("get_backup_target", {
            target_id: targetId,
        });
        result.target_id = targetId;
        // compare with old state, if changed, emit event
        const old_target = this.targets.get(targetId);
        if (old_target) {
            this.targets.set(targetId, result);
            if (old_target.state !== result.state) {
                this.emitTaskEvent(TaskEventType.CHANGE_TARGET_STATE, {
                    targetId,
                    oldState: old_target.state,
                    newState: result.state,
                });
            }
        } else {
            this.targets.set(targetId, result);
        }
        return result;
    }

    async updateBackupTarget(targetInfo: BackupTargetInfo): Promise<boolean> {
        const result = await this.rpc_client.call(
            "update_backup_target",
            targetInfo
        );
        await this.emitTaskEvent(TaskEventType.UPDATE_TARGET, result);
        return result.result === "success";
    }

    async removeBackupTarget(targetId: string): Promise<boolean> {
        const result = await this.rpc_client.call("remove_backup_target", {
            target_id: targetId,
        });
        await this.emitTaskEvent(TaskEventType.REMOVE_TARGET, result);
        return result.result === "success";
    }

    async consumeSizeSummary(): Promise<{ total: number; today: number }> {
        const result = await this.rpc_client.call("consume_size_summary", {});
        return result;
    }

    async statisticsSummary(
        from: number,
        to: number
    ): Promise<{ complete: number; failed: number }> {
        const result = await this.rpc_client.call("statistics_summary", {
            from: from,
            to: to,
        });
        return result;
    }

    startRefreshUncompleteTaskStateTimer(): number {
        let timer_id = this.next_timer_id++;
        this.uncomplete_task_timer.listener_timers.add(timer_id);
        if (this.uncomplete_task_timer.is_stop) {
            this.uncomplete_task_timer.is_stop = false;
            callInInterval(
                async () => {
                    try {
                        let taskid_list = await this.listBackupTasks({
                            state: [
                                TaskState.RUNNING,
                                TaskState.PENDING,
                                TaskState.PAUSED,
                                TaskState.FAILED,
                            ],
                        });
                        await Promise.all(
                            taskid_list.task_ids.map((taskid) =>
                                this.getTaskInfo(taskid)
                            )
                        );
                        // remove tasks that are no longer uncomplete
                        for (const taskid of this.uncomplete_tasks.keys()) {
                            if (!taskid_list.task_ids.includes(taskid)) {
                                const comp_task =
                                    this.uncomplete_tasks.get(taskid);
                                if (
                                    comp_task &&
                                    comp_task.state !== TaskState.DONE
                                ) {
                                    await this.emitTaskEvent(
                                        TaskEventType.COMPLETE_TASK,
                                        comp_task
                                    );
                                }
                                this.uncomplete_tasks.delete(taskid);
                            }
                        }
                    } catch (error) {
                        console.error(
                            "Error refreshing uncomplete task state:",
                            error
                        );
                    }
                },
                1000,
                (_) => {
                    return this.uncomplete_task_timer.is_stop;
                }
            );
        }
        return timer_id;
    }

    stopRefreshUncompleteTaskStateTimer(timerId: number) {
        this.uncomplete_task_timer.listener_timers.delete(timerId);
        if (this.uncomplete_task_timer.listener_timers.size === 0) {
            this.uncomplete_task_timer.is_stop = true;
        }
    }

    startRefreshTargetStateTimer(): number {
        let timer_id = this.next_timer_id++;
        this.target_timer.listener_timers.add(timer_id);
        if (this.target_timer.is_stop) {
            this.target_timer.is_stop = false;
            callInInterval(
                async () => {
                    try {
                        let target_ids = await this.listBackupTargets();
                        await Promise.all(
                            target_ids.map((target_id) =>
                                this.getBackupTarget(target_id)
                            )
                        );
                        // remove targets that are no longer present
                        for (const target_id of this.targets.keys()) {
                            if (!target_ids.includes(target_id)) {
                                this.targets.delete(target_id);
                            }
                        }
                    } catch (error) {
                        console.error("Error refreshing target state:", error);
                    }
                },
                1000,
                (_) => {
                    return this.target_timer.is_stop;
                }
            );
        }
        return timer_id;
    }

    stopRefreshTargetStateTimer(timerId: number) {
        this.target_timer.listener_timers.delete(timerId);
        if (this.target_timer.listener_timers.size === 0) {
            this.target_timer.is_stop = true;
        }
    }

    async listDirChildren(
        path?: string,
        purpose?: DirectoryPurpose,
        options?: {
            only_dirs?: boolean;
            only_files?: boolean;
        }
    ): Promise<DirectoryNode[]> {
        return this.rpc_client.call("list_directory_children", { path: path });
    }

    async listLogs(
        offset: number,
        limit: number,
        orderBy: ListOrder,
        subject?:
            | { plan_id: string }
            | { target_id: string }
            | { task_id: string }
    ): Promise<{ logs: BackupLog[]; total: number }> {
        const result = await this.rpc_client.call("list_logs", {
            offset: offset,
            limit: limit,
            subject: subject,
        });
        return { logs: result.logs, total: result.total };
    }
}

export function callInInterval(
    callback: () => Promise<void>,
    interval: number,
    setIntervalHandle: (intervalHandle: number | null) => boolean
) {
    const timerDisable = true;
    if (!timerDisable) {
        let isStop = false;
        let intervalHandle: number | undefined;
        const tick = async () => {
            if (isStop) return;
            await callback();
            if (intervalHandle) {
                clearInterval(intervalHandle);
            }
            intervalHandle = window.setInterval(tick, interval);
            isStop = setIntervalHandle(intervalHandle);
        };
        tick();
    }
}

// Export a singleton instance
export const taskManager = new BackupTaskManager();
