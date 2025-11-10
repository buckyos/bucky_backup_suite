import {
    BackupLog,
    BackupPlanInfo,
    BackupTargetInfo,
    TaskInfo,
} from "./task_mgr";
import {
    taskManager as taskManagerInner,} from "./task_mgr";

export const taskManager = taskManagerInner;

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
        if (typeof size !== 'number') return "NAN";
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
        return this.formatSize(task.speed || 0) + "/s";
    }

    static taskETA(task: TaskInfo): string {
        if (!task.speed) return "∞";
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
            return plan.policy_disabled || plan.policy.length === 0
                ? PlanState.DISABLED
                : PlanState.ACTIVE;
        } else {
            const latestTask = myUncompleteTasks[0];
            if (latestTask.state === "FAILED") {
                return PlanState.ERROR;
            } else if (myUncompleteTasks.find((t) => t.state === "FAILED")) {
                return PlanState.WARNING;
            } else {
                return plan.policy_disabled || plan.policy.length === 0
                    ? PlanState.DISABLED
                    : PlanState.ACTIVE;
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
            return now.getTime() + secondsUntilNext * 1000;
        } else if ("date" in period) {
            // 每月的某一天
            let targetDate = new Date(now);
            targetDate.setDate(period.date);
            if (targetDate.getMonth() > now.getMonth()) {
                targetDate.setMonth(now.getMonth() + 1, 0); // 设置为下个月的最后一天
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

    static formatPlanPolicy(plan: BackupPlanInfo): string[] {
        let result: string[] = ["手动"];
        if (plan.policy.length === 0 || plan.policy_disabled) return result;
        for (const p of plan.policy) {
            if ("minutes" in p) {
                if ("week" in p) {
                    const weekDays = [
                        "星期一",
                        "星期二",
                        "星期三",
                        "星期四",
                        "星期五",
                        "星期六",
                        "星期日",
                    ];
                    result.push(
                        `每周: ${
                            weekDays[p.week]
                        } ${TaskMgrHelper.formatMinutesToHHMM(p.minutes)}`
                    );
                } else if ("date" in p) {
                    result.push(
                        `每月: ${p.date}日 ${TaskMgrHelper.formatMinutesToHHMM(
                            p.minutes
                        )}`
                    );
                } else {
                    result.push(
                        `每天: ${TaskMgrHelper.formatMinutesToHHMM(p.minutes)}`
                    );
                }
            } else if ("update_delay" in p) {
                result.push(`事件: 更新延迟${p.update_delay}秒`);
            }
        }
        return result;
    }

    static formatTime(timestamp?: number, never_str: string = "从不"): string {
        if (timestamp === undefined || timestamp === null) {
            return never_str;
        }

        // 后端返回的时间戳是 Unix 秒，需要转换成毫秒后再格式化
        const msTimestamp = timestamp > 0 && timestamp < 1_000_000_000_000
            ? timestamp * 1000
            : timestamp;
        if (msTimestamp <= 0) {
            return never_str;
        }

        const date = new Date(msTimestamp);
        if (Number.isNaN(date.getTime())) {
            return never_str;
        }

        return date.toLocaleString();
    }

    static targetUsagePercent(target: BackupTargetInfo): number {
        return TaskMgrHelper.percent(target.used, target.total);
    }

    static formatMinutesToHHMM(minutes?: number): string {
        if (minutes === undefined) return "";
        const hours = Math.floor(minutes / 60);
        const mins = minutes % 60;
        return `${String(hours).padStart(2, "0")}:${String(mins).padStart(
            2,
            "0"
        )}`;
    }

    static minutesFromHHMM(hhmm: string): number | null {
        const match = hhmm.match(/^(\d{2}):(\d{2})$/);
        if (!match) return null;
        const hours = parseInt(match[1], 10);
        const minutes = parseInt(match[2], 10);
        return hours * 60 + minutes;
    }

    static formatLog(log: BackupLog): string {
        return `${log.params}`;
    }
}
