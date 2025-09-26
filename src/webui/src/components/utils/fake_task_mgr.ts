import {
    BackupPlanInfo,
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

export class FakeTaskManager extends BackupTaskManager {
    private plan_list = {
        next_plan_id: 1,
        plans: new Array<BackupPlanInfo>(),
    };
    private task_list = {
        next_task_id: 1,
        tasks: new Array<TaskInfo>(),
    };
    private target_list = {
        next_target_id: 1,
        targets: new Array<BackupTargetInfo>(),
    };

    async createBackupPlan(params: {
        type_str: string;
        source_type: SourceType;
        source: string;
        target_type: TargetType;
        target: string;
        title: string;
        description: string;
        policy: PlanPolicy[];
        priority: number;
    }): Promise<string> {
        const result = {
            ...params,
            plan_id: `plan_${this.plan_list.next_plan_id++}`,
            last_checkpoint_index: -1,
            last_run_time: 0,
        };
        console.log("Created plan:", result);
        this.plan_list.plans.push(result);
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
        await this.emitTaskEvent(TaskEventType.UPDATE_PLAN, planInfo);
        return true;
    }

    async removeBackupPlan(planId: string): Promise<boolean> {
        const idx = this.plan_list.plans.findIndex((p) => p.plan_id === planId);
        if (idx === -1) return false;
        this.plan_list.plans.splice(idx, 1);
        await this.emitTaskEvent(TaskEventType.REMOVE_PLAN, planId);
        return true;
    }

    async createBackupTask(planId: string, parentCheckpointId?: string) {
        let plan = this.plan_list.plans.find((p) => p.plan_id === planId)!;
        plan.last_run_time = Date.now();
        const checkpoint_id = plan.last_checkpoint_index++;
        const result: TaskInfo = {
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
        };
        this.task_list.tasks.push(result);
        await this.emitTaskEvent(TaskEventType.CREATE_TASK, result);
        return result.taskid;
    }

    async createRestoreTask(
        planId: string,
        checkpointId: string,
        targetLocationUrl: string,
        is_clean_folder?: boolean
    ) {
        const plan = this.plan_list.plans.find((p) => p.plan_id === planId)!;
        const backup_task = this.task_list.tasks
            .filter((t) => t.owner_plan_id === planId)
            .find((t) => t.checkpoint_id === checkpointId);
        const result: TaskInfo = {
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
        };
        this.task_list.tasks.push(result);
        await this.emitTaskEvent(TaskEventType.CREATE_TASK, result);
        return result.taskid;
    }

    async listBackupTasks(
        filter: TaskFilter[] = [TaskFilter.ALL],
        offset: number = 0,
        limit: number | null = null,
        orderBy: Map<ListTaskOrderBy, ListOrder> | null = null
    ): Promise<string[]> {
        if (filter.includes(TaskFilter.ALL)) {
            return this.task_list.tasks.map((t) => t.taskid);
        } else {
            let tasks = this.task_list.tasks.filter((t) => {
                switch (t.state) {
                    case TaskState.PENDING:
                    case TaskState.RUNNING:
                        return filter.includes(TaskFilter.RUNNING);
                    case TaskState.FAILED:
                        return filter.includes(TaskFilter.FAILED);
                    case TaskState.DONE:
                        return filter.includes(TaskFilter.DONE);
                    case TaskState.PAUSED:
                        return filter.includes(TaskFilter.PAUSED);
                }
            });
            return tasks.map((t) => t.taskid);
        }
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
        if (
            task.state === TaskState.PAUSED ||
            task.state === TaskState.FAILED
        ) {
            task.state = TaskState.RUNNING;
            task.update_time = Date.now();
            await this.emitTaskEvent(TaskEventType.RESUME_TASK, task);
        }
        return true;
    }

    async pauseBackupTask(taskId: string) {
        const task = this.task_list.tasks.find((t) => t.taskid === taskId)!;
        if (
            task.state === TaskState.RUNNING ||
            task.state === TaskState.PENDING
        ) {
            task.state = TaskState.PAUSED;
            task.update_time = Date.now();
            await this.emitTaskEvent(TaskEventType.PAUSE_TASK, task);
        }
        return true;
    }

    async resume_last_working_task() {
        let taskid_list = await this.listBackupTasks([TaskFilter.PAUSED]);
        if (taskid_list.length > 0) {
            let last_task = taskid_list[0];
            console.log("resume last task:", last_task);
            this.resumeBackupTask(last_task);
            await this.emitTaskEvent(TaskEventType.RESUME_TASK, last_task);
        }
    }

    async pause_all_tasks() {
        let taskid_list = await this.listBackupTasks([TaskFilter.RUNNING]);
        for (let taskid of taskid_list) {
            this.pauseBackupTask(taskid);
            await this.emitTaskEvent(TaskEventType.PAUSE_TASK, taskid);
        }
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
        await this.emitTaskEvent(TaskEventType.UPDATE_TARGET, targetInfo);
        return true;
    }

    async removeBackupTarget(targetId: string): Promise<boolean> {
        const idx = this.target_list.targets.findIndex(
            (t) => t.target_id === targetId
        );
        if (idx === -1) return false;
        this.target_list.targets.splice(idx, 1);
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
        purpose: DirectoryPurpose,
        path?: string,
        options?: {
            only_dirs?: boolean;
            only_files?: boolean;
        }
    ): Promise<DirectoryNode[]> {
        return mockGetDirectories(path);
    }
}

export const taskManager = new FakeTaskManager();

// 模拟的目录树（JSON结构，直观易编辑）
// 约定：键为文件夹名，值为其子目录对象；空对象表示该目录目前无子项
const MOCK_DIR_TREE: Record<string, any> = {
    "C:": {
        Users: {
            Administrator: {
                Desktop: {},
                Documents: {},
                Downloads: {},
                Pictures: {},
            },
            Public: {},
            Default: {},
        },
        "Program Files": {},
        Windows: {},
        Temp: {},
    },
    "D:": {
        Projects: {},
        Backups: {},
        Media: {},
        Data: {},
    },
    "E:": {},
};

function normalizePath(input?: string): string | undefined {
    if (!input) return undefined;
    // 将反斜杠统一为斜杠，便于分割
    let p = input.replace(/\\/g, "/");
    // 去掉路径末尾的斜杠（如 C:/ -> C:）
    if (p.length > 1 && p.endsWith("/")) p = p.slice(0, -1);
    return p;
}

function splitSegments(path: string): string[] {
    // 处理盘符路径与普通段
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

function getChildrenFromTree(
    tree: Record<string, any>,
    path?: string
): DirectoryNode[] {
    const p = normalizePath(path);
    // 根目录：返回所有盘符
    if (!p || p === "/") {
        return Object.keys(tree).map((name) => ({ name, isDirectory: true }));
    }

    const segs = splitSegments(p);
    // 第一段应为盘符
    if (segs.length === 0) {
        return Object.keys(tree).map((name) => ({ name, isDirectory: true }));
    }

    // 自根逐级向下
    let cursor: any = tree;
    for (let i = 0; i < segs.length; i++) {
        const key = segs[i];
        if (cursor && typeof cursor === "object" && key in cursor) {
            cursor = cursor[key];
        } else {
            return [];
        }
    }

    // cursor 应是一个对象，列出其子目录
    if (cursor && typeof cursor === "object") {
        return Object.keys(cursor).map((name) => ({ name, isDirectory: true }));
    }
    return [];
}

// 模拟API调用获取目录结构
async function mockGetDirectories(path?: string): Promise<DirectoryNode[]> {
    await new Promise((resolve) => setTimeout(resolve, 300));
    // 基于 JSON 目录树直接生成结果
    return getChildrenFromTree(MOCK_DIR_TREE, path);
}
