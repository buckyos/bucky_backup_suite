import { MOCK_FILE_SYSTEM_SPEC, MockFsSpec } from "./mock_file_system";
import {
    BackupLog,
    BackupPlanInfo,
    BackupPlanType,
    BackupTargetInfo,
    BackupTaskManager,
    DirectoryNode,
    DirectoryPurpose,
    ListOrder,
    ListTaskOrderBy,
    PlanPolicy,
    SourceType,
    TargetState,
    TargetType,
    TaskEventType,
    TaskFilter,
    TaskInfo,
    TaskState,
    TaskType,
} from "./task_mgr";

type TaskChunk = {
    chunkid: string;
    seq: string;
    size: number;
    status: string;
};

interface TaskFile {
    name: string;
    isDirectory: boolean;
    size?: number;
    modifiedTime?: number;
    children?: Array<TaskFile | TaskChunk>;
}

export class FakeTaskManager extends BackupTaskManager {
    private plan_list = {
        next_plan_id: 1,
        plans: new Array<BackupPlanInfo>(),
    };
    private task_list = {
        next_task_id: 1,
        tasks: new Array<TaskInfo & { root: string }>(),
    };
    private target_list = {
        next_target_id: 1,
        targets: new Array<BackupTargetInfo>(),
    };
    private files_system_tree: TaskFile = {
        name: "/",
        isDirectory: true,
        children: [],
    };
    private logs: BackupLog[] = [];
    private nextLogSeq = 1;
    private taskUpdateTimer?: number;
    private taskTickInFlight = false;
    private taskProgressMilestones: Map<string, number> = new Map();

    constructor() {
        super();
        this.seedMockData();
        this.startTaskSimulationTimer();
    }

