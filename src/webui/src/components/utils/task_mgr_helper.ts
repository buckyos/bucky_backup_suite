import { BackupPlanInfo, BackupTargetInfo, TaskInfo } from "./task_mgr";

export enum PlanState {
    ACTIVE = "ACTIVE",
    DISABLED = "DISABLED",
    WARNING = "WARNING",
    ERROR = "ERROR",
}

export class TaskMgrHelper {
    static percent(sub: number, total: number): number {
        if (total === 0) return 100;
        return Math.floor((sub * 100) / total);
    }

    static taskProgress(task: TaskInfo): number {
        return TaskMgrHelper.percent(task.completed_size, task.total_size);
    }

    static formatSize(size: number): string {
        if (size < 0) return "0 B";
        const units = ["B", "KB", "MB", "GB", "TB"];
        let unitIndex = 0;
        let adjustedSize = size;

        while (adjustedSize >= 1024 && unitIndex < units.length - 1) {
            adjustedSize /= 1024;
            unitIndex++;
        }

        return `${adjustedSize.toFixed(2)} ${units[unitIndex]}`;
    }

    static taskRemaining(task: TaskInfo): number {
        return task.total_size - task.completed_size;
    }

    static taskRemainingStr(task: TaskInfo): string {
        return this.formatSize(this.taskRemaining(task));
    }

    static taskTotalStr(task: TaskInfo): string {
        return this.formatSize(task.total_size);
    }

    static taskCompletedStr(task: TaskInfo): string {
        return this.formatSize(task.completed_size);
    }

    static taskSpeedStr(task: TaskInfo): string {
        return this.formatSize(task.speed) + "/s";
    }

    static taskETA(task: TaskInfo): string {
        if (task.speed === 0) return "∞";
        const remainingSeconds = Math.floor(
            this.taskRemaining(task) / task.speed
        );
        const hours = Math.floor(remainingSeconds / 3600);
        const minutes = Math.floor((remainingSeconds % 3600) / 60);
        const seconds = remainingSeconds % 60;
        return `${hours}h ${minutes}m ${seconds}s`;
    }

    static planState(
        plan: BackupPlanInfo,
        uncompleteTasks: TaskInfo[]
    ): PlanState {
        // 按create_time降序排列
        const myUncompleteTasks = uncompleteTasks
            .filter((t) => t.owner_plan_id === plan.plan_id)
            .sort((a, b) => b.create_time - a.create_time);
        if (myUncompleteTasks.length === 0) {
            return plan.policy ? PlanState.ACTIVE : PlanState.DISABLED;
        } else {
            const latestTask = myUncompleteTasks[0];
            if (latestTask.state === "FAILED") {
                return PlanState.ERROR;
            } else if (myUncompleteTasks.find((t) => t.state === "FAILED")) {
                return PlanState.WARNING;
            } else {
                return plan.policy ? PlanState.ACTIVE : PlanState.DISABLED;
            }
        }
    }

    static planNextRunTime(plan: BackupPlanInfo): number | null {
        if (!plan.policy) {
            return null;
        }
        // todo: 计算下次运行时间
        return null;
    }

    static targetUsagePercent(target: BackupTargetInfo): number {
        return TaskMgrHelper.percent(target.used, target.total);
    }
}
