import React, { useEffect, useMemo, useRef, useState } from "react";
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from "./ui/card";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { ScrollArea } from "./ui/scroll-area";
import { Progress } from "./ui/progress";
import {
    Pagination,
    PaginationContent,
    PaginationEllipsis,
    PaginationItem,
    PaginationLink,
    PaginationNext,
    PaginationPrevious,
} from "./ui/pagination";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import {
    ArrowLeft,
    Calendar,
    Clock,
    Folder,
    Server,
    Play,
    Pause,
    CheckCircle,
    XCircle,
    AlertTriangle,
    FileText,
    Activity,
    Edit,
    Undo2,
} from "lucide-react";
import {
    BackupLog,
    BackupPlanInfo,
    BackupTargetInfo,
    ListOrder,
    ListTaskOrderBy,
    TaskInfo,
    TaskState,
} from "./utils/task_mgr";
import { PlanState, TaskMgrHelper } from "./utils/task_mgr_helper";
import { taskManager } from "./utils/fake_task_mgr";
import { LoadingPage } from "./LoadingPage";
import { Translations } from "./i18n";

interface PlanDetailsProps {
    onBack: () => void;
    onNavigate: (page: string, data?: any) => void;
    plan: BackupPlanInfo;
}

export function PlanDetails({ onBack, onNavigate, plan }: PlanDetailsProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [uncompleteTasks, setUncompleteTasks] = useState<TaskInfo[]>([]);

    useEffect(() => {
        taskManager
            .listBackupTasks({
                state: [
                    TaskState.FAILED,
                    TaskState.PAUSED,
                    TaskState.RUNNING,
                    TaskState.PENDING,
                ],
                owner_plan_id: [plan.plan_id],
            })
            .then(async ({ task_ids }) => {
                const uncompleteTasks = await Promise.all(
                    task_ids.map((id) => taskManager.getTaskInfo(id))
                );
                setUncompleteTasks(uncompleteTasks);
            });
    }, []);

    return (
        <div className={`${isMobile ? "p-4" : "p-6"} space-y-6`}>
            {/* 头部导航 */}
            <div className="flex items-center gap-4">
                <Button variant="ghost" size="sm" onClick={onBack}>
                    <ArrowLeft className="w-4 h-4" />
                    {!isMobile && <span className="ml-2">返回</span>}
                </Button>
                <div className="flex-1">
                    <div className="flex items-center gap-3">
                        <h1 className="text-xl font-semibold">{plan.title}</h1>
                        {getStatusBadge(plan, uncompleteTasks)}
                    </div>
                    <p className="text-sm text-muted-foreground">
                        {plan.description}
                    </p>
                </div>
                <div className="flex gap-2">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => onNavigate("edit-plan", plan)}
                        className="gap-2"
                    >
                        <Edit className="w-4 h-4" />
                        {!isMobile && "编辑"}
                    </Button>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() =>
                            onNavigate("restore", { planId: plan.plan_id })
                        }
                        className="gap-2"
                    >
                        <Undo2 className="w-4 h-4" />
                        {!isMobile && "恢复"}
                    </Button>
                </div>
            </div>

            {/* 内容标签页 */}
            <Tabs defaultValue="overview" className="space-y-6">
                <TabsList className="grid w-full grid-cols-3">
                    <TabsTrigger value="overview">概览</TabsTrigger>
                    <TabsTrigger value="tasks">任务历史</TabsTrigger>
                    <TabsTrigger value="logs">操作日志</TabsTrigger>
                </TabsList>

                {/* 概览 */}
                <TabsContent value="overview" className="space-y-6">
                    <OverviewTabContent
                        plan={plan}
                        t={t}
                        isMobile={isMobile}
                        uncompleteTasks={uncompleteTasks}
                    />
                </TabsContent>

                {/* 任务历史 */}
                <TabsContent value="tasks" className="space-y-4">
                    <TaskHistoryTabContent
                        plan={plan}
                        isMobile={isMobile}
                        t={t}
                        onNavigate={onNavigate}
                    />
                </TabsContent>

                {/* 操作日志 */}
                <TabsContent value="logs" className="space-y-4"></TabsContent>
            </Tabs>
        </div>
    );
}