    private seedMockData() {
        const chunkCounter = { value: 1 };
        this.files_system_tree = buildTaskTree(
            MOCK_FILE_SYSTEM_SPEC,
            chunkCounter
        );

        const now = Date.now();

        const addTarget = (
            info: Omit<BackupTargetInfo, "target_id">
        ): BackupTargetInfo => {
            const target: BackupTargetInfo = {
                ...info,
                target_id: `target_${this.target_list.next_target_id++}`,
            };
            this.target_list.targets.push(target);
            return target;
        };

        const primaryTarget = addTarget({
            target_type: TargetType.FILE,
            url: "file:///mnt/backup/nas",
            name: "Office NAS",
            description: "Primary network storage for workstation backups",
            state: TargetState.ONLINE,
            used: 320 * 1024 * 1024 * 1024,
            total: 512 * 1024 * 1024 * 1024,
            last_error: "",
        });

        const ndnTarget = addTarget({
            target_type: TargetType.NDN,
            url: "ndn://backup/edge-cluster",
            name: "NDN Edge Cluster",
            description: "Distributed edge backup nodes (NDN)",
            state: TargetState.OFFLINE,
            used: 120 * 1024 * 1024 * 1024,
            total: 1_024 * 1024 * 1024 * 1024,
            last_error: "Remote peer unreachable (retry scheduled)",
        });

        const portableTarget = addTarget({
            target_type: TargetType.FILE,
            url: "file:///media/usb/vault",
            name: "Portable Vault",
            description: "Rotating USB drive for monthly exports",
            state: TargetState.UNKNOWN,
            used: 18 * 1024 * 1024 * 1024,
            total: 128 * 1024 * 1024 * 1024,
            last_error: "",
        });

        const planSeeds: Array<{
            title: string;
            description: string;
            source: string;
            target: BackupTargetInfo;
            policy: PlanPolicy[];
            priority: number;
            reserved_versions: number;
        }> = [
            {
                title: "Workstation Documents",
                description:
                    "User documents and spreadsheets from the workstation profile",
                source: "C:/Users/Administrator/Documents",
                target: primaryTarget,
                policy: [{ minutes: 120 }],
                priority: 6,
                reserved_versions: 5,
            },
            {
                title: "Media Archive",
                description: "High quality media assets stored on the D: drive",
                source: "D:/Media",
                target: primaryTarget,
                policy: [{ minutes: 240 }],
                priority: 4,
                reserved_versions: 4,
            },
            {
                title: "Project Sync",
                description:
                    "Active project repositories synchronized to the NDN edge cluster",
                source: "D:/Projects",
                target: ndnTarget,
                policy: [{ minutes: 180 }],
                priority: 7,
                reserved_versions: 6,
            },
            {
                title: "Analytics Snapshot",
                description:
                    "Database snapshots for analytics workloads copied to the portable vault",
                source: "D:/Data",
                target: portableTarget,
                policy: [{ minutes: 360 }],
                priority: 5,
                reserved_versions: 3,
            },
        ];

        const statsByPlan = new Map<
            string,
            { totalSize: number; fileCount: number }
        >();

        const plans: BackupPlanInfo[] = [];

        for (const seed of planSeeds) {
            const planId = `plan_${this.plan_list.next_plan_id++}`;
            const createdAt =
                now - randomInt(4, 10) * 24 * 60 * 60 * 1000 - randomInt(1, 8);
            const stats = getDirectoryStatsForPath(
                this.files_system_tree,
                seed.source
            );
            statsByPlan.set(planId, stats);
            plans.push({
                plan_id: planId,
                title: seed.title,
                description: seed.description,
                type_str: "FULL",
                last_checkpoint_index: 0,
                source_type: SourceType.DIRECTORY,
                source: seed.source,
                target: seed.target.target_id,
                target_type: seed.target.target_type,
                last_run_time: 0,
                policy_disabled: false,
                policy: seed.policy,
                priority: seed.priority,
                reserved_versions: seed.reserved_versions,
                create_time: createdAt,
                update_time: createdAt,
                total_backup: 0,
                total_size: 0,
            });
        }
        this.plan_list.plans = plans;

        const tasks: Array<TaskInfo & { root: string }> = [];

        const addBackupTask = (
            plan: BackupPlanInfo,
            checkpointIndex: number,
            state: TaskState,
            params: {
                name: string;
                createTime: number;
                updateTime?: number;
                completedSize?: number;
                completedItems?: number;
                lastLog?: string | null;
                error?: string;
                speed?: number;
            }
        ) => {
            const stats = statsByPlan.get(plan.plan_id)!;
            const totalSize = stats.totalSize;
            const totalItems = Math.max(stats.fileCount, 1);
            const completedSize =
                state === TaskState.DONE
                    ? totalSize
                    : Math.min(totalSize, params.completedSize ?? 0);
            const completedItems =
                state === TaskState.DONE
                    ? totalItems
                    : Math.min(
                          totalItems,
                          params.completedItems ??
                              Math.round(
                                  (totalItems * completedSize) /
                                      (totalSize || 1)
                              )
                      );
            const task: TaskInfo & { root: string } = {
                taskid: `task_${this.task_list.next_task_id++}`,
                task_type: TaskType.BACKUP,
                owner_plan_id: plan.plan_id,
                checkpoint_id: `checkpoint_${checkpointIndex}`,
                total_size: totalSize,
                completed_size:
                    state === TaskState.DONE ? totalSize : completedSize,
                state,
                error: params.error,
                create_time: params.createTime,
                update_time: params.updateTime ?? params.createTime,
                item_count: totalItems,
                completed_item_count:
                    state === TaskState.DONE ? totalItems : completedItems,
                wait_transfer_item_count: Math.max(
                    totalItems -
                        (state === TaskState.DONE
                            ? totalItems
                            : completedItems),
                    0
                ),
                last_log_content: params.lastLog ?? null,
                name: params.name,
                speed: params.speed ?? 0,
                root: plan.source,
            };
            tasks.push(task);
            return task;
        };

        const addRestoreTask = (
            plan: BackupPlanInfo,
            checkpointId: string,
            state: TaskState,
            params: {
                name: string;
                createTime: number;
                updateTime?: number;
                completedSize?: number;
                completedItems?: number;
                lastLog?: string | null;
                error?: string;
                restoreUrl: string;
                isClean: boolean;
                root: string;
                speed?: number;
            }
        ) => {
            const stats = statsByPlan.get(plan.plan_id)!;
            const totalSize = stats.totalSize;
            const totalItems = Math.max(stats.fileCount, 1);
            const completedSize =
                state === TaskState.DONE
                    ? totalSize
                    : Math.min(totalSize, params.completedSize ?? 0);
            const completedItems =
                state === TaskState.DONE
                    ? totalItems
                    : Math.min(
                          totalItems,
                          params.completedItems ??
                              Math.round(
                                  (totalItems * completedSize) /
                                      (totalSize || 1)
                              )
                      );
            const task = {
                taskid: `task_${this.task_list.next_task_id++}`,
                task_type: TaskType.RESTORE,
                owner_plan_id: plan.plan_id,
                checkpoint_id: checkpointId,
                total_size: totalSize,
                completed_size:
                    state === TaskState.DONE ? totalSize : completedSize,
                state,
                error: params.error,
                create_time: params.createTime,
                update_time: params.updateTime ?? params.createTime,
                item_count: totalItems,
                completed_item_count:
                    state === TaskState.DONE ? totalItems : completedItems,
                wait_transfer_item_count: Math.max(
                    totalItems -
                        (state === TaskState.DONE
                            ? totalItems
                            : completedItems),
                    0
                ),
                last_log_content: params.lastLog ?? null,
                name: params.name,
                speed: params.speed ?? 0,
                restore_location_url: params.restoreUrl,
                is_clean_restore: params.isClean,
                root: params.root,
            } as TaskInfo & { root: string };
            tasks.push(task);
            return task;
        };

        const planDocs = this.plan_list.plans[0];
        const planMedia = this.plan_list.plans[1];
        const planProjects = this.plan_list.plans[2];
        const planAnalytics = this.plan_list.plans[3];

        addBackupTask(planDocs, 0, TaskState.DONE, {
            name: `${planDocs.title} - Full Backup`,
            createTime: now - 72 * 60 * 60 * 1000,
            updateTime: now - 70 * 60 * 60 * 1000,
            lastLog: "Completed workstation documents backup.",
            speed: 12 * 1024 * 1024,
        });

        addBackupTask(planDocs, 1, TaskState.RUNNING, {
            name: `${planDocs.title} - Incremental`,
            createTime: now - 45 * 60 * 1000,
            updateTime: now - 5 * 60 * 1000,
            completedSize: statsByPlan.get(planDocs.plan_id)!.totalSize * 0.55,
            lastLog: "Copying updated spreadsheets...",
            speed: 18 * 1024 * 1024,
        });

        addRestoreTask(planDocs, "checkpoint_0", TaskState.PENDING, {
            name: `${planDocs.title} - Restore to Exchange`,
            createTime: now - 10 * 60 * 1000,
            restoreUrl: "file:///E:/Exchange/DocumentsRestore",
            isClean: true,
            root: "E:/Exchange",
            lastLog: "Waiting for operator approval.",
        });

        addBackupTask(planMedia, 0, TaskState.DONE, {
            name: `${planMedia.title} - Monthly Archive`,
            createTime: now - 14 * 24 * 60 * 60 * 1000,
            updateTime: now - 13 * 24 * 60 * 60 * 1000,
            lastLog: "Media archive synchronized.",
            speed: 48 * 1024 * 1024,
        });

        addBackupTask(planMedia, 1, TaskState.PAUSED, {
            name: `${planMedia.title} - Differential`,
            createTime: now - 3 * 60 * 60 * 1000,
            updateTime: now - 30 * 60 * 1000,
            completedSize: statsByPlan.get(planMedia.plan_id)!.totalSize * 0.42,
            lastLog: "Paused by user request during maintenance window.",
            speed: 0,
        });

        addBackupTask(planProjects, 0, TaskState.FAILED, {
            name: `${planProjects.title} - Full Sync`,
            createTime: now - 2 * 60 * 60 * 1000,
            updateTime: now - 45 * 60 * 1000,
            completedSize:
                statsByPlan.get(planProjects.plan_id)!.totalSize * 0.35,
            lastLog: "Network error while pushing to NDN edge nodes.",
            error: "NDN sync timeout after 3 retries",
            speed: 6 * 1024 * 1024,
        });

        addBackupTask(planAnalytics, 0, TaskState.DONE, {
            name: `${planAnalytics.title} - Baseline Snapshot`,
            createTime: now - 7 * 24 * 60 * 60 * 1000,
            updateTime: now - 6 * 24 * 60 * 60 * 1000,
            lastLog: "Baseline analytics snapshot stored on portable vault.",
            speed: 22 * 1024 * 1024,
        });

        addBackupTask(planAnalytics, 1, TaskState.RUNNING, {
            name: `${planAnalytics.title} - Snapshot`,
            createTime: now - 25 * 60 * 1000,
            updateTime: now - 30 * 1000,
            completedSize:
                statsByPlan.get(planAnalytics.plan_id)!.totalSize * 0.28,
            lastLog: "Streaming analytics.db snapshot chunks...",
            speed: 32 * 1024 * 1024,
        });

        addRestoreTask(planAnalytics, "checkpoint_0", TaskState.RUNNING, {
            name: `${planAnalytics.title} - Restore verification`,
            createTime: now - 12 * 60 * 1000,
            updateTime: now - 60 * 1000,
            completedSize:
                statsByPlan.get(planAnalytics.plan_id)!.totalSize * 0.15,
            lastLog: "Validating restored indexes...",
            restoreUrl: "file:///C:/Temp/analytics-restore",
            isClean: false,
            root: "C:/Temp",
            speed: 10 * 1024 * 1024,
        });

        this.task_list.tasks = tasks;

        for (const plan of this.plan_list.plans) {
            const planTasks = tasks.filter(
                (task) =>
                    task.owner_plan_id === plan.plan_id &&
                    task.task_type === TaskType.BACKUP
            );
            plan.last_checkpoint_index = planTasks.length;
            if (planTasks.length > 0) {
                const latestUpdate = planTasks.reduce(
                    (max, task) => Math.max(max, task.update_time),
                    plan.update_time
                );
                plan.last_run_time = latestUpdate;
                plan.update_time = Math.max(plan.update_time, latestUpdate);
            }
            const completedBackups = planTasks.filter(
                (task) => task.state === TaskState.DONE
            );
            plan.total_backup = completedBackups.length;
            plan.total_size = completedBackups.reduce(
                (sum, task) => sum + task.total_size,
                0
            );
        }

        for (const target of this.target_list.targets) {
            this.logTargetCreated(target);
        }

        for (const plan of this.plan_list.plans) {
            this.logPlanCreated(plan);
        }

        for (const task of tasks) {
            const plan = this.plan_list.plans.find(
                (p) => p.plan_id === task.owner_plan_id
            );
            if (!plan) continue;
            const target = this.target_list.targets.find(
                (t) => t.target_id === plan.target
            );
            const percent =
                task.total_size > 0
                    ? Math.floor((task.completed_size / task.total_size) * 100)
                    : 0;
            this.taskProgressMilestones.set(task.taskid, percent);

            if (task.task_type === TaskType.BACKUP) {
                this.logBackupTaskCreated(plan, task);
                if (task.state === TaskState.DONE) {
                    this.logPlanRun(plan, task);
                    this.logPlanRunSuccess(plan, task);
                    this.logBackupTaskSuccess(plan, task);
                    if (target) {
                        const durationSeconds = Math.max(
                            1,
                            Math.round(
                                Math.max(
                                    task.update_time - task.create_time,
                                    1_000
                                ) / 1000
                            )
                        );
                        this.logTargetTransferSuccess(
                            target,
                            plan.source,
                            task.total_size,
                            durationSeconds
                        );
                    }
                } else if (task.state === TaskState.RUNNING) {
                    this.logPlanRun(plan, task);
                    if (target) {
                        this.logTargetTransferStart(
                            target,
                            plan.source,
                            Math.max(task.completed_size, 0)
                        );
                    }
                } else if (task.state === TaskState.PAUSED) {
                    this.logPlanRun(plan, task);
                    this.logBackupTaskPaused(plan, task);
                } else if (task.state === TaskState.FAILED) {
                    const reason =
                        task.error ?? "Unknown failure during backup";
                    this.logPlanRun(plan, task);
                    this.logPlanRunFail(plan, task, reason);
                    this.logBackupTaskFail(plan, task, reason);
                    if (target) {
                        this.logTargetTransferFail(target, plan.source, reason);
                    }
                }
            } else {
                this.logRestoreTaskCreated(plan, task);
                if (task.state === TaskState.DONE) {
                    this.logRestoreTaskSuccess(plan, task);
                    if (target) {
                        const durationSeconds = Math.max(
                            1,
                            Math.round(
                                Math.max(
                                    task.update_time - task.create_time,
                                    1_000
                                ) / 1000
                            )
                        );
                        this.logTargetTransferSuccess(
                            target,
                            plan.source,
                            task.total_size,
                            durationSeconds
                        );
                    }
                } else if (task.state === TaskState.RUNNING) {
                    if (target) {
                        this.logTargetTransferStart(
                            target,
                            plan.source,
                            Math.max(task.completed_size, 0)
                        );
                    }
                } else if (task.state === TaskState.FAILED) {
                    const reason =
                        task.error ?? "Unknown failure during restore";
                    this.logRestoreTaskFail(plan, task, reason);
                    if (target) {
                        this.logTargetTransferFail(target, plan.source, reason);
                    }
                }
            }
        }

        for (const task of tasks) {
            if (
                task.state !== TaskState.DONE &&
                task.state !== TaskState.FAILED
            ) {
                this.uncomplete_tasks.set(task.taskid, {
                    ...task,
                    last_query_time: now,
                } as TaskInfo & { last_query_time: number });
            }
        }
    }

