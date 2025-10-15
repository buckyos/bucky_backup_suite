import {
    BackupLog,
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

interface TaskFile {
    name: string;
    isDirectory: boolean;
    size?: number;
    modifiedTime?: number;
    children?: TaskFile[];
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

    constructor() {
        super();
        // 初始化一些模拟数据
        // TODO: 从MOCK_DIR_TREE构造files_system_tree
    }

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
        reserved_versions: number;
    }): Promise<string> {
        const result = {
            ...params,
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
            .find((t) => t.checkpoint_id === checkpointId);
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
        if (offset > 0) {
            result_tasks = result_tasks.slice(offset);
        }
        if (!limit) {
            result_tasks = result_tasks.slice(0, limit);
        }
        return {
            task_ids: result_tasks.map((t) => t.taskid),
            total: this.task_list.tasks.length,
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

    async removeBackupTask(taskId: string): Promise<boolean> {
        const idx = this.task_list.tasks.findIndex((t) => t.taskid === taskId);
        if (idx === -1) return false;
        this.task_list.tasks.splice(idx, 1);
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
        if (!subDir) {
            subDir = task.root;
        }

        // find subDir under files_system_tree
        const subPaths = subDir.split("/").filter((s) => s.length > 0);
        let found = this.files_system_tree;
        for (const part of subPaths) {
            if (!found.children) return [];
            const next = found.children.find((c) => c.name === part);
            if (!next) return [];
            found = next;
        }
        return found.children
            ? found.children.map((c) => ({
                  name: c.name,
                  len: c.size || 0,
                  create_time: c.modifiedTime || 0,
                  update_time: c.modifiedTime || 0,
                  is_dir: c.isDirectory,
              }))
            : [];
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

    async listLogs(
        offset: number,
        limit: number,
        orderBy: ListOrder,
        subject?:
            | { plan_id: string }
            | { target_id: string }
            | { task_id: string }
    ): Promise<{ logs: BackupLog[]; total: number }> {
        return { logs: [], total: 0 };
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