function OverviewTabContent({
    plan,
    t,
    isMobile,
    uncompleteTasks,
}: {
    plan: BackupPlanInfo;
    t: Translations;
    isMobile: boolean;
    uncompleteTasks: TaskInfo[];
}) {
    const [service, setService] = useState<BackupTargetInfo | null>(null);
    const policies = TaskMgrHelper.formatPlanPolicy(plan);

    useEffect(() => {
        taskManager.getBackupTarget(plan.target).then((target) => {
            setService(target);
        });
    }, []);
    return (
        <>
            <div className="grid gap-6 md:grid-cols-2">
                {/* 基本信息 */}
                <Card>
                    <CardHeader>
                        <CardTitle className="text-base">基本信息</CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="space-y-3">
                            <div className="flex justify-between">
                                <span className="text-sm text-muted-foreground">
                                    计划名称:
                                </span>
                                <span className="text-sm font-medium">
                                    {plan.title}
                                </span>
                            </div>
                            <div className="flex justify-between">
                                <span className="text-sm text-muted-foreground">
                                    状态:
                                </span>
                                <span className="text-sm">
                                    {getStatusBadge(plan, uncompleteTasks)}
                                </span>
                            </div>
                            <div className="flex justify-between">
                                <span className="text-sm text-muted-foreground">
                                    启用状态:
                                </span>
                                <span className="text-sm font-medium">
                                    {plan.policy_disabled ? "已启用" : "已禁用"}
                                </span>
                            </div>
                            <div className="flex justify-between">
                                <span className="text-sm text-muted-foreground">
                                    创建时间:
                                </span>
                                <span className="text-sm font-medium">
                                    {TaskMgrHelper.formatTime(
                                        plan.create_time,
                                        "未知"
                                    )}
                                </span>
                            </div>
                            <div className="flex justify-between">
                                <span className="text-sm text-muted-foreground">
                                    最后修改:
                                </span>
                                <span className="text-sm font-medium">
                                    {TaskMgrHelper.formatTime(
                                        plan.update_time,
                                        "未知"
                                    )}
                                </span>
                            </div>
                        </div>
                    </CardContent>
                </Card>

                {/* 配置信息 */}
                <Card>
                    <CardHeader>
                        <CardTitle className="text-base">配置信息</CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <div className="space-y-3">
                            <div className="flex items-start gap-2">
                                <Folder className="w-4 h-4 mt-0.5 text-muted-foreground" />
                                <div className="flex-1">
                                    <p className="text-sm text-muted-foreground">
                                        备份源
                                    </p>
                                    <p className="text-sm font-medium">
                                        {plan.source}
                                    </p>
                                </div>
                            </div>
                            <div className="flex items-start gap-2">
                                <Server className="w-4 h-4 mt-0.5 text-muted-foreground" />
                                <div className="flex-1">
                                    <p className="text-sm text-muted-foreground">
                                        备份目标
                                    </p>
                                    <p className="text-sm font-medium">
                                        {service
                                            ? `${service.name} ${service.url}`
                                            : "加载中..."}
                                    </p>
                                </div>
                            </div>
                            <div className="flex items-start gap-2">
                                <Calendar className="w-4 h-4 mt-0.5 text-muted-foreground" />
                                <div className="flex-1">
                                    <p className="text-sm text-muted-foreground">
                                        执行计划
                                    </p>
                                    <p className="text-sm font-medium">
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
                            <div className="flex items-start gap-2">
                                <Clock className="w-4 h-4 mt-0.5 text-muted-foreground" />
                                <div className="flex-1">
                                    <p className="text-sm text-muted-foreground">
                                        下次执行
                                    </p>
                                    <p className="text-sm font-medium">
                                        {TaskMgrHelper.formatTime(
                                            TaskMgrHelper.planNextRunTime(plan),
                                            "--"
                                        )}
                                    </p>
                                </div>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* 统计信息 */}
            <div className="grid gap-4 md:grid-cols-4">
                <Card>
                    <CardContent className="p-4">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-blue-100 rounded-full">
                                <Activity className="w-4 h-4 text-blue-600" />
                            </div>
                            <div>
                                <p className="text-sm text-muted-foreground">
                                    总备份次数
                                </p>
                                <p className="text-lg font-semibold">
                                    {plan.total_backup}
                                </p>
                            </div>
                        </div>
                    </CardContent>
                </Card>
                {/* <Card>
                            <CardContent className="p-4">
                                <div className="flex items-center gap-3">
                                    <div className="p-2 bg-green-100 rounded-full">
                                        <CheckCircle className="w-4 h-4 text-green-600" />
                                    </div>
                                    <div>
                                        <p className="text-sm text-muted-foreground">
                                            成功次数
                                        </p>
                                        <p className="text-lg font-semibold">
                                            {planData.totalBackups - 1}
                                        </p>
                                    </div>
                                </div>
                            </CardContent>
                        </Card>
                        <Card>
                            <CardContent className="p-4">
                                <div className="flex items-center gap-3">
                                    <div className="p-2 bg-red-100 rounded-full">
                                        <XCircle className="w-4 h-4 text-red-600" />
                                    </div>
                                    <div>
                                        <p className="text-sm text-muted-foreground">
                                            失败次数
                                        </p>
                                        <p className="text-lg font-semibold">
                                            1
                                        </p>
                                    </div>
                                </div>
                            </CardContent>
                        </Card> */}
                <Card>
                    <CardContent className="p-4">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-purple-100 rounded-full">
                                <Server className="w-4 h-4 text-purple-600" />
                            </div>
                            <div>
                                <p className="text-sm text-muted-foreground">
                                    总数据量
                                </p>
                                <p className="text-lg font-semibold">
                                    {TaskMgrHelper.formatSize(plan.total_size)}
                                </p>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            </div>
        </>
    );
}