    private appendLog(
        subject: BackupLog["subject"],
        type: BackupLog["type"],
        params: any
    ) {
        this.logs.push({
            seq: this.nextLogSeq++,
            timestamp: Date.now(),
            subject,
            type,
            params,
        } as BackupLog);
    }

    private planSubject(plan: BackupPlanInfo): BackupLog["subject"] {
        return {
            kind: "plan",
            plan_id: plan.plan_id,
            plan_title: plan.title,
        };
    }

    private targetSubject(target: BackupTargetInfo): BackupLog["subject"] {
        return {
            kind: "target",
            target_id: target.target_id,
            name: target.name,
            target_url: target.url,
        };
    }

    private taskSubject(task: TaskInfo): BackupLog["subject"] {
        return {
            kind: "task",
            task_id: task.taskid,
            task_name: task.name,
            task_type: task.task_type,
        };
    }

    private backupTaskPayload(
        plan: BackupPlanInfo,
        task: TaskInfo
    ): {
        plan_id: string;
        plan_title: string;
        backup_task: { id: string; task_name: string };
    } {
        return {
            plan_id: plan.plan_id,
            plan_title: plan.title,
            backup_task: { id: task.taskid, task_name: task.name },
        };
    }

    private restoreTaskPayload(
        plan: BackupPlanInfo,
        task: TaskInfo
    ): {
        plan_id: string;
        plan_title: string;
        restore_task: { id: string; task_name: string };
    } {
        return {
            plan_id: plan.plan_id,
            plan_title: plan.title,
            restore_task: { id: task.taskid, task_name: task.name },
        };
    }

