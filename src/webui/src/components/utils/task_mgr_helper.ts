import { AlertDialogContent } from "../ui/alert-dialog";
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

    static planNextRunTime(plan: BackupPlanInfo): number | undefined {
        const period = plan.policy.find((p) => "minutes" in p);
        if (!period) {
            return undefined;
        }

        const now = new Date();
        if ("week" in period) {
            // 每周的某一天
            const nowWeek =
                now.getDay() * 86400 +
                now.getHours() * 3600 +
                now.getMinutes() * 60 +
                now.getSeconds();
            const secondsUntilNext =
                ((period.week + 7) * 86400 +
                    period.minutes * 60 -
                    (nowWeek % (7 * 86400))) %
                    (7 * 86400) || 7 * 86400; // 计算到下一个指定星期几的秒数
            return (now.getTime() + secondsUntilNext) * 1000;
        } else if ("date" in period) {
            // 每月的某一天
            let targetDate = new Date(now);
            targetDate.setDate(period.date);
            if (targetDate.getMonth() > now.getMonth()) {
                targetDate.setMonth(targetDate.getMonth() + 1, 0);
            }
            targetDate.setHours(
                Math.floor(period.minutes / 60),
                period.minutes % 60,
                0,
                0
            );
            if (targetDate <= now) {
                // 如果今天已经过了这个时间，就设置到下个月
                targetDate.setMonth(now.getMonth() + 1, period.date);
                if (targetDate.getMonth() > now.getMonth() + 1) {
                    targetDate.setDate(0); // 设置为下个月的最后一天
                }
            }
            return targetDate.getTime();
        } else {
            // 每天
            let targetDate = new Date(now);
            targetDate.setHours(
                Math.floor(period.minutes / 60),
                period.minutes % 60,
                0,
                0
            );
            if (targetDate <= now) {
                return targetDate.getTime() + 86400000; // 明天
            }
            return targetDate.getTime();
        }
    }

    static formatTime(timestamp?: number, never_str: string = "从不"): string {
        if (timestamp === undefined) return never_str;
        const date = new Date(timestamp);
        return date.toLocaleString();
    }

    static targetUsagePercent(target: BackupTargetInfo): number {
        return TaskMgrHelper.percent(target.used, target.total);
    }
}