const TASK_HISTORY_PAGE_SIZE = 5;
type TaskHistoryPaginationItem = number | "ellipsis";

function getTaskHistoryPaginationRange(
    current: number,
    total: number
): TaskHistoryPaginationItem[] {
    if (total <= 0) {
        return [];
    }
    const delta = 1;
    const range: number[] = [];
    for (let i = 1; i <= total; i++) {
        if (
            i === 1 ||
            i === total ||
            (i >= current - delta && i <= current + delta)
        ) {
            range.push(i);
        }
    }

    const result: TaskHistoryPaginationItem[] = [];
    let previous: number | null = null;

    for (const pageNumber of range) {
        if (previous !== null) {
            if (pageNumber - previous === 2) {
                result.push(previous + 1);
            } else if (pageNumber - previous > 2) {
                result.push("ellipsis");
            }
        }
        result.push(pageNumber);
        previous = pageNumber;
    }

    return result;
}

function TaskHistoryTabContent({
    plan,
    isMobile,
    t,
    onNavigate,
}: {
    plan: BackupPlanInfo;
    isMobile: boolean;
    t: Translations;
    onNavigate: (page: string, data?: any) => void;
}) {
    const [taskHistory, setTaskHistory] = useState<TaskInfo[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [page, setPage] = useState(1);
    const [totalTasks, setTotalTasks] = useState(0);
    const lastPlanIdRef = useRef<string>(plan.plan_id);

    useEffect(() => {
        if (plan.plan_id !== lastPlanIdRef.current) {
            lastPlanIdRef.current = plan.plan_id;
            if (page !== 1) {
                setPage(1);
                return;
            }
        }

        let cancelled = false;
        setIsLoading(true);

        taskManager
            .listBackupTasks(
                { owner_plan_id: [plan.plan_id] },
                (page - 1) * TASK_HISTORY_PAGE_SIZE,
                TASK_HISTORY_PAGE_SIZE,
                [[ListTaskOrderBy.CREATE_TIME, ListOrder.DESC]]
            )
            .then(async ({ task_ids, total }) => {
                if (cancelled) {
                    return;
                }
                const detailedTasks =
                    task_ids.length > 0
                        ? await Promise.all(
                              task_ids.map((id) => taskManager.getTaskInfo(id))
                          )
                        : [];
                if (!cancelled) {
                    setTaskHistory(detailedTasks);
                    setTotalTasks(total);
                }
            })
            .catch((error) => {
                if (!cancelled) {
                    console.error("Error fetching task history:", error);
                    setTaskHistory([]);
                    setTotalTasks(0);
                }
            })
            .finally(() => {
                if (!cancelled) {
                    setIsLoading(false);
                }
            });

        return () => {
            cancelled = true;
        };
    }, [plan.plan_id, page]);

    useEffect(() => {
        const maxPage = Math.max(
            1,
            Math.ceil(totalTasks / TASK_HISTORY_PAGE_SIZE)
        );
        if (page > maxPage) {
            setPage(maxPage);
        }
    }, [totalTasks, page]);

    const totalPages =
        totalTasks > 0
            ? Math.ceil(totalTasks / TASK_HISTORY_PAGE_SIZE)
            : 1;
    const paginationItems = useMemo(
        () => getTaskHistoryPaginationRange(page, totalPages),
        [page, totalPages]
    );
    const visibleStart =
        !isLoading && taskHistory.length > 0
            ? (page - 1) * TASK_HISTORY_PAGE_SIZE + 1
            : 0;
    const visibleEnd =
        !isLoading && taskHistory.length > 0
            ? Math.min(totalTasks, visibleStart + taskHistory.length - 1)
            : 0;
    const showPagination =
        !isLoading && totalTasks > TASK_HISTORY_PAGE_SIZE;

    const getTaskStatusBadge = (status: TaskState) => {
        switch (status) {
            case TaskState.DONE:
                return (
                    <Badge className="bg-green-100 text-green-800 text-xs">
                        已完成
                    </Badge>
                );
            case TaskState.FAILED:
                return (
                    <Badge className="bg-red-100 text-red-800 text-xs">
                        失败
                    </Badge>
                );
            case TaskState.RUNNING:
                return (
                    <Badge className="bg-blue-100 text-blue-800 text-xs">
                        执行中
                    </Badge>
                );
            case TaskState.PAUSED:
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                        暂停
                    </Badge>
                );
            case TaskState.PENDING:
                return (
                    <Badge className="bg-gray-100 text-gray-800 text-xs">
                        排队中
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

    const getTaskStatusIcon = (status: TaskState) => {
        switch (status) {
            case TaskState.DONE:
                return <CheckCircle className="w-4 h-4 text-green-500" />;
            case TaskState.FAILED:
                return <XCircle className="w-4 h-4 text-red-500" />;
            case TaskState.RUNNING:
                return <Activity className="w-4 h-4 text-blue-500" />;
            case TaskState.PAUSED:
                return <Pause className="w-4 h-4 text-yellow-500" />;
            case TaskState.PENDING:
                return <Play className="w-4 h-4 text-gray-500" />;
            default:
                return <AlertTriangle className="w-4 h-4 text-yellow-500" />;
        }
    };

    return (
        <>
            <div className="flex items-center justify-between">
                <div>
                    <h3 className="text-lg font-medium">任务执行历史</h3>
                    <p className="text-sm text-muted-foreground">
                        按时间降序排列的备份任务
                    </p>
                </div>
            </div>
            <div className="space-y-3">
                {isLoading ? (
                    LoadingPage({})
                ) : taskHistory.length === 0 ? (
                    <div className="p-4 text-center text-muted-foreground">
                        暂无任务执行历史
                    </div>
                ) : (
                    taskHistory.map((task) => (
                        <Card key={task.taskid}>
                            <CardContent className="p-4">
                                <div className="flex items-start gap-4">
                                    {getTaskStatusIcon(task.state)}
                                    <div className="flex-1 space-y-2">
                                        <div className="flex items-center justify-between">
                                            <div className="flex items-center gap-2">
                                                <h4 className="font-medium">
                                                    {task.name}
                                                </h4>
                                                {getTaskStatusBadge(task.state)}
                                            </div>
                                            <div className="text-right">
                                                <p className="text-sm font-medium">
                                                    {TaskMgrHelper.formatSize(
                                                        task.total_size
                                                    )}
                                                </p>
                                            </div>
                                        </div>

                                        <div
                                            className={`grid ${
                                                isMobile
                                                    ? "grid-cols-1"
                                                    : "grid-cols-3"
                                            } gap-4 text-sm`}
                                        >
                                            <div>
                                                <p className="text-muted-foreground">
                                                    创建时间
                                                </p>
                                                <p className="font-medium">
                                                    {TaskMgrHelper.formatTime(
                                                        task.create_time,
                                                        "未知"
                                                    )}
                                                </p>
                                            </div>
                                            <div>
                                                <p className="text-muted-foreground">
                                                    文件/目录
                                                </p>
                                                <p className="font-medium">
                                                    {task.completed_item_count}{" "}
                                                    个
                                                </p>
                                            </div>
                                            <div>
                                                <p className="text-muted-foreground">
                                                    传输速度
                                                </p>
                                                <p className="font-medium">
                                                    {`$${TaskMgrHelper.formatSize(
                                                        task.speed
                                                    )}/s`}
                                                </p>
                                            </div>
                                        </div>

                                        {task.error && (
                                            <div className="p-3 bg-red-50 text-red-700 rounded-md text-sm">
                                                错误: {task.error}
                                            </div>
                                        )}

                                        {task.state === TaskState.DONE && (
                                            <div className="flex justify-end">
                                                <Button
                                                    variant="outline"
                                                    size="sm"
                                                    onClick={() =>
                                                        onNavigate("restore", {
                                                            taskId: task.taskid,
                                                        })
                                                    }
                                                    className="gap-2"
                                                >
                                                    <Undo2 className="w-3 h-3" />
                                                    恢复
                                                </Button>
                                            </div>
                                        )}
                                    </div>
                                </div>
                            </CardContent>
                        </Card>
                    ))
                )}
            </div>
            {showPagination && (
                <div className="flex flex-wrap items-center justify-between gap-2 pt-2">
                    <div className="text-sm text-muted-foreground">
                        {`${visibleStart}-${visibleEnd} / ${totalTasks}`}
                    </div>
                    <Pagination className="mx-0 justify-center sm:justify-end">
                        <PaginationContent>
                            <PaginationItem>
                                <PaginationPrevious
                                    href="#"
                                    aria-disabled={page === 1}
                                    className={
                                        page === 1
                                            ? "pointer-events-none opacity-50"
                                            : ""
                                    }
                                    onClick={(event) => {
                                        event.preventDefault();
                                        if (page > 1) {
                                            setPage(page - 1);
                                        }
                                    }}
                                />
                            </PaginationItem>
                            {paginationItems.map((item, index) =>
                                item === "ellipsis" ? (
                                    <PaginationItem
                                        key={`ellipsis-${index}`}
                                    >
                                        <PaginationEllipsis />
                                    </PaginationItem>
                                ) : (
                                    <PaginationItem key={`page-${item}`}>
                                        <PaginationLink
                                            href="#"
                                            isActive={item === page}
                                            onClick={(event) => {
                                                event.preventDefault();
                                                setPage(item);
                                            }}
                                        >
                                            {item}
                                        </PaginationLink>
                                    </PaginationItem>
                                )
                            )}
                            <PaginationItem>
                                <PaginationNext
                                    href="#"
                                    aria-disabled={page === totalPages}
                                    className={
                                        page === totalPages
                                            ? "pointer-events-none opacity-50"
                                            : ""
                                    }
                                    onClick={(event) => {
                                        event.preventDefault();
                                        if (page < totalPages) {
                                            setPage(page + 1);
                                        }
                                    }}
                                />
                            </PaginationItem>
                        </PaginationContent>
                    </Pagination>
                </div>
            )}
        </>
    );
}

function planLogsTabContent(
    plan: BackupPlanInfo,
    isMobile: boolean,
    t: Translations
) {
    const [logs, setLogs] = useState<{
        logs: BackupLog[];
        total: number;
    } | null>(null);

    useEffect(() => {
        taskManager
            .listLogs(0, 0, ListOrder.DESC, { plan_id: plan.plan_id })
            .then((logs) => {
                setLogs(logs);
            })
            .catch((error) => {
                console.error("Error fetching plan logs:", error);
            });
    }, [plan.plan_id]);

    const getLogTypeIcon = (type: string) => {
        switch (type) {
            case "task_start":
                return <Play className="w-4 h-4 text-blue-500" />;
            case "task_complete":
                return <CheckCircle className="w-4 h-4 text-green-500" />;
            case "task_fail":
                return <XCircle className="w-4 h-4 text-red-500" />;
            case "plan_update":
                return <Edit className="w-4 h-4 text-orange-500" />;
            default:
                return <FileText className="w-4 h-4 text-muted-foreground" />;
        }
    };

    return (
        <>
            <div className="flex items-center justify-between">
                <div>
                    <h3 className="text-lg font-medium">日志</h3>
                    <p className="text-sm text-muted-foreground">
                        计划相关的所有记录
                    </p>
                </div>
            </div>
            {logs ? (
                logs.logs.length === 0 ? (
                    <div className="p-4 text-center text-muted-foreground">
                        暂无日志
                    </div>
                ) : (
                    <Card>
                        <CardContent className="p-0">
                            <div className="space-y-0">
                                {logs.logs.map((log, index) => (
                                    <div
                                        key={log.seq}
                                        className={`p-4 ${
                                            index < logs.logs.length - 1
                                                ? "border-b"
                                                : ""
                                        }`}
                                    >
                                        <div className="flex items-start gap-3">
                                            {getLogTypeIcon(log.type)}
                                            <div className="flex-1">
                                                <div className="flex items-center justify-between mb-1">
                                                    <p className="font-medium text-sm">
                                                        {TaskMgrHelper.formatLog(
                                                            log
                                                        )}
                                                    </p>
                                                    <p className="text-xs text-muted-foreground">
                                                        {TaskMgrHelper.formatTime(
                                                            log.timestamp
                                                        )}
                                                    </p>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </CardContent>
                    </Card>
                )
            ) : (
                LoadingPage({})
            )}
        </>
    );
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