    private logPlanCreated(plan: BackupPlanInfo) {
        this.appendLog(this.planSubject(plan), "create_plan", {});
    }

    private logPlanUpdated(plan: BackupPlanInfo) {
        this.appendLog(this.planSubject(plan), "update_plan", {});
    }

    private logPlanRemoved(planId: string, planTitle: string) {
        this.appendLog(
            {
                kind: "plan",
                plan_id: planId,
                plan_title: planTitle,
            },
            "remove_plan",
            {}
        );
    }

    private logPlanRun(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(this.planSubject(plan), "run_plan", {
            task_id: task.taskid,
            task_name: task.name,
        });
    }

    private logPlanRunSuccess(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(this.planSubject(plan), "run_success", {
            task_id: task.taskid,
            task_name: task.name,
        });
    }

    private logPlanRunFail(
        plan: BackupPlanInfo,
        task: TaskInfo,
        reason: string
    ) {
        this.appendLog(this.planSubject(plan), "run_fail", {
            task_id: task.taskid,
            task_name: task.name,
            reason,
        });
    }

    private logTargetCreated(target: BackupTargetInfo) {
        this.appendLog(this.targetSubject(target), "create_target", {});
    }

    private logTargetUpdated(target: BackupTargetInfo) {
        this.appendLog(this.targetSubject(target), "update_target", {});
    }

    private logTargetRemoved(targetId: string, name: string, url: string) {
        this.appendLog(
            {
                kind: "target",
                target_id: targetId,
                name,
                target_url: url,
            },
            "remove_target",
            {}
        );
    }

    private logTargetTransferStart(
        target: BackupTargetInfo,
        path: string,
        size: number
    ) {
        this.appendLog(this.targetSubject(target), "transfer_start", {
            path,
            size,
        });
    }

    private logTargetTransferSuccess(
        target: BackupTargetInfo,
        path: string,
        size: number,
        durationSeconds: number
    ) {
        this.appendLog(this.targetSubject(target), "transfer_success", {
            path,
            size,
            duration: durationSeconds,
        });
    }

    private logTargetTransferFail(
        target: BackupTargetInfo,
        path: string,
        reason: string
    ) {
        this.appendLog(this.targetSubject(target), "transfer_fail", {
            path,
            reason,
        });
    }

    private logTargetStateChange(
        target: BackupTargetInfo,
        oldState: TargetState,
        newState: TargetState
    ) {
        this.appendLog(this.targetSubject(target), "check_target", {
            old_state: oldState,
            new_state: newState,
        });
    }

    private logBackupTaskCreated(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(
            this.taskSubject(task),
            "create_task",
            this.backupTaskPayload(plan, task)
        );
    }

    private logBackupTaskProgress(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(
            this.taskSubject(task),
            "update_task",
            this.backupTaskPayload(plan, task)
        );
    }

    private logBackupTaskPaused(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(
            this.taskSubject(task),
            "pause_task",
            this.backupTaskPayload(plan, task)
        );
    }

    private logBackupTaskResumed(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(
            this.taskSubject(task),
            "resume_task",
            this.backupTaskPayload(plan, task)
        );
    }

    private logBackupTaskSuccess(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(this.taskSubject(task), "task_success", {
            ...this.backupTaskPayload(plan, task),
            consume_size: task.total_size,
        });
    }

    private logBackupTaskFail(
        plan: BackupPlanInfo,
        task: TaskInfo,
        reason: string
    ) {
        this.appendLog(this.taskSubject(task), "task_fail", {
            ...this.backupTaskPayload(plan, task),
            reason,
        });
    }

    private logBackupTaskRemoved(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(
            this.taskSubject(task),
            "remove_task",
            this.backupTaskPayload(plan, task)
        );
    }

    private logRestoreTaskCreated(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(
            this.taskSubject(task),
            "restore_backup",
            this.restoreTaskPayload(plan, task)
        );
    }

    private logRestoreTaskSuccess(plan: BackupPlanInfo, task: TaskInfo) {
        this.appendLog(
            this.taskSubject(task),
            "restore_success",
            this.restoreTaskPayload(plan, task)
        );
    }

    private logRestoreTaskFail(
        plan: BackupPlanInfo,
        task: TaskInfo,
        reason: string
    ) {
        this.appendLog(this.taskSubject(task), "restore_fail", {
            ...this.restoreTaskPayload(plan, task),
            reason,
        });
    }

    private startTaskSimulationTimer() {
        if (this.taskUpdateTimer) {
            clearInterval(this.taskUpdateTimer);
        }
        this.taskUpdateTimer = setInterval(() => {
            void this.simulateTaskProgress();
        }, 4_000);
    }

