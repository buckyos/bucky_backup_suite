import React, { useEffect, useState } from "react";
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from "./ui/card";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { Switch } from "./ui/switch";
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
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { LoadingPage } from "./LoadingPage";
import { toast } from "sonner";
import {
    Plus,
    Play,
    Edit,
    Eye,
    Trash2,
    Calendar,
    Folder,
    Server,
    ArrowLeft,
    Undo2,
} from "lucide-react";
import {
    BackupPlanInfo,
    BackupTargetInfo,
    TaskFilter,
    TaskInfo,
    TaskState,
} from "./utils/task_mgr";
import { taskManager } from "./utils/fake_task_mgr";
import { PlanState, TaskMgrHelper } from "./utils/task_mgr_helper";
import { Translations } from "./i18n";

interface BackupPlansProps {
    onNavigate?: (page: string, data?: any) => void;
}
export function BackupPlans({ onNavigate }: BackupPlansProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [loading, setLoading] = useState(true);
    const [plans, setPlans] = useState<BackupPlanInfo[]>([]);
    const [services, setServices] = useState<BackupTargetInfo[]>([]);
    const [uncompleteTasks, setUncompleteTasks] = useState<TaskInfo[]>([]);

    useEffect(() => {
        taskManager.listBackupPlans().then(async (planIds) => {
            const planDetails = await Promise.all(
                planIds.map((id) => taskManager.getBackupPlan(id))
            );
            setPlans(planDetails);

            if (planDetails.length > 0) {
                const targetIds = await taskManager.listBackupTargets();
                const targetDetails = await Promise.all(
                    targetIds.map((id) => taskManager.getBackupTarget(id))
                );
                setServices(targetDetails);
            }
            setLoading(false);
        });
        taskManager
            .listBackupTasks({
                state: [
                    TaskState.FAILED,
                    TaskState.PAUSED,
                    TaskState.RUNNING,
                    TaskState.PENDING,
                ],
            })
            .then(async ({ task_ids }) => {
                const uncompleteTasks = await Promise.all(
                    task_ids.map((id) => taskManager.getTaskInfo(id))
                );
                setUncompleteTasks(uncompleteTasks);
            });
        const timerId = taskManager.startRefreshUncompleteTaskStateTimer();
        return () => {
            taskManager.stopRefreshUncompleteTaskStateTimer(timerId);
        };
    }, []);

    if (loading) {
        return (
            <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-6`}>
                <div>
                    <h1 className="mb-2">{t.plans.title}</h1>
                    <p className="text-muted-foreground">{t.plans.subtitle}</p>
                </div>
                <LoadingPage status={`${t.common.loading} ${t.nav.plans}...`} />
            </div>
        );
    }

    // 检查是否有未完成的任务
    const hasRunningTasks = (planId: string) => {
        return (
            uncompleteTasks.find((t) => t.owner_plan_id === planId) !==
            undefined
        );
    };

    const togglePlan = async (plan: BackupPlanInfo) => {
        const success = await taskManager.updateBackupPlan({
            ...plan,
            policy_disabled: !plan.policy_disabled,
        });

        if (!success) {
            toast.error("更新备份计划状态失败");
            return;
        } else {
            plan.policy_disabled = !plan.policy_disabled;
            setPlans([...plans]);
        }
    };

    const deletePlan = async (planId: string) => {
        const success = await taskManager.removeBackupPlan(planId);
        if (!success) {
            toast.error("删除备份计划失败");
        } else {
            // 删除成功
            setPlans(plans.filter((plan) => plan.plan_id !== planId));
            toast.success("备份计划已删除");
        }
    };

    const runPlan = async (plan: BackupPlanInfo) => {
        if (hasRunningTasks(plan.plan_id)) {
            toast.error("当前有任务正在执行，请等待完成或删除现有任务");
            return;
        }
        await taskManager.createBackupTask(plan.plan_id);
        toast.success(`正在启动备份计划: ${plan.title}`);
    };

    return (
        <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-6`}>
            {/* 头部 */}
            <div className="flex items-center justify-between">
                <div>
                    {!isMobile && (
                        <>
                            <h1 className="mb-2">{t.plans.title}</h1>
                            <p className="text-muted-foreground">
                                {t.plans.subtitle}
                            </p>
                        </>
                    )}
                </div>
                <Button
                    className={`gap-2 ${isMobile ? "px-3" : ""}`}
                    onClick={() => onNavigate?.("create-plan")}
                >
                    <Plus className="w-4 h-4" />
                    {isMobile ? "" : t.plans.createNew}
                </Button>
            </div>

            {/* 计划列表 */}
            <div className="grid gap-4">
                {plans.length === 0 ? (
                    <Card
                        className={`w-full ${
                            isMobile ? "" : "max-w-2xl"
                        } text-center m-auto`}
                    >
                        <CardHeader>
                            <CardTitle>
                                {services.length === 0
                                    ? "还没有可用的备份服务"
                                    : "还没有备份计划"}
                            </CardTitle>
                            <CardDescription>
                                {services.length === 0
                                    ? "创建备份计划前，请先配置一个备份服务"
                                    : "创建你的第一个备份计划以保护重要数据"}
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            <div
                                className={`flex ${
                                    isMobile
                                        ? "flex-col gap-2"
                                        : "items-center justify-center gap-3"
                                }`}
                            >
                                {services.length === 0 ? (
                                    <>
                                        <Button
                                            onClick={() =>
                                                onNavigate?.("services")
                                            }
                                            className="gap-2"
                                        >
                                            <Server className="w-4 h-4" />
                                            去配置备份服务
                                        </Button>
                                        {!isMobile && (
                                            <span className="text-muted-foreground">
                                                或
                                            </span>
                                        )}
                                        <Button
                                            variant="outline"
                                            onClick={() =>
                                                onNavigate?.("create-plan")
                                            }
                                            className="gap-2"
                                            disabled
                                        >
                                            <Plus className="w-4 h-4" />
                                            新建备份计划
                                        </Button>
                                    </>
                                ) : (
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
                        </CardContent>
                    </Card>
                ) : isMobile ? (
                    <PlanListMobile
                        plans={plans}
                        services={services}
                        uncompleteTasks={uncompleteTasks}
                        t={t}
                        togglePlan={togglePlan}
                        runPlan={runPlan}
                        deletePlan={deletePlan}
                        onNavigate={onNavigate}
                    />
                ) : (
                    <PlanListDesktop
                        plans={plans}
                        services={services}
                        uncompleteTasks={uncompleteTasks}
                        t={t}
                        togglePlan={togglePlan}
                        runPlan={runPlan}
                        deletePlan={deletePlan}
                        onNavigate={onNavigate}
                    />
                )}
            </div>

            {/* 统计信息 */}
            {!isMobile && (
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-base">
                                总计划数
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <p className="text-2xl">{plans.length}</p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-base">
                                启用计划
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <p className="text-2xl">
                                {
                                    plans.filter(
                                        (p) =>
                                            !(
                                                p.policy_disabled ||
                                                p.policy.length === 0
                                            )
                                    ).length
                                }
                            </p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-base">
                                需要关注
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <p className="text-2xl">
                                {
                                    plans.filter((p) => {
                                        const state = TaskMgrHelper.planState(
                                            p,
                                            uncompleteTasks
                                        );
                                        return (
                                            state === PlanState.WARNING ||
                                            state === PlanState.ERROR
                                        );
                                    }).length
                                }
                            </p>
                        </CardContent>
                    </Card>
                </div>
            )}
        </div>
    );
}

function PlanListMobile({
    plans,
    services,
    uncompleteTasks,
    t,
    togglePlan,
    runPlan,
    deletePlan,
    onNavigate,
}: {
    plans: Array<BackupPlanInfo>;
    services: Array<BackupTargetInfo>;
    uncompleteTasks: TaskInfo[];
    t: Translations;
    togglePlan: (plan: BackupPlanInfo) => void;
    runPlan: (plan: BackupPlanInfo) => void;
    deletePlan: (planId: string) => void;
    onNavigate?: (page: string, data?: any) => void;
}) {
    return plans.map((plan) => {
        const policies = TaskMgrHelper.formatPlanPolicy(plan);
        const service = services.find((s) => s.target_id === plan.target);
        return (
            <Card
                key={plan.plan_id}
                className={`transition-all gap-4 ${
                    plan.policy_disabled || plan.policy.length === 0
                        ? "opacity-60"
                        : ""
                }`}
                onClick={() => onNavigate?.("plan-details", plan)}
            >
                <CardHeader className="pt-4 pb-2">
                    <div className="flex items-start justify-between">
                        <div className="flex-1">
                            <div className="flex items-center gap-3 mb-2">
                                <CardTitle className="text-base">
                                    {plan.title}
                                </CardTitle>
                                {getStatusBadge(plan, uncompleteTasks)}
                            </div>
                        </div>
                        <div className="flex items-center gap-3">
                            <Switch
                                disabled={plan.policy.length === 0}
                                checked={
                                    !(
                                        plan.policy_disabled ||
                                        plan.policy.length === 0
                                    )
                                }
                                onClick={(event) => event.stopPropagation()}
                                onCheckedChange={() => togglePlan(plan)}
                            />
                        </div>
                    </div>
                </CardHeader>
                <CardContent className="pt-0 pb-4 [&:last-child]:pb-4">
                    <div className="grid grid-cols-1 gap-4 mb-4">
                        <div className="flex items-center gap-2">
                            <Folder className="w-4 h-4 text-muted-foreground" />
                            <div>
                                <p className="text-sm text-muted-foreground">
                                    {t.plans.source}
                                </p>
                                <p className="font-medium truncate text-sm">
                                    {plan.source}
                                </p>
                            </div>
                        </div>
                        <div className="flex items-center gap-2">
                            <Server className="w-4 h-4 text-muted-foreground" />
                            <div>
                                <p className="text-sm text-muted-foreground">
                                    {t.plans.destination}
                                </p>
                                <p className="font-medium text-sm">
                                    {service
                                        ? service.name || service.url
                                        : "未知服务"}
                                </p>
                            </div>
                        </div>
                        <div className="flex items-center gap-2">
                            <Calendar className="w-4 h-4 text-muted-foreground" />
                            <div>
                                <p className="text-sm text-muted-foreground">
                                    {t.plans.schedule}
                                </p>
                                <p className="font-medium text-sm">
                                    {policies.map((s, idx) => (
                                        <span key={idx}>
                                            {idx === policies.length - 1
                                                ? `${s}`
                                                : `${s}|`}
                                        </span>
                                    ))}
                                </p>
                            </div>
                        </div>
                        <div>
                            <p className="text-sm text-muted-foreground">
                                {t.plans.nextRun}
                            </p>
                            <p className="font-medium text-sm">
                                {TaskMgrHelper.formatTime(
                                    TaskMgrHelper.planNextRunTime(plan),
                                    "--"
                                )}
                            </p>
                        </div>
                    </div>

                    <div className="flex items-center justify-between pt-4 border-t">
                        <div className="text-muted-foreground text-xs">
                            {t.plans.lastRun}:{" "}
                            {TaskMgrHelper.formatTime(plan.last_run_time, "--")}
                        </div>
                        <div className="flex items-center gap-2">
                            <Button
                                variant="outline"
                                size="sm"
                                className="p-2"
                                onClick={(event) => {
                                    event.stopPropagation();
                                    runPlan(plan);
                                }}
                            >
                                <Play className="w-3 h-3" />
                            </Button>
                            <Button
                                variant="outline"
                                size="sm"
                                className="p-2"
                                onClick={(event) => {
                                    event.stopPropagation();
                                    onNavigate?.("restore", {
                                        planId: plan.plan_id,
                                    });
                                }}
                            >
                                <Undo2 className="w-3 h-3" />
                            </Button>
                            <Button
                                variant="outline"
                                size="sm"
                                className="p-2"
                                onClick={(event) => {
                                    event.stopPropagation();
                                    onNavigate?.("edit-plan", plan);
                                }}
                            >
                                <Edit className="w-3 h-3" />
                            </Button>
                            <AlertDialog>
                                <AlertDialogTrigger asChild>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        className="p-2 text-destructive hover:text-destructive-foreground hover:bg-destructive"
                                        onClick={(event) =>
                                            event.stopPropagation()
                                        }
                                    >
                                        <Trash2 className="w-3 h-3" />
                                    </Button>
                                </AlertDialogTrigger>
                                <AlertDialogContent>
                                    <AlertDialogHeader>
                                        <AlertDialogTitle>
                                            删除备份计划
                                        </AlertDialogTitle>
                                        <AlertDialogDescription>
                                            确定要删除备份计划 "{plan.title}"
                                            吗？此操作不可撤销。
                                        </AlertDialogDescription>
                                    </AlertDialogHeader>
                                    <AlertDialogFooter>
                                        <AlertDialogCancel>
                                            取消
                                        </AlertDialogCancel>
                                        <AlertDialogAction
                                            className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                                            onClick={() =>
                                                deletePlan(plan.plan_id)
                                            }
                                        >
                                            删除
                                        </AlertDialogAction>
                                    </AlertDialogFooter>
                                </AlertDialogContent>
                            </AlertDialog>
                        </div>
                    </div>
                </CardContent>
            </Card>
        );
    });
}

function PlanListDesktop({
    plans,
    services,
    uncompleteTasks,
    t,
    togglePlan,
    runPlan,
    deletePlan,
    onNavigate,
}: {
    plans: Array<BackupPlanInfo>;
    services: Array<BackupTargetInfo>;
    uncompleteTasks: TaskInfo[];
    t: Translations;
    togglePlan: (plan: BackupPlanInfo) => void;
    runPlan: (plan: BackupPlanInfo) => void;
    deletePlan: (planId: string) => void;
    onNavigate?: (page: string, data?: any) => void;
}) {
    return plans.map((plan) => {
        const policies = TaskMgrHelper.formatPlanPolicy(plan);
        const service = services.find((s) => s.target_id === plan.target);
        return (
            <Card
                key={plan.plan_id}
                className={`transition-all gap-2 ${
                    plan.policy_disabled || plan.policy.length === 0
                        ? "opacity-60"
                        : ""
                }`}
                onClick={() => onNavigate?.("plan-details", plan)}
            >
                <CardHeader className="pt-4 pb-0">
                    <div className="flex items-start justify-between">
                        <div className="flex-1">
                            <div className="flex items-center gap-3 mb-2">
                                <CardTitle className="text-lg">
                                    {plan.title}
                                </CardTitle>
                                {getStatusBadge(plan, uncompleteTasks)}
                            </div>
                            <CardDescription>
                                {plan.description}
                            </CardDescription>
                        </div>
                        <div className="flex items-center gap-3">
                            <Switch
                                disabled={plan.policy.length === 0}
                                checked={
                                    !(
                                        plan.policy_disabled ||
                                        plan.policy.length === 0
                                    )
                                }
                                onClick={(event) => event.stopPropagation()}
                                onCheckedChange={() => togglePlan(plan)}
                            />
                        </div>
                    </div>
                </CardHeader>
                <CardContent className="pt-0 pb-2 [&:last-child]:pb-4">
                    <div className="grid grid-cols-1 lg:grid-cols-4 gap-4 mb-4">
                        <div className="flex items-center gap-2">
                            <Folder className="w-4 h-4 text-muted-foreground" />
                            <div>
                                <p className="text-sm text-muted-foreground">
                                    {t.plans.source}
                                </p>
                                <p className="font-medium truncate">
                                    {plan.source}
                                </p>
                            </div>
                        </div>
                        <div className="flex items-center gap-2">
                            <Server className="w-4 h-4 text-muted-foreground" />
                            <div>
                                <p className="text-sm text-muted-foreground">
                                    {t.plans.destination}
                                </p>
                                <p className="font-medium">
                                    {service
                                        ? service.name || service.url
                                        : "未知服务"}
                                </p>
                            </div>
                        </div>
                        <div className="flex items-center gap-2">
                            <Calendar className="w-4 h-4 text-muted-foreground" />
                            <div>
                                <p className="text-sm text-muted-foreground">
                                    {t.plans.schedule}
                                </p>
                                <p className="font-medium">
                                    {policies.map((s, idx) => (
                                        <span key={idx}>
                                            {idx === policies.length - 1
                                                ? `${s}`
                                                : `${s}|`}
                                        </span>
                                    ))}
                                </p>
                            </div>
                        </div>
                        <div>
                            <p className="text-sm text-muted-foreground">
                                {t.plans.nextRun}
                            </p>
                            <p className="font-medium">
                                {TaskMgrHelper.formatTime(
                                    TaskMgrHelper.planNextRunTime(plan),
                                    "--"
                                )}
                            </p>
                        </div>
                    </div>

                    <div className="flex items-center justify-between pt-4 border-t">
                        <div className="text-muted-foreground text-sm">
                            {t.plans.lastRun}:{" "}
                            {TaskMgrHelper.formatTime(plan.last_run_time, "--")}
                        </div>
                        <div className="flex items-center gap-2">
                            <Button
                                variant="outline"
                                size="sm"
                                className="gap-1"
                                onClick={(event) => {
                                    event.stopPropagation();
                                    runPlan(plan);
                                }}
                            >
                                <Play className="w-3 h-3" />
                                {t.plans.runNow}
                            </Button>
                            <Button
                                variant="outline"
                                size="sm"
                                className="gap-1"
                                onClick={(event) => {
                                    event.stopPropagation();
                                    onNavigate?.("restore", {
                                        planId: plan.plan_id,
                                    });
                                }}
                            >
                                <Undo2 className="w-3 h-3" />
                                {t.common.restore}
                            </Button>
                            <Button
                                variant="outline"
                                size="sm"
                                className="gap-1"
                                onClick={(event) => {
                                    event.stopPropagation();
                                    onNavigate?.("edit-plan", plan);
                                }}
                            >
                                <Edit className="w-3 h-3" />
                                {t.common.edit}
                            </Button>
                            <AlertDialog>
                                <AlertDialogTrigger asChild>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        className="gap-1 text-destructive hover:text-destructive-foreground hover:bg-destructive"
                                        onClick={(event) =>
                                            event.stopPropagation()
                                        }
                                    >
                                        <Trash2 className="w-3 h-3" />
                                        {t.common.delete}
                                    </Button>
                                </AlertDialogTrigger>
                                <AlertDialogContent>
                                    <AlertDialogHeader>
                                        <AlertDialogTitle>
                                            删除备份计划
                                        </AlertDialogTitle>
                                        <AlertDialogDescription>
                                            确定要删除备份计划 "{plan.title}"
                                            吗？此操作不可撤销。
                                        </AlertDialogDescription>
                                    </AlertDialogHeader>
                                    <AlertDialogFooter>
                                        <AlertDialogCancel>
                                            取消
                                        </AlertDialogCancel>
                                        <AlertDialogAction
                                            className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                                            onClick={() =>
                                                deletePlan(plan.plan_id)
                                            }
                                        >
                                            删除
                                        </AlertDialogAction>
                                    </AlertDialogFooter>
                                </AlertDialogContent>
                            </AlertDialog>
                        </div>
                    </div>
                </CardContent>
            </Card>
        );
    });
}

function getStatusBadge(plan: BackupPlanInfo, uncompleteTasks: TaskInfo[]) {
    const status = TaskMgrHelper.planState(plan, uncompleteTasks);
    switch (status) {
        case PlanState.ACTIVE:
            return <Badge className="bg-green-100 text-green-800">正常</Badge>;
        case PlanState.WARNING:
            return (
                <Badge className="bg-yellow-100 text-yellow-800">警告</Badge>
            );
        case PlanState.DISABLED:
            return <Badge variant="secondary">已禁用</Badge>;
        case PlanState.ERROR:
            return <Badge className="bg-red-100 text-red-800">错误</Badge>;
        default:
            return <Badge variant="outline">未知</Badge>;
    }
}
