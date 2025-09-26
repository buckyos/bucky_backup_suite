import React, { useEffect, useState } from "react";
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from "./ui/card";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";
import { Badge } from "./ui/badge";
import { ScrollArea } from "./ui/scroll-area";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogTrigger,
} from "./ui/alert-dialog";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "./ui/select";
import { toast } from "sonner";
import {
    Activity,
    HardDrive,
    Clock,
    CheckCircle,
    AlertTriangle,
    XCircle,
    Plus,
    Play,
    Server,
    Network,
    ArrowRight,
    MoreVertical,
} from "lucide-react";
import {
    // taskManager,
    TaskFilter,
    TaskInfo,
    BackupPlanInfo,
    BackupTargetInfo,
    ListOrder,
    ListTaskOrderBy,
    TaskEventType,
    TaskState,
} from "./utils/task_mgr";
import { taskManager } from "./utils/fake_task_mgr";
import { LoadingData, LoadingPage } from "./LoadingPage";
import { PlanState, TaskMgrHelper } from "./utils/task_mgr_helper";

interface DashboardProps {
    onNavigate?: (page: string, data?: any) => void;
}

export function Dashboard({ onNavigate }: DashboardProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [uncompleteTask, setUncompleteTask] = useState(
        new LoadingData<TaskInfo[]>(null)
    );
    const [lastCompletedTasks, setLastCompletedTasks] = useState(
        new LoadingData<TaskInfo[]>(null)
    );
    const [consumeSize, setConsumeSize] = useState(
        new LoadingData<{ total: number; today: number }>(null)
    );
    const [statistics, setStatistics] = useState(
        new LoadingData<{ complete: number; failed: number }>(null)
    );
    const [plans, setPlans] = useState(new LoadingData<BackupPlanInfo[]>(null));
    const [services, setServices] = useState(
        new LoadingData<BackupTargetInfo[]>(null)
    );
    const [selectedPlan, setSelectedPlan] = useState("");

    const loading = !(
        uncompleteTask.isLoaded() &&
        lastCompletedTasks.isLoaded() &&
        plans.isLoaded() &&
        services.isLoaded()
    );

    const loadingText = () => {
        if (!uncompleteTask.isLoaded()) return `${t.common.loading} 任务...`;
        if (!lastCompletedTasks.isLoaded())
            return `${t.common.loading} 最近任务...`;
        if (!plans.isLoaded()) return `${t.common.loading} 备份计划...`;
        if (!services.isLoaded()) return `${t.common.loading} 备份服务...`;
    };

    const refreshUncompleteTasks = () => {
        taskManager
            .listBackupTasks([
                TaskFilter.RUNNING,
                TaskFilter.PAUSED,
                TaskFilter.FAILED,
            ])
            .then(async (taskIds) => {
                const taskInfos = await Promise.all(
                    taskIds.map((taskid) => taskManager.getTaskInfo(taskid))
                );
                setUncompleteTask(new LoadingData(taskInfos));
            });
    };

    const refreshCompleteTasks = () => {
        taskManager
            .listBackupTasks(
                [TaskFilter.DONE],
                0,
                null,
                new Map([[ListTaskOrderBy.COMPLETE_TIME, ListOrder.DESC]])
            )
            .then(async (taskIds) => {
                const taskInfos = await Promise.all(
                    taskIds
                        .slice(0, 3)
                        .map((taskid) => taskManager.getTaskInfo(taskid))
                );
                setLastCompletedTasks(new LoadingData(taskInfos));
            });
    };

    const refreshConsumeSize = () => {
        taskManager.consumeSizeSummary().then((data) => {
            setConsumeSize(new LoadingData(data));
        });
    };

    const refreshBackupPlans = () => {
        taskManager.listBackupPlans().then(async (planIds) => {
            const planInfos = await Promise.all(
                planIds.map((planid) => taskManager.getBackupPlan(planid))
            );
            setPlans(new LoadingData(planInfos));
        });
    };

    const refreshTaskTargets = () => {
        taskManager.listBackupTargets().then(async (targetIds) => {
            const targetInfos = await Promise.all(
                targetIds.map((targetid) =>
                    taskManager.getBackupTarget(targetid)
                )
            );
            setServices(new LoadingData(targetInfos));
        });
    };

    const refreshStatistics = () => {
        // 过去30天的统计数据
        const now = Date.now();
        const last30DaysStart = new Date(now);
        last30DaysStart.setDate(last30DaysStart.getDate() - 30);
        taskManager
            .statisticsSummary(last30DaysStart.getTime(), now)
            .then((data) => {
                setStatistics(new LoadingData(data));
            });
    };

    const completePercent = () => {
        if (!statistics.isLoaded()) return "--";
        const stat = statistics.check();
        return TaskMgrHelper.percent(
            stat.complete,
            stat.complete + stat.failed
        ).toFixed(1);
    };

    console.log("Dashboard render", { loading });

    useEffect(() => {
        refreshUncompleteTasks();
        refreshCompleteTasks();
        refreshConsumeSize();
        refreshBackupPlans();
        refreshTaskTargets();

        const taskEventHandler = async (event: TaskEventType, data: any) => {
            console.log("task event:", event, data);
            switch (event) {
                case TaskEventType.CREATE_TASK:
                case TaskEventType.FAIL_TASK:
                case TaskEventType.PAUSE_TASK:
                case TaskEventType.RESUME_TASK:
                    refreshUncompleteTasks();
                    refreshStatistics();
                    break;
                case TaskEventType.UPDATE_TASK:
                case TaskEventType.COMPLETE_TASK:
                case TaskEventType.REMOVE_TASK:
                    refreshUncompleteTasks();
                    refreshCompleteTasks();
                    refreshConsumeSize();
                    refreshStatistics();
                    break;
                case TaskEventType.CREATE_PLAN:
                case TaskEventType.UPDATE_PLAN:
                case TaskEventType.REMOVE_PLAN:
                    refreshBackupPlans();
                    break;
                case TaskEventType.CREATE_TARGET:
                case TaskEventType.UPDATE_TARGET:
                case TaskEventType.REMOVE_TARGET:
                case TaskEventType.CHANGE_TARGET_STATE:
                    refreshTaskTargets();
                    break;
            }
        };

        taskManager.addTaskEventListener(taskEventHandler);
        const timerId = taskManager.startRefreshUncompleteTaskStateTimer();
        return () => {
            taskManager.removeTaskEventListener(taskEventHandler);
            taskManager.stopRefreshUncompleteTaskStateTimer(timerId);
        };
    }, []);

    const getStatusIcon = (status: string) => {
        switch (status) {
            case "success":
                return <CheckCircle className="w-3 h-3 text-green-500" />;
            case "warning":
                return <AlertTriangle className="w-3 h-3 text-yellow-500" />;
            case "error":
                return <XCircle className="w-3 h-3 text-red-500" />;
            default:
                return <Activity className="w-3 h-3" />;
        }
    };

    const getStatusBadge = (status: TaskState) => {
        switch (status) {
            case TaskState.RUNNING:
                return (
                    <Badge variant="default" className="bg-blue-500 text-xs">
                        执行中
                    </Badge>
                );
            case TaskState.DONE:
                return (
                    <Badge variant="secondary" className="bg-green-500 text-xs">
                        已完成
                    </Badge>
                );
            case TaskState.PAUSED:
                return (
                    <Badge variant="outline" className="text-xs">
                        暂停
                    </Badge>
                );
            case TaskState.FAILED:
                return (
                    <Badge variant="destructive" className="text-xs">
                        失败
                    </Badge>
                );
            case TaskState.PENDING:
                return (
                    <Badge variant="outline" className="text-xs">
                        等待中
                    </Badge>
                );
            default:
                return (
                    <Badge variant="outline" className="text-xs">
                        未知
                    </Badge>
                );
        }
    };

    const getServiceIcon = (type: string) => {
        return type === "local" ? HardDrive : Network;
    };

    const getServiceStatusBadge = (status: string) => {
        // todo: 检查服务状态
        switch (status) {
            case "healthy":
                return (
                    <Badge className="bg-green-100 text-green-800 text-xs">
                        正常
                    </Badge>
                );
            case "warning":
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                        警告
                    </Badge>
                );
            case "offline":
                return (
                    <Badge variant="destructive" className="text-xs">
                        离线
                    </Badge>
                );
            default:
                return (
                    <Badge variant="outline" className="text-xs">
                        未知
                    </Badge>
                );
        }
    };

    const getPlanStatusBadge = (status: PlanState) => {
        switch (status) {
            case PlanState.ACTIVE:
                return (
                    <Badge className="bg-green-100 text-green-800 text-xs">
                        正常
                    </Badge>
                );
            case PlanState.DISABLED:
                return (
                    <Badge variant="secondary" className="text-xs">
                        已禁用
                    </Badge>
                );
            case PlanState.WARNING:
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                        警告
                    </Badge>
                );
            case PlanState.ERROR:
                return (
                    <Badge variant="destructive" className="text-xs">
                        失败
                    </Badge>
                );
            default:
                return (
                    <Badge variant="outline" className="text-xs">
                        未知
                    </Badge>
                );
        }
    };

    const handleBackupNow = () => {
        const plan = plans.check().find((p) => p.plan_id === selectedPlan);
        if (plan) {
            toast.success(`已启动备份计划: ${plan.title}`);
        }
        setSelectedPlan("");
    };

    if (loading) {
        return (
            <div className={`${isMobile ? "p-4 pt-20" : "p-6"} space-y-4`}>
                {!isMobile && (
                    <div className="flex items-center justify-between">
                        <div>
                            <h1 className="mb-2">{t.dashboard.title}</h1>
                            <p className="text-muted-foreground">
                                {t.dashboard.subtitle}
                            </p>
                        </div>
                    </div>
                )}
                <LoadingPage status={loadingText()} />
            </div>
        );
    }

    return (
        <>
            <div className={`${isMobile ? "p-4 pt-20" : "p-6"} space-y-4`}>
                {!isMobile && (
                    <div className="flex items-center justify-between">
                        <div>
                            <h1 className="mb-2">{t.dashboard.title}</h1>
                            <p className="text-muted-foreground">
                                {t.dashboard.subtitle}
                            </p>
                        </div>
                        {services.check().length > 0 && (
                            <div className="flex gap-3">
                                <Button
                                    variant="outline"
                                    className="gap-2"
                                    onClick={() => onNavigate?.("create-plan")}
                                >
                                    <Plus className="w-4 h-4" />
                                    {t.dashboard.createNewPlan}
                                </Button>
                                {plans.check().length > 0 && (
                                    <AlertDialog>
                                        <AlertDialogTrigger asChild>
                                            <Button className="gap-2">
                                                <Play className="w-4 h-4" />
                                                {t.dashboard.backupNow}
                                            </Button>
                                        </AlertDialogTrigger>
                                        <AlertDialogContent>
                                            <AlertDialogHeader>
                                                <AlertDialogTitle>
                                                    {t.dashboard.backupNow}
                                                </AlertDialogTitle>
                                                <AlertDialogDescription>
                                                    选择要立即执行的备份计划
                                                </AlertDialogDescription>
                                            </AlertDialogHeader>
                                            <div className="space-y-4">
                                                <Select
                                                    value={selectedPlan}
                                                    onValueChange={
                                                        setSelectedPlan
                                                    }
                                                >
                                                    <SelectTrigger>
                                                        <SelectValue placeholder="选择备份计划" />
                                                    </SelectTrigger>
                                                    <SelectContent>
                                                        {plans
                                                            .check()
                                                            .filter(
                                                                (plan) =>
                                                                    plan.policy
                                                            )
                                                            .map((plan) => (
                                                                <SelectItem
                                                                    key={
                                                                        plan.plan_id
                                                                    }
                                                                    value={
                                                                        plan.plan_id
                                                                    }
                                                                >
                                                                    {plan.title}
                                                                </SelectItem>
                                                            ))}
                                                    </SelectContent>
                                                </Select>
                                            </div>
                                            <AlertDialogFooter>
                                                <AlertDialogCancel>
                                                    取消
                                                </AlertDialogCancel>
                                                <AlertDialogAction
                                                    disabled={!selectedPlan}
                                                    onClick={handleBackupNow}
                                                >
                                                    立即执行
                                                </AlertDialogAction>
                                            </AlertDialogFooter>
                                        </AlertDialogContent>
                                    </AlertDialog>
                                )}
                            </div>
                        )}
                    </div>
                )}

                {/* 状态概览卡片 */}
                <div
                    className={`grid ${
                        isMobile ? "grid-cols-2" : "grid-cols-4"
                    } gap-4`}
                >
                    <Card>
                        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                            <CardTitle
                                className={`${
                                    isMobile ? "text-xs" : "text-sm"
                                }`}
                            >
                                {t.dashboard.activeTasks}
                            </CardTitle>
                            <Activity className="h-4 w-4 text-muted-foreground" />
                        </CardHeader>
                        <CardContent>
                            <div
                                className={`${
                                    isMobile ? "text-xl" : "text-2xl"
                                } font-bold`}
                            >
                                {uncompleteTask.check().length}
                            </div>
                            <p className="text-xs text-muted-foreground">
                                {
                                    uncompleteTask
                                        .check()
                                        .filter(
                                            (task) =>
                                                task.state === TaskState.RUNNING
                                        ).length
                                }
                                个执行中
                            </p>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                            <CardTitle
                                className={`${
                                    isMobile ? "text-xs" : "text-sm"
                                }`}
                            >
                                {t.dashboard.totalBackupSize}
                            </CardTitle>
                            <HardDrive className="h-4 w-4 text-muted-foreground" />
                        </CardHeader>
                        <CardContent>
                            <div
                                className={`${
                                    isMobile ? "text-xl" : "text-2xl"
                                } font-bold`}
                            >
                                {consumeSize.isLoaded()
                                    ? TaskMgrHelper.formatSize(
                                          consumeSize.check().total
                                      )
                                    : "--"}
                            </div>
                            <p className="text-xs text-muted-foreground">
                                {consumeSize.isLoaded()
                                    ? "+" +
                                      TaskMgrHelper.formatSize(
                                          consumeSize.check().today
                                      ) +
                                      " 今日"
                                    : "--"}
                            </p>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                            <CardTitle
                                className={`${
                                    isMobile ? "text-xs" : "text-sm"
                                }`}
                            >
                                {t.dashboard.backupPlans}
                            </CardTitle>
                            <Clock className="h-4 w-4 text-muted-foreground" />
                        </CardHeader>
                        <CardContent>
                            <div
                                className={`${
                                    isMobile ? "text-xl" : "text-2xl"
                                } font-bold`}
                            >
                                {plans.check().length}
                            </div>
                            <p className="text-xs text-muted-foreground">
                                {
                                    plans.check().filter((plan) => plan.policy)
                                        .length
                                }
                                个已启用
                            </p>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                            <CardTitle
                                className={`${
                                    isMobile ? "text-xs" : "text-sm"
                                }`}
                            >
                                {t.dashboard.successRate}
                            </CardTitle>
                            <CheckCircle className="h-4 w-4 text-muted-foreground" />
                        </CardHeader>
                        <CardContent>
                            <div
                                className={`${
                                    isMobile ? "text-xl" : "text-2xl"
                                } font-bold`}
                            >
                                {completePercent()}%
                            </div>
                            <p className="text-xs text-muted-foreground">
                                过去30天
                            </p>
                        </CardContent>
                    </Card>
                </div>

                {/* 主要内容区域 */}
                <div className="space-y-4">
                    {/* 当前任务 - 只显示执行中的任务 */}
                    <Card>
                        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
                            <div>
                                <CardTitle className="text-base">
                                    {t.dashboard.currentTasks}
                                </CardTitle>
                                <CardDescription className="text-sm">
                                    正在执行的备份任务
                                </CardDescription>
                            </div>
                            <Button
                                variant="ghost"
                                size="sm"
                                className="gap-1 text-xs"
                                onClick={() => onNavigate?.("tasks")}
                            >
                                {t.dashboard.viewAll}
                                <ArrowRight className="w-3 h-3" />
                            </Button>
                        </CardHeader>
                        <CardContent>
                            {uncompleteTask.check().length === 0 &&
                            services.check().length > 0 &&
                            plans.check().length > 0 ? (
                                <div
                                    className={`flex ${
                                        isMobile
                                            ? "flex-col gap-2"
                                            : "items-center gap-3"
                                    } justify-center`}
                                >
                                    <Button
                                        onClick={() => onNavigate?.("plans")}
                                        className="gap-2"
                                    >
                                        <Plus className="w-4 h-4" />
                                        前往计划列表执行一次备份
                                    </Button>
                                </div>
                            ) : isMobile ? (
                                <ScrollArea className="h-20">
                                    <div className="grid grid-cols-2 gap-3">
                                        {uncompleteTask.check().map((task) => (
                                            <div
                                                key={task.taskid}
                                                className="flex items-center gap-2 text-sm cursor-pointer"
                                                onClick={() =>
                                                    onNavigate?.("tasks")
                                                }
                                            >
                                                <div className="flex-1 min-w-0">
                                                    <div className="flex items-center gap-2 mb-1">
                                                        <span className="font-medium truncate">
                                                            {task.name}
                                                        </span>
                                                        {getStatusBadge(
                                                            task.state
                                                        )}
                                                    </div>
                                                    <div className="space-y-1">
                                                        <Progress
                                                            value={TaskMgrHelper.taskProgress(
                                                                task
                                                            )}
                                                            className="h-1"
                                                        />
                                                        <div className="flex justify-between text-xs text-muted-foreground">
                                                            <span>
                                                                {TaskMgrHelper.taskProgress(
                                                                    task
                                                                )}
                                                                %
                                                            </span>
                                                            <span>
                                                                {TaskMgrHelper.taskRemainingStr(
                                                                    task
                                                                )}
                                                            </span>
                                                        </div>
                                                    </div>
                                                </div>
                                            </div>
                                        ))}
                                        {/* {uncompleteTask.check().length ===
                                            0 && (
                                            <div className="text-center py-4">
                                                {plans.check().length === 0 ? (
                                                    <div
                                                        className={`flex ${
                                                            isMobile
                                                                ? "flex-col gap-2"
                                                                : "items-center gap-3"
                                                        } justify-center`}
                                                    >
                                                        {services.check()
                                                            .length === 0 ? (
                                                            <>
                                                                <Button
                                                                    onClick={() =>
                                                                        onNavigate?.(
                                                                            "add-service"
                                                                        )
                                                                    }
                                                                    size="sm"
                                                                    className="gap-2"
                                                                >
                                                                    去配置备份服务
                                                                </Button>
                                                                {!isMobile && (
                                                                    <span className="text-muted-foreground text-sm">
                                                                        或
                                                                    </span>
                                                                )}
                                                                <Button
                                                                    variant="outline"
                                                                    size="sm"
                                                                    className="gap-2"
                                                                    disabled
                                                                >
                                                                    新建备份计划
                                                                </Button>
                                                            </>
                                                        ) : (
                                                            <Button
                                                                onClick={() =>
                                                                    onNavigate?.(
                                                                        "create-plan"
                                                                    )
                                                                }
                                                                size="sm"
                                                                className="gap-2"
                                                            >
                                                                新建备份计划
                                                            </Button>
                                                        )}
                                                    </div>
                                                ) : (
                                                    <Button
                                                        onClick={() =>
                                                            onNavigate?.(
                                                                "plans"
                                                            )
                                                        }
                                                        size="sm"
                                                        className="gap-2"
                                                    >
                                                        前往计划列表执行一次备份
                                                    </Button>
                                                )}
                                            </div>
                                        )} */}
                                    </div>
                                </ScrollArea>
                            ) : (
                                <div className="grid grid-cols-3 gap-3">
                                    {/* {uncompleteTask.check().length === 0 && (
                                        <div className="text-center py-2">
                                            {plans.check().length === 0 ? (
                                                <div
                                                    className={`flex ${
                                                        isMobile
                                                            ? "flex-col gap-2"
                                                            : "items-center gap-3"
                                                    } justify-center`}
                                                >
                                                    {services.check().length ===
                                                    0 ? (
                                                        <>
                                                            <Button
                                                                onClick={() =>
                                                                    onNavigate?.(
                                                                        "add-service"
                                                                    )
                                                                }
                                                                size="sm"
                                                                className="gap-2"
                                                            >
                                                                去配置备份服务
                                                            </Button>
                                                            {!isMobile && (
                                                                <span className="text-muted-foreground text-sm">
                                                                    或
                                                                </span>
                                                            )}
                                                            <Button
                                                                variant="outline"
                                                                size="sm"
                                                                className="gap-2"
                                                                disabled
                                                            >
                                                                新建备份计划
                                                            </Button>
                                                        </>
                                                    ) : (
                                                        <Button
                                                            onClick={() =>
                                                                onNavigate?.(
                                                                    "create-plan"
                                                                )
                                                            }
                                                            size="sm"
                                                            className="gap-2"
                                                        >
                                                            新建备份计划
                                                        </Button>
                                                    )}
                                                </div>
                                            ) : (
                                                <Button
                                                    onClick={() =>
                                                        onNavigate?.("plans")
                                                    }
                                                    size="sm"
                                                    className="gap-2"
                                                >
                                                    前往计划列表执行一次备份
                                                </Button>
                                            )}
                                        </div>
                                    )} */}
                                    {uncompleteTask.check().map((task) => (
                                        <div
                                            key={task.taskid}
                                            className="flex items-center gap-3"
                                        >
                                            <div className="flex-1 min-w-0">
                                                <div className="flex items-center gap-2 mb-1">
                                                    <span className="font-medium truncate">
                                                        {task.name}
                                                    </span>
                                                    {getStatusBadge(task.state)}
                                                </div>
                                                <div className="space-y-1">
                                                    <Progress
                                                        value={TaskMgrHelper.taskProgress(
                                                            task
                                                        )}
                                                        className="h-1.5"
                                                    />
                                                    <div className="flex justify-between text-xs text-muted-foreground">
                                                        <span>
                                                            {TaskMgrHelper.taskCompletedStr(
                                                                task
                                                            )}
                                                            % •{" "}
                                                            {TaskMgrHelper.taskSpeedStr(
                                                                task
                                                            )}
                                                        </span>
                                                        <span>
                                                            {TaskMgrHelper.taskRemainingStr(
                                                                task
                                                            )}{" "}
                                                            • ETA{" "}
                                                            {TaskMgrHelper.taskETA(
                                                                task
                                                            )}
                                                        </span>
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </CardContent>
                    </Card>

                    {/* 备份服务 */}
                    <Card>
                        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
                            <div>
                                <CardTitle className="text-base">
                                    {t.dashboard.backupServices}
                                </CardTitle>
                                <CardDescription className="text-sm">
                                    备份目标服务状态
                                </CardDescription>
                            </div>
                            <div className="flex gap-2">
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className="gap-1 text-xs"
                                    onClick={() => onNavigate?.("services")}
                                >
                                    {t.dashboard.viewAll}
                                    <ArrowRight className="w-3 h-3" />
                                </Button>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="gap-1 text-xs"
                                    onClick={() => onNavigate?.("add-service")}
                                >
                                    <Plus className="w-3 h-3" />
                                    {isMobile ? "" : t.dashboard.addService}
                                </Button>
                            </div>
                        </CardHeader>
                        <CardContent>
                            {services.check().length === 0 ? (
                                <div
                                    className={`flex ${
                                        isMobile
                                            ? "flex-col gap-2"
                                            : "items-center gap-3"
                                    } justify-center`}
                                >
                                    <Button
                                        onClick={() =>
                                            onNavigate?.("add-service")
                                        }
                                        className="gap-2"
                                    >
                                        <Plus className="w-4 h-4" />
                                        去配置备份服务
                                    </Button>
                                </div>
                            ) : isMobile ? (
                                <ScrollArea className="h-20">
                                    <div className="space-y-2">
                                        {services.check().map((service) => {
                                            const ServiceIcon = getServiceIcon(
                                                service.target_type
                                            );
                                            const usagePercent =
                                                TaskMgrHelper.targetUsagePercent(
                                                    service
                                                );
                                            return (
                                                <div
                                                    key={service.target_id}
                                                    className="flex flex-col gap-2 text-sm cursor-pointer border rounded-lg p-3 hover:bg-accent/30"
                                                    onClick={() =>
                                                        onNavigate?.("services")
                                                    }
                                                >
                                                    <ServiceIcon className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                                                    <div className="flex-1 min-w-0">
                                                        <div className="flex items-center gap-2 mb-1">
                                                            <span className="font-medium truncate">
                                                                {service.name}
                                                            </span>
                                                            {getServiceStatusBadge(
                                                                service.state
                                                            )}
                                                        </div>
                                                        <div className="flex items-center gap-2 text-xs text-muted-foreground">
                                                            <span>
                                                                {service.used}
                                                            </span>
                                                            {service.used >
                                                                0 && (
                                                                <>
                                                                    <span>
                                                                        •
                                                                    </span>
                                                                    <div className="flex items-center gap-1">
                                                                        <div className="w-8 bg-secondary rounded-full h-1">
                                                                            <div
                                                                                className={`h-1 rounded-full ${
                                                                                    usagePercent >
                                                                                    90
                                                                                        ? "bg-red-500"
                                                                                        : usagePercent >
                                                                                          70
                                                                                        ? "bg-yellow-500"
                                                                                        : "bg-green-500"
                                                                                }`}
                                                                                style={{
                                                                                    width: `${usagePercent}%`,
                                                                                }}
                                                                            />
                                                                        </div>
                                                                        <span>
                                                                            {
                                                                                usagePercent
                                                                            }
                                                                            %
                                                                        </span>
                                                                    </div>
                                                                </>
                                                            )}
                                                        </div>
                                                    </div>
                                                </div>
                                            );
                                        })}
                                    </div>
                                </ScrollArea>
                            ) : (
                                <div className="grid grid-cols-3 gap-3">
                                    {services.check().map((service) => {
                                        const ServiceIcon = getServiceIcon(
                                            service.target_type
                                        );
                                        const usagePercent =
                                            TaskMgrHelper.targetUsagePercent(
                                                service
                                            );
                                        return (
                                            <div
                                                key={service.target_id}
                                                className="flex flex-col gap-2 border rounded-lg p-3 hover:bg-accent/30"
                                            >
                                                <ServiceIcon className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                                                <div className="flex-1 min-w-0">
                                                    <div className="flex items-center gap-2 mb-1">
                                                        <span className="font-medium truncate">
                                                            {service.name}
                                                        </span>
                                                        {getServiceStatusBadge(
                                                            service.state
                                                        )}
                                                    </div>
                                                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                                                        <span>
                                                            {service.used} /{" "}
                                                            {service.total}
                                                        </span>
                                                        {usagePercent > 0 && (
                                                            <>
                                                                <span>•</span>
                                                                <div className="flex items-center gap-1">
                                                                    <div className="w-12 bg-secondary rounded-full h-1">
                                                                        <div
                                                                            className={`h-1 rounded-full ${
                                                                                usagePercent >
                                                                                90
                                                                                    ? "bg-red-500"
                                                                                    : usagePercent >
                                                                                      70
                                                                                    ? "bg-yellow-500"
                                                                                    : "bg-green-500"
                                                                            }`}
                                                                            style={{
                                                                                width: `${usagePercent}%`,
                                                                            }}
                                                                        />
                                                                    </div>
                                                                    <span>
                                                                        {
                                                                            usagePercent
                                                                        }
                                                                        %
                                                                    </span>
                                                                </div>
                                                            </>
                                                        )}
                                                    </div>
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            )}
                        </CardContent>
                    </Card>

                    {/* 备份计划 */}
                    <Card>
                        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
                            <div>
                                <CardTitle className="text-base">
                                    {t.dashboard.backupPlans}
                                </CardTitle>
                                <CardDescription className="text-sm">
                                    自动备份计划列表
                                </CardDescription>
                            </div>
                            <div className="flex gap-2">
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className="gap-1 text-xs"
                                    onClick={() => onNavigate?.("plans")}
                                >
                                    {t.dashboard.viewAll}
                                    <ArrowRight className="w-3 h-3" />
                                </Button>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="gap-1 text-xs"
                                    onClick={() => onNavigate?.("create-plan")}
                                >
                                    <Plus className="w-3 h-3" />
                                    {isMobile ? "" : t.dashboard.createNewPlan}
                                </Button>
                            </div>
                        </CardHeader>
                        <CardContent>
                            {plans.check().length === 0 ? (
                                <div
                                    className={`flex ${
                                        isMobile
                                            ? "flex-col gap-2"
                                            : "items-center gap-3"
                                    } justify-center`}
                                >
                                    {services.check().length > 0 && (
                                        <Button
                                            onClick={() =>
                                                onNavigate?.("create-plan")
                                            }
                                            className="gap-2"
                                        >
                                            <Plus className="w-4 h-4" />
                                            新建备份计划
                                        </Button>
                                    )}
                                </div>
                            ) : isMobile ? (
                                <ScrollArea className="h-24">
                                    <div className="grid grid-cols-2 gap-3">
                                        {plans.check().map((plan) => {
                                            const state =
                                                TaskMgrHelper.planState(
                                                    plan,
                                                    uncompleteTask.check()
                                                );
                                            return (
                                                <div
                                                    key={plan.plan_id}
                                                    className="flex flex-col gap-2 text-sm cursor-pointer border rounded-lg p-3 hover:bg-accent/30"
                                                    onClick={() =>
                                                        onNavigate?.("plans")
                                                    }
                                                >
                                                    <div className="flex items-center gap-2 flex-1 min-w-0">
                                                        <div className="flex-1">
                                                            <div className="flex items-center gap-2 mb-1">
                                                                <span className="font-medium truncate">
                                                                    {plan.title}
                                                                </span>
                                                                {getPlanStatusBadge(
                                                                    state
                                                                )}
                                                            </div>
                                                            <div className="text-xs text-muted-foreground">
                                                                {TaskMgrHelper.planNextRunTime(
                                                                    plan
                                                                )}
                                                            </div>
                                                        </div>
                                                    </div>
                                                    <Button
                                                        variant="ghost"
                                                        size="sm"
                                                        className="p-1 h-6 w-6 flex-shrink-0"
                                                        disabled={!plan.policy}
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            toast.success(
                                                                `正在启动计划: ${plan.title}`
                                                            );
                                                        }}
                                                    >
                                                        <Play className="w-3 h-3" />
                                                    </Button>
                                                </div>
                                            );
                                        })}
                                    </div>
                                </ScrollArea>
                            ) : (
                                <div className="space-y-3">
                                    {plans.check().map((plan) => (
                                        <div
                                            key={plan.plan_id}
                                            className="flex flex-col gap-2 border rounded-lg p-3 hover:bg-accent/30"
                                        >
                                            <div className="flex items-center gap-3 flex-1 min-w-0">
                                                <div className="flex-1">
                                                    <div className="flex items-center gap-2 mb-1">
                                                        <span className="font-medium truncate">
                                                            {plan.title}
                                                        </span>
                                                        {getPlanStatusBadge(
                                                            TaskMgrHelper.planState(
                                                                plan,
                                                                uncompleteTask.check()
                                                            )
                                                        )}
                                                    </div>
                                                    <div className="text-xs text-muted-foreground">
                                                        下次执行:{" "}
                                                        {TaskMgrHelper.planNextRunTime(
                                                            plan
                                                        )}
                                                    </div>
                                                </div>
                                            </div>
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                className="gap-1 text-xs flex-shrink-0"
                                                disabled={!plan.policy}
                                                onClick={() =>
                                                    toast.success(
                                                        `正在启动计划: ${plan.title}`
                                                    )
                                                }
                                            >
                                                <Play className="w-3 h-3" />
                                                执行
                                            </Button>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </CardContent>
                    </Card>

                    {/* 近期活动 */}
                    <Card>
                        <CardHeader>
                            <CardTitle className="text-base">
                                {t.dashboard.recentActivities}
                            </CardTitle>
                            <CardDescription className="text-sm">
                                最近完成的备份任务记录
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            <div className="space-y-3">
                                {lastCompletedTasks.check().map((task) => (
                                    <div
                                        key={task.taskid}
                                        className={`flex items-center justify-between ${
                                            isMobile ? "text-sm" : ""
                                        }`}
                                    >
                                        <div className="flex items-center gap-3 flex-1 min-w-0">
                                            {getStatusIcon(task.state)}
                                            <div className="flex-1 min-w-0">
                                                <p className="font-medium truncate">
                                                    {task.name}
                                                </p>
                                                <p className="text-xs text-muted-foreground">
                                                    {task.update_time} •{" "}
                                                    {task.total_size}
                                                </p>
                                            </div>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </CardContent>
                    </Card>
                </div>

                {/* 移动端快速操作按钮 */}
                {isMobile && (
                    <div className="fixed bottom-4 right-4 flex flex-col gap-3">
                        <AlertDialog>
                            <AlertDialogTrigger asChild>
                                <Button
                                    size="sm"
                                    className="rounded-full gap-2 shadow-lg"
                                >
                                    <Play className="w-4 h-4" />
                                    {t.dashboard.backupNow}
                                </Button>
                            </AlertDialogTrigger>
                            <AlertDialogContent>
                                <AlertDialogHeader>
                                    <AlertDialogTitle>
                                        {t.dashboard.backupNow}
                                    </AlertDialogTitle>
                                    <AlertDialogDescription>
                                        选择要立即执行的备份计划
                                    </AlertDialogDescription>
                                </AlertDialogHeader>
                                <div className="space-y-4">
                                    <Select
                                        value={selectedPlan}
                                        onValueChange={setSelectedPlan}
                                    >
                                        <SelectTrigger>
                                            <SelectValue placeholder="选择备份计划" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            {plans.check().map((plan) => (
                                                <SelectItem
                                                    key={plan.plan_id}
                                                    value={plan.plan_id}
                                                >
                                                    {plan.title}
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>
                                </div>
                                <AlertDialogFooter>
                                    <AlertDialogCancel>取消</AlertDialogCancel>
                                    <AlertDialogAction
                                        disabled={!selectedPlan}
                                        onClick={handleBackupNow}
                                    >
                                        立即执行
                                    </AlertDialogAction>
                                </AlertDialogFooter>
                            </AlertDialogContent>
                        </AlertDialog>
                        <Button
                            variant="outline"
                            size="sm"
                            className="rounded-full gap-2 shadow-lg bg-background"
                            onClick={() => onNavigate?.("create-plan")}
                        >
                            <Plus className="w-4 h-4" />
                            {t.dashboard.createNewPlan}
                        </Button>
                    </div>
                )}
            </div>
        </>
    );
}