    private async simulateTaskProgress() {
        if (this.taskTickInFlight) return;
        this.taskTickInFlight = true;
        try {
            const now = Date.now();
            for (const task of this.task_list.tasks) {
                const originalState = task.state;
                if (
                    originalState === TaskState.DONE ||
                    originalState === TaskState.FAILED
                ) {
                    continue;
                }

                const plan = this.plan_list.plans.find(
                    (p) => p.plan_id === task.owner_plan_id
                );
                if (!plan) continue;
                const target = this.target_list.targets.find(
                    (t) => t.target_id === plan.target
                );

                if (task.state === TaskState.PAUSED) {
                    if (Math.random() < 0.2) {
                        task.state = TaskState.RUNNING;
                        task.update_time = now;
                        task.last_log_content =
                            "Task resumed automatically for simulation.";
                        this.logBackupTaskResumed(plan, task);
                        if (target) {
                            this.logTargetTransferStart(
                                target,
                                plan.source,
                                Math.max(task.completed_size, 0)
                            );
                        }
                        await this.emitTaskEvent(
                            TaskEventType.RESUME_TASK,
                            task
                        );
                    }
                    continue;
                }

                if (task.state === TaskState.PENDING) {
                    task.state = TaskState.RUNNING;
                    task.update_time = now;
                    task.last_log_content = "Task started by simulator.";
                    if (task.task_type === TaskType.BACKUP) {
                        this.logPlanRun(plan, task);
                    }
                    if (target) {
                        this.logTargetTransferStart(
                            target,
                            plan.source,
                            Math.max(task.completed_size, 0)
                        );
                    }
                    await this.emitTaskEvent(TaskEventType.UPDATE_TASK, task);
                }

                if (task.state !== TaskState.RUNNING) {
                    continue;
                }

                const totalSize = task.total_size || 0;
                const totalItems = Math.max(task.item_count, 1);
                const failureChance =
                    task.task_type === TaskType.BACKUP ? 0.07 : 0.05;

                if (Math.random() < failureChance) {
                    const reason =
                        task.task_type === TaskType.BACKUP
                            ? "Simulated network interruption."
                            : "Simulated restore verification error.";
                    task.state = TaskState.FAILED;
                    task.error = reason;
                    task.update_time = now;
                    task.last_log_content = reason;
                    if (task.task_type === TaskType.BACKUP) {
                        this.logPlanRunFail(plan, task, reason);
                        this.logBackupTaskFail(plan, task, reason);
                    } else {
                        this.logRestoreTaskFail(plan, task, reason);
                    }
                    if (target) {
                        this.logTargetTransferFail(target, plan.source, reason);
                        if (target.state === TargetState.ONLINE) {
                            this.logTargetStateChange(
                                target,
                                target.state,
                                TargetState.ERROR
                            );
                            target.state = TargetState.ERROR;
                        }
                        target.last_error = reason;
                    }
                    this.taskProgressMilestones.delete(task.taskid);
                    this.uncomplete_tasks.delete(task.taskid);
                    await this.emitTaskEvent(TaskEventType.FAIL_TASK, task);
                    continue;
                }

                const previousCompleted = task.completed_size;
                const remaining = Math.max(totalSize - previousCompleted, 0);
                const chunk =
                    totalSize === 0
                        ? 0
                        : Math.min(
                              remaining,
                              randomInt(2 * 1024 * 1024, 32 * 1024 * 1024)
                          );
                if (chunk > 0) {
                    task.completed_size += chunk;
                }
                if (totalSize === 0) {
                    task.completed_item_count = totalItems;
                    task.wait_transfer_item_count = 0;
                } else {
                    const completedItems = Math.min(
                        totalItems,
                        Math.max(
                            task.completed_item_count,
                            Math.round(
                                (task.completed_size / totalSize) * totalItems
                            )
                        )
                    );
                    task.completed_item_count = completedItems;
                    task.wait_transfer_item_count = Math.max(
                        totalItems - completedItems,
                        0
                    );
                }
                task.update_time = now;

                const intervalMs = 4_000;
                const delta = task.completed_size - previousCompleted;
                task.speed =
                    intervalMs > 0
                        ? Math.max(Math.round(delta / (intervalMs / 1_000)), 0)
                        : task.speed;

                const percentComplete =
                    totalSize > 0
                        ? Math.min(
                              100,
                              Math.floor(
                                  (task.completed_size / totalSize) * 100
                              )
                          )
                        : 100;
                const lastMilestone =
                    this.taskProgressMilestones.get(task.taskid) ?? 0;
                if (
                    task.task_type === TaskType.BACKUP &&
                    percentComplete >= lastMilestone + 15 &&
                    percentComplete < 100
                ) {
                    this.taskProgressMilestones.set(
                        task.taskid,
                        percentComplete
                    );
                    this.logBackupTaskProgress(plan, task);
                }

                task.last_log_content =
                    percentComplete >= 100
                        ? "Finalizing task..."
                        : `Progress ${percentComplete}%`;
                this.uncomplete_tasks.set(task.taskid, {
                    ...task,
                    last_query_time: now,
                } as TaskInfo & { last_query_time: number });

                await this.emitTaskEvent(TaskEventType.UPDATE_TASK, task);

                if (
                    (totalSize === 0 || task.completed_size >= totalSize) &&
                    task.state === TaskState.RUNNING
                ) {
                    task.state = TaskState.DONE;
                    task.completed_size = totalSize;
                    task.completed_item_count = totalItems;
                    task.wait_transfer_item_count = 0;
                    task.update_time = now;
                    task.last_log_content =
                        task.task_type === TaskType.BACKUP
                            ? "Backup completed successfully."
                            : "Restore completed successfully.";
                    this.taskProgressMilestones.set(task.taskid, 100);
                    this.uncomplete_tasks.delete(task.taskid);

                    if (task.task_type === TaskType.BACKUP) {
                        if (originalState !== TaskState.DONE) {
                            plan.total_backup += 1;
                            plan.total_size += totalSize;
                        }
                        plan.last_run_time = now;
                        plan.update_time = now;
                        this.logBackupTaskSuccess(plan, task);
                        this.logPlanRunSuccess(plan, task);
                        if (target) {
                            target.used = Math.min(
                                target.total,
                                target.used + totalSize
                            );
                            const durationSeconds = Math.max(
                                1,
                                Math.round(
                                    Math.max(
                                        task.update_time - task.create_time,
                                        1_000
                                    ) / 1_000
                                )
                            );
                            this.logTargetTransferSuccess(
                                target,
                                plan.source,
                                totalSize,
                                durationSeconds
                            );
                            if (target.state === TargetState.ERROR) {
                                this.logTargetStateChange(
                                    target,
                                    target.state,
                                    TargetState.ONLINE
                                );
                                target.state = TargetState.ONLINE;
                                target.last_error = "";
                            }
                        }
                    } else {
                        plan.update_time = now;
                        this.logRestoreTaskSuccess(plan, task);
                        if (target) {
                            const durationSeconds = Math.max(
                                1,
                                Math.round(
                                    Math.max(
                                        task.update_time - task.create_time,
                                        1_000
                                    ) / 1_000
                                )
                            );
                            this.logTargetTransferSuccess(
                                target,
                                plan.source,
                                totalSize,
                                durationSeconds
                            );
                        }
                    }

                    await this.emitTaskEvent(TaskEventType.COMPLETE_TASK, task);
                }
            }
        } finally {
            this.taskTickInFlight = false;
        }
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
        reserved_versions: number;
    }): Promise<string> {
        const targetInfo = this.target_list.targets.find(
            (t) => t.target_id === params.target
        );
        const result = {
            ...params,
            target_name: targetInfo?.name,
            target_url: targetInfo?.url,
            plan_id: `plan_${this.plan_list.next_plan_id++}`,
            last_checkpoint_index: -1,
            last_run_time: 0,
            create_time: Date.now(),
            update_time: Date.now(),
            total_backup: 0,
            total_size: 0,
        };
        console.log("Created plan:", result);
        this.plan_list.plans.push(result);
        this.logPlanCreated(result);
        await this.emitTaskEvent(TaskEventType.CREATE_PLAN, result);
        return result.plan_id;
    }

    async listBackupPlans(): Promise<string[]> {
        return this.plan_list.plans.map((p) => p.plan_id);
    }

    async getBackupPlan(planId: string): Promise<BackupPlanInfo> {
        console.log("Get plan:", planId);
        return this.plan_list.plans.find((p) => p.plan_id === planId)!;
    }

    async updateBackupPlan(planInfo: BackupPlanInfo): Promise<boolean> {
        const idx = this.plan_list.plans.findIndex(
            (p) => p.plan_id === planInfo.plan_id
        );
        if (idx === -1) return false;
        this.plan_list.plans[idx] = planInfo;
        this.logPlanUpdated(planInfo);
        await this.emitTaskEvent(TaskEventType.UPDATE_PLAN, planInfo);
        return true;
    }

    async removeBackupPlan(planId: string): Promise<boolean> {
        const idx = this.plan_list.plans.findIndex((p) => p.plan_id === planId);
        if (idx === -1) return false;
        const plan = this.plan_list.plans[idx];
        this.plan_list.plans.splice(idx, 1);
        this.logPlanRemoved(plan.plan_id, plan.title);
        await this.emitTaskEvent(TaskEventType.REMOVE_PLAN, planId);
        return true;
    }

    async createBackupTask(planId: string, parentCheckpointId?: string) {
        let plan = this.plan_list.plans.find((p) => p.plan_id === planId)!;
        plan.last_run_time = Date.now();
        const checkpoint_id = plan.last_checkpoint_index++;
        const result = {
            taskid: `task_${this.task_list.next_task_id++}`,
            owner_plan_id: planId,
            checkpoint_id: `checkpoint_${checkpoint_id}`,
            state: TaskState.PENDING,
            create_time: Date.now(),
            task_type: TaskType.BACKUP,
            total_size: 0,
            completed_size: 0,
            update_time: Date.now(),
            item_count: 0,
            completed_item_count: 0,
            wait_transfer_item_count: 0,
            last_log_content: null,
            name: `${plan.title}-BACKUP-${checkpoint_id}`,
            speed: 0,
            root: plan.source,
        };
        this.task_list.tasks.push(result);
        this.taskProgressMilestones.set(result.taskid, 0);
        this.logBackupTaskCreated(plan, result);
        await this.emitTaskEvent(TaskEventType.CREATE_TASK, result);
        return result.taskid;
    }

    async createRestoreTask(
        planId: string,
        checkpointId: string,
        targetLocationUrl: string,
        is_clean_folder?: boolean,
        subPath?: string
    ) {
        const plan = this.plan_list.plans.find((p) => p.plan_id === planId)!;
        const backup_task = this.task_list.tasks
            .filter((t) => t.owner_plan_id === planId)
            .find((t) => t.taskid === checkpointId);
        console.log(
            "Create restore task for plan:",
            planId,
            "checkpoint:",
            checkpointId,
            "based on backup task:",
            backup_task,
            "plans:",
            this.plan_list.plans,
            "tasks:",
            this.task_list.tasks
        );

        const result = {
            taskid: `task_${this.task_list.next_task_id++}`,
            owner_plan_id: planId,
            checkpoint_id: checkpointId,
            state: TaskState.PENDING,
            create_time: Date.now(),
            task_type: TaskType.RESTORE,
            total_size: backup_task!.total_size,
            completed_size: 0,
            update_time: 0,
            item_count: backup_task!.item_count,
            completed_item_count: 0,
            wait_transfer_item_count: 0,
            last_log_content: null,
            name: `${plan.title}-RESTORE-${checkpointId}`,
            speed: 0,
            restore_location_url: targetLocationUrl,
            is_clean_restore: is_clean_folder,
            root: subPath || backup_task!.root,
        };
        this.task_list.tasks.push(result);
        this.taskProgressMilestones.set(result.taskid, 0);
        this.logRestoreTaskCreated(plan, result as TaskInfo);
        await this.emitTaskEvent(TaskEventType.CREATE_TASK, result);
        return result.taskid;
    }

    async listBackupTasks(
        filter: TaskFilter = {},
        offset: number = 0,
        limit?: number,
        orderBy?: Array<[ListTaskOrderBy, ListOrder]>
    ): Promise<{ task_ids: string[]; total: number }> {
        let result_tasks = this.task_list.tasks;
        if (filter.state) {
            result_tasks = result_tasks.filter((t) =>
                filter.state?.includes(t.state)
            );
        }
        if (filter.owner_plan_id) {
            result_tasks = result_tasks.filter((t) =>
                filter.owner_plan_id?.includes(t.owner_plan_id)
            );
        }
        if (filter.type) {
            result_tasks = result_tasks.filter((t) =>
                filter.type?.includes(t.task_type)
            );
        }
        if (filter.owner_plan_title) {
            const lowercase_owner_plan_titles = filter.owner_plan_title.map(
                (title) => title.toLowerCase()
            );
            result_tasks = result_tasks.filter((t) => {
                const plan = this.plan_list.plans.find(
                    (p) => p.plan_id === t.owner_plan_id
                );
                return (
                    plan &&
                    lowercase_owner_plan_titles.find(
                        (title) => plan.title.toLowerCase().indexOf(title) >= 0
                    )
                );
            });
        }
        if (orderBy) {
            result_tasks = result_tasks.sort((a, b) => {
                for (let [key, order] of orderBy) {
                    let cmp = 0;
                    switch (key) {
                        case ListTaskOrderBy.CREATE_TIME:
                            cmp = a.create_time - b.create_time;
                            break;
                        case ListTaskOrderBy.UPDATE_TIME:
                            cmp = a.update_time - b.update_time;
                            break;
                        case ListTaskOrderBy.COMPLETE_TIME:
                            if (
                                a.state === TaskState.DONE &&
                                b.state === TaskState.DONE
                            ) {
                                cmp = a.update_time - b.update_time;
                            } else if (a.state === TaskState.DONE) return 1;
                            else if (b.state === TaskState.DONE) return -1;
                            break;
                    }
                    if (cmp !== 0) {
                        return order === ListOrder.ASC ? cmp : -cmp;
                    }
                }
                return 0;
            });
        }

        const total = result_tasks.length;
        if (offset > 0) {
            result_tasks = result_tasks.slice(offset);
        }
        if (limit) {
            result_tasks = result_tasks.slice(0, limit);
        }
        return {
            task_ids: result_tasks.map((t) => t.taskid),
            total,
        };
    }

    async getTaskInfo(taskId: string): Promise<TaskInfo> {
        const result = this.task_list.tasks.find((t) => t.taskid === taskId);
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
                this.uncomplete_tasks.set(taskId, {
                    ...result,
                    last_query_time: now,
                });
            }
        }
        return result!;
    }

    async resumeBackupTask(taskId: string) {
        const task = this.task_list.tasks.find((t) => t.taskid === taskId)!;
        const plan = this.plan_list.plans.find(
            (p) => p.plan_id === task.owner_plan_id
        );
        const target = plan
            ? this.target_list.targets.find((t) => t.target_id === plan.target)
            : undefined;
        if (
            task.state === TaskState.PAUSED ||
            task.state === TaskState.FAILED
        ) {
            task.state = TaskState.RUNNING;
            task.update_time = Date.now();
            task.last_log_content = "Task resumed manually.";
            if (plan && task.task_type === TaskType.BACKUP) {
                this.logBackupTaskResumed(plan, task);
                if (target) {
                    this.logTargetTransferStart(
                        target,
                        plan.source,
                        Math.max(task.completed_size, 0)
                    );
                }
            }
            await this.emitTaskEvent(TaskEventType.RESUME_TASK, task);
        }
        return true;
    }

    async pauseBackupTask(taskId: string) {
        const task = this.task_list.tasks.find((t) => t.taskid === taskId)!;
        const plan = this.plan_list.plans.find(
            (p) => p.plan_id === task.owner_plan_id
        );
        if (
            task.state === TaskState.RUNNING ||
            task.state === TaskState.PENDING
        ) {
            task.state = TaskState.PAUSED;
            task.update_time = Date.now();
            task.last_log_content = "Task paused manually.";
            if (plan && task.task_type === TaskType.BACKUP) {
                this.logBackupTaskPaused(plan, task);
            }
            await this.emitTaskEvent(TaskEventType.PAUSE_TASK, task);
        }
        return true;
    }

    async removeBackupTask(taskId: string): Promise<boolean> {
        const idx = this.task_list.tasks.findIndex((t) => t.taskid === taskId);
        if (idx === -1) return false;
        const task = this.task_list.tasks[idx];
        const plan = this.plan_list.plans.find(
            (p) => p.plan_id === task.owner_plan_id
        );
        this.task_list.tasks.splice(idx, 1);
        this.taskProgressMilestones.delete(taskId);
        if (plan) {
            if (task.task_type === TaskType.BACKUP) {
                this.logBackupTaskRemoved(plan, task);
            } else {
                this.appendLog(this.taskSubject(task), "remove_task", {
                    plan_id: plan.plan_id,
                    plan_title: plan.title,
                });
            }
        }
        await this.emitTaskEvent(TaskEventType.REMOVE_TASK, taskId);
        return true;
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
        const task = this.task_list.tasks.find((t) => t.taskid === taskId)!;
        if (!task) return [];
        const targetPath = subDir ?? task.root;
        const node = findNodeByPath(this.files_system_tree, targetPath);
        if (!node || !node.isDirectory || !node.children) {
            return [];
        }
        return node.children
            .filter((child): child is TaskFile => isTaskFileEntry(child))
            .map((child) => ({
                name: child.name,
                len: child.size || 0,
                create_time: child.modifiedTime || 0,
                update_time: child.modifiedTime || 0,
                is_dir: child.isDirectory,
            }));
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
        const task = this.task_list.tasks.find((t) => t.taskid === taskId)!;
        if (!task) return [];
        const node = findNodeByPath(this.files_system_tree, filePath);
        if (!node || node.isDirectory || !node.children) {
            return [];
        }
        return node.children.filter(
            (child): child is TaskChunk => !isTaskFileEntry(child)
        );
    }

    async createBackupTarget(
        target_type: TargetType,
        target_url: string,
        name: string,
        config: any
    ): Promise<string> {
        const result: BackupTargetInfo = {
            target_id: `target_${this.target_list.next_target_id++}`,
            target_type,
            url: target_url,
            name,
            description: "",
            state: TargetState.UNKNOWN,
            used: 0,
            total: 0,
            last_error: "",
        };
        this.target_list.targets.push(result);
        this.logTargetCreated(result);
        await this.emitTaskEvent(TaskEventType.CREATE_TARGET, result);
        return result.target_id;
    }

    async listBackupTargets(): Promise<string[]> {
        return this.target_list.targets.map((t) => t.target_id);
    }

    async getBackupTarget(targetId: string): Promise<BackupTargetInfo> {
        const result = this.target_list.targets.find(
            (t) => t.target_id === targetId
        )!;
        return result;
    }

    async updateBackupTarget(targetInfo: BackupTargetInfo): Promise<boolean> {
        const idx = this.target_list.targets.findIndex(
            (t) => t.target_id === targetInfo.target_id
        );
        if (idx === -1) return false;
        this.target_list.targets[idx] = targetInfo;
        this.logTargetUpdated(targetInfo);
        await this.emitTaskEvent(TaskEventType.UPDATE_TARGET, targetInfo);
        return true;
    }

    async removeBackupTarget(targetId: string): Promise<boolean> {
        const idx = this.target_list.targets.findIndex(
            (t) => t.target_id === targetId
        );
        if (idx === -1) return false;
        const target = this.target_list.targets[idx];
        this.target_list.targets.splice(idx, 1);
        this.logTargetRemoved(target.target_id, target.name, target.url);
        await this.emitTaskEvent(TaskEventType.REMOVE_TARGET, {
            target_id: targetId,
        });
        return true;
    }

    async consumeSizeSummary(): Promise<{ total: number; today: number }> {
        const total = this.task_list.tasks
            .map((t) => t.completed_size)
            .reduce((a, b) => a + b, 0);
        return { total, today: total };
    }

    async statisticsSummary(
        from: number,
        to: number
    ): Promise<{ complete: number; failed: number }> {
        const complete = this.task_list.tasks.filter(
            (t) => t.state === TaskState.DONE
        ).length;
        const failed = this.task_list.tasks.filter(
            (t) => t.state === TaskState.FAILED
        ).length;
        return { complete, failed };
    }

    async listDirChildren(
        path?: string,
        purpose?: DirectoryPurpose,
        options?: {
            only_dirs?: boolean;
            only_files?: boolean;
        }
    ): Promise<DirectoryNode[]> {
        await new Promise((resolve) => setTimeout(resolve, 200));
        return listDirectoryEntries(this.files_system_tree, path, options);
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
        await new Promise((resolve) => setTimeout(resolve, 150));
        let filtered = [...this.logs];
        if (subject) {
            filtered = filtered.filter((log) => {
                if (subject && "plan_id" in subject) {
                    return (
                        log.subject.kind === "plan" &&
                        log.subject.plan_id === subject.plan_id
                    );
                }
                if (subject && "target_id" in subject) {
                    return (
                        log.subject.kind === "target" &&
                        log.subject.target_id === subject.target_id
                    );
                }
                if (subject && "task_id" in subject) {
                    return (
                        log.subject.kind === "task" &&
                        log.subject.task_id === subject.task_id
                    );
                }
                return true;
            });
        }

        filtered.sort((a, b) => {
            if (orderBy === ListOrder.ASC) {
                if (a.timestamp === b.timestamp) {
                    return a.seq - b.seq;
                }
                return a.timestamp - b.timestamp;
            } else {
                if (a.timestamp === b.timestamp) {
                    return b.seq - a.seq;
                }
                return b.timestamp - a.timestamp;
            }
        });

        const total = filtered.length;
        const start = Math.max(offset, 0);
        const end = limit && limit > 0 ? start + limit : filtered.length;
        const logs = filtered.slice(start, end);
        return { logs, total };
    }
}

export const taskManager = new FakeTaskManager();

function randomInt(min: number, max: number): number {
    return Math.floor(Math.random() * (max - min + 1)) + min;
}

function randomHex(length: number): string {
    const alphabet = "0123456789abcdef";
    let output = "";
    for (let i = 0; i < length; i++) {
        output += alphabet.charAt(randomInt(0, alphabet.length - 1));
    }
    return output;
}

function generateChunks(
    totalSize: number,
    counter: { value: number },
    sizeRange: [number, number] = [64 * 1024, 512 * 1024]
): TaskChunk[] {
    const chunks: TaskChunk[] = [];
    let remaining = totalSize;
    let seq = 0;
    while (remaining > 0) {
        const maxChunk = Math.min(sizeRange[1], remaining);
        const minChunk = Math.min(sizeRange[0], remaining);
        const size =
            remaining <= sizeRange[0]
                ? remaining
                : randomInt(minChunk, maxChunk);
        chunks.push({
            chunkid: `chunk-${counter.value++}-${randomHex(6)}`,
            seq: `${seq++}`,
            size,
            status: "stored",
        });
        remaining -= size;
    }
    return chunks;
}

function buildTaskTree(spec: MockFsSpec, counter: { value: number }): TaskFile {
    if (spec.kind === "dir") {
        const children = spec.children.map((child) =>
            buildTaskTree(child, counter)
        );
        const size = children.reduce(
            (sum, child) => sum + (child.size ?? 0),
            0
        );
        const modifiedTime = children.reduce(
            (latest, child) =>
                Math.max(latest, child.modifiedTime ?? Date.now()),
            0
        );
        return {
            name: spec.name,
            isDirectory: true,
            size,
            modifiedTime,
            children,
        };
    }
    const chunks = generateChunks(spec.size, counter, spec.chunkSizeRange);
    return {
        name: spec.name,
        isDirectory: false,
        size: spec.size,
        modifiedTime: Date.now() - spec.modifiedHoursAgo * 3600000,
        children: chunks,
    };
}

function isTaskFileEntry(entry: TaskFile | TaskChunk): entry is TaskFile {
    return (entry as TaskFile).isDirectory !== undefined;
}

function normalizeFsPath(input?: string): string | undefined {
    if (!input) return undefined;
    let normalized = input.replace(/\\/g, "/");
    if (normalized.length > 1 && normalized.endsWith("/")) {
        normalized = normalized.slice(0, -1);
    }
    return normalized;
}

function splitFsSegments(path: string): string[] {
    const driveMatch = path.match(/^[A-Za-z]:/);
    if (driveMatch) {
        const drive = driveMatch[0];
        const rest = path.slice(drive.length);
        const parts = rest.split("/").filter(Boolean);
        return [drive, ...parts];
    }
    if (path === "/") return [];
    return path.split("/").filter(Boolean);
}

function findNodeByPath(root: TaskFile, path?: string): TaskFile | undefined {
    const normalized = normalizeFsPath(path);
    if (!normalized || normalized === "/") {
        return root;
    }
    const segments = splitFsSegments(normalized);
    let current: TaskFile | undefined = root;
    for (const seg of segments) {
        if (!current?.children || !current.isDirectory) {
            return undefined;
        }
        const next = current.children.find(
            (child) => isTaskFileEntry(child) && child.name === seg
        );
        if (!next) {
            return undefined;
        }
        current = next;
    }
    return current;
}

function computeDirectoryStats(node: TaskFile): {
    totalSize: number;
    fileCount: number;
} {
    if (!node.isDirectory) {
        return { totalSize: node.size ?? 0, fileCount: 1 };
    }
    if (!node.children) {
        return { totalSize: 0, fileCount: 0 };
    }
    return node.children.reduce(
        (acc, child) => {
            if (!isTaskFileEntry(child)) {
                return acc;
            }
            const stats = computeDirectoryStats(child);
            return {
                totalSize: acc.totalSize + stats.totalSize,
                fileCount: acc.fileCount + stats.fileCount,
            };
        },
        { totalSize: 0, fileCount: 0 }
    );
}

function listDirectoryEntries(
    root: TaskFile,
    path?: string,
    options?: { only_dirs?: boolean; only_files?: boolean }
): DirectoryNode[] {
    const node = findNodeByPath(root, path);
    if (!node?.children) {
        return [];
    }
    return node.children
        .filter((child): child is TaskFile => isTaskFileEntry(child))
        .filter((child) => {
            if (options?.only_dirs) return child.isDirectory;
            if (options?.only_files) return !child.isDirectory;
            return true;
        })
        .map((child) => ({
            name: child.name,
            isDirectory: child.isDirectory,
        }));
}

function getDirectoryStatsForPath(
    root: TaskFile,
    path: string
): { totalSize: number; fileCount: number } {
    const node = findNodeByPath(root, path);
    if (!node) {
        return { totalSize: 0, fileCount: 0 };
    }
    return computeDirectoryStats(node);
}
