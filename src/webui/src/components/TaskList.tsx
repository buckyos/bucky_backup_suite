import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from "./ui/card";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { Progress } from "./ui/progress";
import { Input } from "./ui/input";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "./ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { Checkbox } from "./ui/checkbox";
import {
    Sheet,
    SheetContent,
    SheetTrigger,
    SheetHeader,
    SheetTitle,
} from "./ui/sheet";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import {
    Pagination,
    PaginationContent,
    PaginationEllipsis,
    PaginationItem,
    PaginationLink,
    PaginationNext,
    PaginationPrevious,
} from "./ui/pagination";
import { TaskDetail } from "./TaskDetail";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { LoadingPage } from "./LoadingPage";
import {
    Search,
    Filter,
    Pause,
    Play,
    Square,
    Trash2,
    Eye,
    Calendar,
    Clock,
    FileText,
    MoreVertical,
    ChevronLeft,
    Undo2,
} from "lucide-react";
import { Translations } from "./i18n";
import {
    BackupPlanInfo,
    TaskEventType,
    TaskFilter,
    TaskInfo,
    TaskState,
    TaskType,
} from "./utils/task_mgr";
import { PlanState, TaskMgrHelper, taskManager } from "./utils/task_mgr_helper";

const HISTORY_PAGE_SIZE = 10;

type PaginationItemType = number | "ellipsis";

function getPaginationRange(
    current: number,
    total: number
): PaginationItemType[] {
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

    const result: PaginationItemType[] = [];
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

interface TaskListProps {
    onNavigate: (page: string, data?: any) => void;
}

function getStatusBadge(status: TaskState, t: Translations) {
    switch (status) {
        case TaskState.RUNNING:
            return (
                <Badge className="bg-blue-100 text-blue-800 text-xs">
                    {t.tasks.running}
                </Badge>
            );
        case TaskState.DONE:
            return (
                <Badge className="bg-green-100 text-green-800 text-xs">
                    {t.tasks.completed}
                </Badge>
            );
        case TaskState.PAUSED:
            return (
                <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                    {t.tasks.paused}
                </Badge>
            );
        case TaskState.FAILED:
            return (
                <Badge className="bg-red-100 text-red-800 text-xs">
                    {t.tasks.failed}
                </Badge>
            );
        case TaskState.PAUSING:
            return (
                <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                    {t.tasks.paused}
                </Badge>
            );
        case TaskState.PENDING:
            return (
                <Badge className="bg-gray-100 text-gray-800 text-xs">
                    {t.tasks.queued}
                </Badge>
            );
        default:
            return (
                <Badge variant="outline" className="text-xs">
                    未知
                </Badge>
            );
    }
}

function getTypeBadge(type: TaskType, t: Translations) {
    return type === TaskType.BACKUP ? (
        <Badge variant="outline" className="text-xs">
            {t.tasks.backup}
        </Badge>
    ) : (
        <Badge
            variant="outline"
            className="border-purple-200 text-purple-700 text-xs"
        >
            {t.tasks.restore}
        </Badge>
    );
}

function formatTaskCountLabel(
    template: string,
    visible: number,
    total: number
) {
    return template
        .replace("{visible}", visible.toString())
        .replace("{total}", total.toString());
}

enum TaskAction {
    PAUSE,
    REMOVE,
    RESUME,
    DETAIL,
}

function getTaskActions(
    task: TaskInfo,
    onNavigate: (page: string, data?: any) => void,
    t: Translations
) {
    const actions = [];

    if (task.state === TaskState.RUNNING || task.state === TaskState.PENDING) {
        actions.push(
            <Button
                key="pause"
                variant="outline"
                size="sm"
                className="gap-1"
                onClick={() => taskManager.pauseWorkTask(task.taskid)}
            >
                <Pause className="w-3 h-3" />
                暂停
            </Button>
        );
    } else {
        if (task.state !== TaskState.PAUSING) {
            actions.push(
                <Button
                    key="delete"
                    variant="outline"
                    size="sm"
                    className="gap-1 text-destructive hover:text-destructive-foreground hover:bg-destructive"
                    onClick={() => taskManager.removeBackupTask(task.taskid)}
                >
                    <Trash2 className="w-3 h-3" />
                    删除
                </Button>
            );
        }

        if (
            task.state === TaskState.PAUSED ||
            task.state === TaskState.FAILED
        ) {
            actions.push(
                <Button
                    key="resume"
                    variant="outline"
                    size="sm"
                    className="gap-1"
                    onClick={() => taskManager.resumeWorkTask(task.taskid)}
                >
                    <Play className="w-3 h-3" />
                    继续
                </Button>
            );
        } else if (task.state === TaskState.DONE) {
            if (task.task_type === TaskType.BACKUP) {
                actions.push(
                    <Button
                        variant="outline"
                        size="sm"
                        className="gap-1"
                        onClick={() =>
                            onNavigate?.("restore", {
                                planId: task.owner_plan_id,
                                taskId: task.taskid,
                            })
                        }
                    >
                        <Undo2 className="w-3 h-3" />
                        {t.common.restore}
                    </Button>
                );
            }
        }
    }

    return actions;
}

export function TaskList({ onNavigate }: TaskListProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [runningTaskCount, setRunningTaskCount] = useState(0);
    const [filterTaskCount, setFilterTaskCount] = useState(0);
    const [allHistoryTaskCount, setAllHistoryTaskCount] = useState(0);
    const [showDetailTask, setShowDetailTask] = useState<TaskInfo | null>(null);
    const [plans, setPlans] = useState<BackupPlanInfo[]>([]);

    const refreshAllHistoryTaskCount = async () => {
        const { total } = await taskManager.listBackupTasks({
            state: [TaskState.DONE],
        });
        setAllHistoryTaskCount(total);
    };

    useEffect(() => {
        refreshAllHistoryTaskCount();
        const taskEventHandler = async (event: TaskEventType, data: any) => {
            console.log("task event:", event, data);
            switch (event) {
                case TaskEventType.COMPLETE_TASK:
                case TaskEventType.REMOVE_TASK:
                    await refreshAllHistoryTaskCount();
                    break;
            }
        };

        taskManager.addTaskEventListener(taskEventHandler);

        const timerId = taskManager.startRefreshUncompleteTaskStateTimer();
        return () => {
            taskManager.stopRefreshUncompleteTaskStateTimer(timerId);
            taskManager.removeTaskEventListener(taskEventHandler);
        };
    }, []);

    return (
        <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-4`}>
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="mb-2">{t.tasks.title}</h1>
                    <p className="text-muted-foreground">{t.tasks.subtitle}</p>
                </div>
            </div>

            {/* 任务列表 */}
            <Tabs defaultValue="running" className="space-y-4">
                <div className="flex items-center justify-between">
                    <TabsList>
                        <TabsTrigger value="running">
                            {t.tasks.runningTasks} ({runningTaskCount})
                        </TabsTrigger>
                        <TabsTrigger value="history">
                            {"历史任务"} (
                            {!allHistoryTaskCount
                                ? ""
                                : filterTaskCount
                                ? `${filterTaskCount}/${allHistoryTaskCount}`
                                : allHistoryTaskCount}
                            {""})
                        </TabsTrigger>
                    </TabsList>
                </div>

                <TabsContent value="history" className="space-y-4">
                    <HistoryTaskTabContent
                        t={t}
                        isMobile={isMobile}
                        setFilterTaskCount={setFilterTaskCount}
                        showDetailTask={setShowDetailTask}
                        plans={plans}
                        setPlans={setPlans}
                        onNavigate={onNavigate}
                    />
                </TabsContent>

                <TabsContent value="running" className="space-y-4">
                    <RunningTaskTabContent
                        t={t}
                        isMobile={isMobile}
                        setTaskCount={setRunningTaskCount}
                        showDetailTask={setShowDetailTask}
                        plans={plans}
                        setPlans={setPlans}
                        onNavigate={onNavigate}
                    />
                </TabsContent>
            </Tabs>

            {/* 任务详情对话框 */}
            {showDetailTask && (
                <div className="fixed inset-0 bg-background z-50">
                    <TaskDetail
                        task={showDetailTask}
                        onBack={() => setShowDetailTask(null)}
                    />
                </div>
            )}
        </div>
    );
}

function Loading({ isMobile, t }: { isMobile?: boolean; t: Translations }) {
    return (
        <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-4`}>
            <div>
                <h1 className="mb-2">{t.tasks.title}</h1>
                <p className="text-muted-foreground">{t.tasks.subtitle}</p>
            </div>
            <LoadingPage status={`${t.common.loading} ${t.nav.tasks}...`} />
        </div>
    );
}

function refreshFilterTasks(
    filter: TaskFilter,
    {
        plans,
        setPlans,
        setFilterTasks,
        setServiceCount,
        setAllTaskCount,
        offset = 0,
        limit,
    }: {
        plans: BackupPlanInfo[];
        setPlans: (plans: BackupPlanInfo[]) => void;
        setFilterTasks: (tasks: TaskInfo[]) => void;
        setServiceCount: (count: number) => void;
        setAllTaskCount: (count: number) => void;
        offset?: number;
        limit?: number;
    }
) {
    const refreshAllPlans = async () => {
        const newPlanIds = await taskManager.listBackupPlans();
        const newPlans = await Promise.all(
            newPlanIds.map((planId) => taskManager.getBackupPlan(planId))
        );
        plans.splice(0, plans.length);
        newPlans.forEach((p) => plans.push(p));
        setPlans(plans);
    };

    const refreshAllServices = async () => {
        const serviceIds = await taskManager.listBackupTargets();
        setServiceCount(serviceIds.length);
    };

    taskManager
        .listBackupTasks(filter, offset, limit)
        .then(async ({ task_ids, total }) => {
            console.log("Filtered tasks:", task_ids, total, filter);
            const taskInfos = await Promise.all(
                task_ids.map((taskid) => taskManager.getTaskInfo(taskid))
            );
            for (const task of taskInfos) {
                if (!plans.find((p) => task.owner_plan_id === p.plan_id)) {
                    await refreshAllPlans();
                }
            }
            console.log("tasks: ", taskInfos);
            setFilterTasks(taskInfos);
            setAllTaskCount(total);

            if (total === 0) {
                await refreshAllPlans();
                await refreshAllServices();
            }
        });
}

function CreateFirstTaskGuide({
    isMobile,
    plansCount,
    servicesCount,
    onNavigate,
}: {
    isMobile: boolean;
    plansCount: number;
    servicesCount: number;
    onNavigate: (page: string, data?: any) => void;
}) {
    return (
        <Card
            className={`w-full ${
                isMobile ? "" : "max-w-2xl mx-auto"
            } text-center`}
        >
            <CardHeader>
                <CardTitle>暂无任务</CardTitle>
                <CardDescription>
                    {servicesCount === 0
                        ? "开始前，请先配置一个备份服务"
                        : plansCount === 0
                        ? "配置好服务后，创建你的第一个备份计划"
                        : "你可以立即执行一次备份任务"}
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
                    {servicesCount === 0 ? (
                        <>
                            <Button
                                onClick={() => onNavigate?.("add-service")}
                                className="gap-2"
                            >
                                去配置备份服务
                            </Button>
                            {!isMobile && (
                                <span className="text-muted-foreground">
                                    或
                                </span>
                            )}
                            <Button
                                variant="outline"
                                disabled
                                className="gap-2"
                            >
                                新建备份计划
                            </Button>
                        </>
                    ) : plansCount === 0 ? (
                        <Button
                            onClick={() => onNavigate?.("create-plan")}
                            className="gap-2"
                        >
                            新建备份计划
                        </Button>
                    ) : (
                        <Button
                            onClick={() => onNavigate?.("plans")}
                            className="gap-2"
                        >
                            前往计划列表执行一次备份
                        </Button>
                    )}
                </div>
            </CardContent>
        </Card>
    );
}

function RunningTaskTabContent({
    isMobile,
    t,
    setTaskCount,
    showDetailTask,
    plans,
    setPlans,
    onNavigate,
}: {
    isMobile: boolean;
    t: Translations;
    setTaskCount: (count: number) => void;
    showDetailTask: (task: TaskInfo | null) => void;
    plans: BackupPlanInfo[];
    setPlans: (plans: BackupPlanInfo[]) => void;
    onNavigate: (page: string, data?: any) => void;
}) {
    const [uncompleteTasks, setUncompleteTasks] = useState<TaskInfo[] | null>(
        null
    );
    const [serviceCount, setServiceCount] = useState(0);
    const [allTaskCount, setAllTaskCountInner] = useState(0);

    const refreshUncompleteTasks = () => {
        refreshFilterTasks(
            {
                state: [
                    TaskState.RUNNING,
                    TaskState.PENDING,
                    TaskState.PAUSED,
                    TaskState.FAILED,
                ],
            },
            {
                plans,
                setPlans,
                setFilterTasks: (tasks: TaskInfo[]) => {
                    setUncompleteTasks(tasks);
                    setTaskCount(tasks.length);
                },
                setServiceCount,
                setAllTaskCount: setAllTaskCountInner,
            }
        );
    };

    const taskEventHandler = async (event: TaskEventType, data: any) => {
        console.log("task event:", event, data);
        switch (event) {
            case TaskEventType.CREATE_TASK:
            case TaskEventType.FAIL_TASK:
            case TaskEventType.PAUSE_TASK:
            case TaskEventType.RESUME_TASK:
            case TaskEventType.UPDATE_TASK:
            case TaskEventType.COMPLETE_TASK:
            case TaskEventType.REMOVE_TASK:
            case TaskEventType.CREATE_TARGET:
                refreshUncompleteTasks();
                break;
        }
    };

    useEffect(() => {
        refreshUncompleteTasks();
        taskManager.addTaskEventListener(taskEventHandler);
        return () => {
            taskManager.removeTaskEventListener(taskEventHandler);
        };
    }, []);

    if (uncompleteTasks === null) {
        return <Loading isMobile={isMobile} t={t} />;
    }

    if (allTaskCount === 0) {
        return (
            <CreateFirstTaskGuide
                plansCount={plans.length}
                servicesCount={serviceCount}
                onNavigate={onNavigate}
                isMobile={isMobile}
            />
        );
    }

    return (
        <>
            {uncompleteTasks!.map((task) => {
                const taskProgress = TaskMgrHelper.taskProgress(task);
                const taskRemainStr = TaskMgrHelper.taskRemainingStr(task);
                console.log("TaskList: plans: ", plans, "task:", task);
                return (
                    <Card
                        key={task.taskid}
                        className="cursor-pointer hover:bg-accent/50 gap-1"
                        onClick={() => showDetailTask(task)}
                    >
                        <CardHeader className="pt-4 pb-0">
                            <div className="flex items-center justify-between">
                                <div className="flex-1 min-w-0">
                                    <CardTitle
                                        className={`${
                                            isMobile ? "text-base" : "text-lg"
                                        } truncate`}
                                    >
                                        {task.name}
                                    </CardTitle>
                                    <CardDescription className="text-sm">
                                        {
                                            plans.find(
                                                (p) =>
                                                    p.plan_id ===
                                                    task.owner_plan_id
                                            )!.title
                                        }
                                    </CardDescription>
                                </div>
                                <div className="flex items-center gap-2 flex-shrink-0">
                                    {getStatusBadge(task.state, t)}
                                    {getTypeBadge(task.task_type, t)}
                                    {isMobile && (
                                        <DropdownMenu>
                                            <DropdownMenuTrigger asChild>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    className="h-6 w-6 p-0"
                                                    onClick={(e) =>
                                                        e.stopPropagation()
                                                    }
                                                >
                                                    <MoreVertical className="w-4 h-4" />
                                                </Button>
                                            </DropdownMenuTrigger>
                                            <DropdownMenuContent align="end">
                                                <DropdownMenuItem
                                                    onClick={(e) =>
                                                        e.stopPropagation()
                                                    }
                                                >
                                                    <Pause className="w-4 h-4 mr-2" />
                                                    {t.tasks.pause}
                                                </DropdownMenuItem>
                                                <DropdownMenuItem
                                                    onClick={(e) =>
                                                        e.stopPropagation()
                                                    }
                                                    className="text-destructive"
                                                >
                                                    <Square className="w-4 h-4 mr-2" />
                                                    {t.tasks.stop}
                                                </DropdownMenuItem>
                                            </DropdownMenuContent>
                                        </DropdownMenu>
                                    )}
                                </div>
                            </div>
                        </CardHeader>
                        <CardContent className="pt-0 pb-2 [&:last-child]:pb-4">
                            <div className="space-y-3">
                                <div>
                                    <div className="flex justify-between text-sm mb-2">
                                        <span>
                                            {t.tasks.progress}: {taskProgress}%
                                        </span>
                                        <span>
                                            {task.state === TaskState.RUNNING
                                                ? TaskMgrHelper.taskSpeedStr(
                                                      task
                                                  )
                                                : "--"}
                                        </span>
                                    </div>
                                    <Progress
                                        value={taskProgress}
                                        className="h-2"
                                    />
                                    <div className="flex justify-between text-sm text-muted-foreground mt-1">
                                        <span>
                                            {TaskMgrHelper.formatSize(
                                                task.completed_size
                                            )}
                                        </span>
                                        <span>{taskRemainStr}</span>
                                    </div>
                                </div>
                                <div className="flex items-center justify-between pt-2 border-t">
                                    <div className="text-xs text-muted-foreground">
                                        开始时间:{" "}
                                        {new Date(
                                            task.create_time
                                        ).toLocaleString()}
                                    </div>
                                    {!isMobile && (
                                        <div
                                            className="flex items-center gap-2"
                                            onClick={(e) => e.stopPropagation()}
                                        >
                                            {getTaskActions(
                                                task,
                                                onNavigate,
                                                t
                                            )}
                                        </div>
                                    )}
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                );
            })}
            {uncompleteTasks.length === 0 && (
                <Card>
                    <CardContent className="py-12 text-center">
                        <p className="text-muted-foreground">
                            当前没有执行中的任务
                        </p>
                    </CardContent>
                </Card>
            )}
        </>
    );
}

function HistoryTaskTabContent({
    isMobile,
    t,
    setFilterTaskCount,
    showDetailTask,
    plans,
    setPlans,
    onNavigate,
}: {
    isMobile?: boolean;
    t: Translations;
    setFilterTaskCount: (count: number) => void;
    showDetailTask: (task: TaskInfo | null) => void;
    plans: BackupPlanInfo[];
    setPlans: (plans: BackupPlanInfo[]) => void;
    onNavigate: (page: string, data?: any) => void;
}) {
    // Ensure hooks order is stable on every render
    const [searchPlanFilter, setSearchPlanFilter] = useState("");
    const [typeFilter, setTypeFilter] = useState<TaskType | null>(null);
    const [filterTasks, setFilterTasks] = useState<TaskInfo[] | null>(null);
    const [serviceCount, setServiceCount] = useState(0);
    const [allTaskCount, setAllTaskCount] = useState(0);
    const [page, setPage] = useState(1);

    const fetchHistoryTasks = useCallback(() => {
        refreshFilterTasks(
            {
                owner_plan_title: searchPlanFilter
                    ? [searchPlanFilter]
                    : undefined,
                type: typeFilter ? [typeFilter] : undefined,
                state: [TaskState.DONE],
            },
            {
                plans,
                setPlans,
                setFilterTasks: (tasks: TaskInfo[]) => {
                    setFilterTasks(tasks);
                    setFilterTaskCount(tasks.length);
                },
                setServiceCount,
                setAllTaskCount,
                offset: (page - 1) * HISTORY_PAGE_SIZE,
                limit: HISTORY_PAGE_SIZE,
            }
        );
    }, [
        page,
        plans,
        searchPlanFilter,
        setFilterTaskCount,
        setPlans,
        typeFilter,
    ]);

    useEffect(() => {
        fetchHistoryTasks();

        const taskEventHandler = async (event: TaskEventType, data: any) => {
            console.log("task event:", event, data);
            switch (event) {
                case TaskEventType.COMPLETE_TASK:
                case TaskEventType.REMOVE_TASK:
                case TaskEventType.CREATE_TARGET:
                    fetchHistoryTasks();
                    break;
            }
        };

        taskManager.addTaskEventListener(taskEventHandler);
        return () => {
            taskManager.removeTaskEventListener(taskEventHandler);
        };
    }, [fetchHistoryTasks]);

    useEffect(() => {
        if (allTaskCount === 0) {
            if (page !== 1) {
                setPage(1);
            }
            return;
        }

        const maxPage = Math.max(
            1,
            Math.ceil(allTaskCount / HISTORY_PAGE_SIZE)
        );

        if (page > maxPage) {
            setPage(maxPage);
        }
    }, [allTaskCount, page]);

    const totalPages =
        allTaskCount > 0 ? Math.ceil(allTaskCount / HISTORY_PAGE_SIZE) : 0;
    const paginationItems = useMemo(
        () => getPaginationRange(page, totalPages),
        [page, totalPages]
    );

    if (filterTasks === null) {
        return <Loading isMobile={isMobile} t={t} />;
    }

    if (allTaskCount === 0) {
        return (
            <CreateFirstTaskGuide
                plansCount={plans.length}
                servicesCount={serviceCount}
                isMobile={false}
                onNavigate={onNavigate}
            />
        );
    }

    const showPagination = totalPages > 1 && filterTasks.length > 0;
    const visibleStart =
        filterTasks.length > 0 ? (page - 1) * HISTORY_PAGE_SIZE + 1 : 0;
    const visibleEnd =
        filterTasks.length > 0
            ? Math.min(allTaskCount, visibleStart + filterTasks.length - 1)
            : 0;

    return (
        <>
            <Card>
                <CardHeader>
                    <CardTitle className="text-lg">筛选和搜索</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="flex flex-wrap items-end gap-4">
                        <div className="flex-1 min-w-[240px] space-y-2">
                            <label className="text-sm font-medium">
                                {t.common.search}
                            </label>
                            <div className="relative">
                                <Search className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                <Input
                                    placeholder="搜索计划名称..."
                                    value={searchPlanFilter}
                                    onChange={(e) => {
                                        setPage(1);
                                        setSearchPlanFilter(e.target.value);
                                    }}
                                    className="pl-10"
                                />
                            </div>
                        </div>
                        <div className="flex-none min-w-[200px] space-y-2">
                            <label className="text-sm font-medium">
                                {t.common.type}
                            </label>
                            <Select
                                value={typeFilter ?? "all"}
                                onValueChange={(taskType: TaskType | "all") => {
                                    setPage(1);
                                    if (taskType === "all") {
                                        setTypeFilter(null);
                                    } else {
                                        setTypeFilter(taskType);
                                    }
                                }}
                            >
                                <SelectTrigger className="w-full">
                                    <SelectValue placeholder="全部类型" />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="all">
                                        全部类型
                                    </SelectItem>
                                    <SelectItem value={TaskType.BACKUP}>
                                        {t.tasks.backup}任务
                                    </SelectItem>
                                    <SelectItem value={TaskType.RESTORE}>
                                        {t.tasks.restore}任务
                                    </SelectItem>
                                </SelectContent>
                            </Select>
                        </div>
                    </div>
                </CardContent>
            </Card>
            {!isMobile && (
                <Card>
                    <CardContent className="py-3">
                        <div className="flex items-center gap-4">
                            <div className="grid grid-cols-5 gap-4 flex-1 items-center">
                                <div className="font-medium">任务名称</div>
                                <div className="font-medium">
                                    {t.common.type}
                                </div>
                                {/* <div className="font-medium">
                                        {t.common.status}
                                    </div> */}
                                <div className="font-medium">大小</div>
                                <div className="font-medium">时间</div>
                                <div className="font-medium text-right">
                                    {t.common.actions}
                                </div>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            )}

            {filterTasks.length === 0 ? (
                <Card>
                    <CardContent className="py-12 text-center">
                        <p className="text-muted-foreground">
                            没有找到匹配的任务
                        </p>
                    </CardContent>
                </Card>
            ) : (
                <>
                    {/* 任务列表 */}
                    {filterTasks.map((task) => {
                        return (
                            <Card
                                key={task.taskid}
                                className="cursor-pointer hover:bg-accent/50"
                                onClick={() => showDetailTask(task)}
                            >
                                <CardContent
                                    className={`${isMobile ? "py-3" : "py-4"}`}
                                >
                                    {isMobile ? (
                                        // 移动端紧凑布局
                                        <div className="space-y-2">
                                            <div className="flex items-center justify-between">
                                                <div className="flex items-center gap-2 flex-1 min-w-0">
                                                    <span className="font-medium text-sm truncate">
                                                        {task.name}
                                                    </span>
                                                    {getTypeBadge(
                                                        task.task_type,
                                                        t
                                                    )}
                                                </div>
                                                <DropdownMenu>
                                                    <DropdownMenuTrigger
                                                        asChild
                                                    >
                                                        <Button
                                                            variant="ghost"
                                                            size="sm"
                                                            className="h-6 w-6 p-0"
                                                            onClick={(e) =>
                                                                e.stopPropagation()
                                                            }
                                                        >
                                                            <MoreVertical className="w-4 h-4" />
                                                        </Button>
                                                    </DropdownMenuTrigger>
                                                    <DropdownMenuContent align="end">
                                                        {/* {(task.state ===
                                                            TaskState.RUNNING ||
                                                            task.state ===
                                                                TaskState.PENDING) && (
                                                            <DropdownMenuItem
                                                                onClick={(e) =>
                                                                    e.stopPropagation()
                                                                }
                                                            >
                                                                <Pause className="w-4 h-4 mr-2" />
                                                                {t.tasks.pause}
                                                            </DropdownMenuItem>
                                                        )}
                                                        {task.state ===
                                                            TaskState.PAUSED && (
                                                            <DropdownMenuItem
                                                                onClick={(e) =>
                                                                    e.stopPropagation()
                                                                }
                                                            >
                                                                <Play className="w-4 h-4 mr-2" />
                                                                {t.tasks.resume}
                                                            </DropdownMenuItem>
                                                        )} */}
                                                        {task.state ===
                                                            TaskState.DONE &&
                                                            task.task_type ===
                                                                TaskType.BACKUP && (
                                                                <DropdownMenuItem
                                                                    onClick={(
                                                                        e
                                                                    ) => {
                                                                        e.stopPropagation();
                                                                        onNavigate?.(
                                                                            "restore",
                                                                            {
                                                                                taskId: task.taskid,
                                                                            }
                                                                        );
                                                                    }}
                                                                >
                                                                    <FileText className="w-4 h-4 mr-2" />
                                                                    恢复
                                                                </DropdownMenuItem>
                                                            )}
                                                        {task.state !==
                                                            TaskState.RUNNING && (
                                                            <DropdownMenuItem
                                                                onClick={(e) =>
                                                                    e.stopPropagation()
                                                                }
                                                                className="text-destructive"
                                                            >
                                                                <Trash2 className="w-4 h-4 mr-2" />
                                                                {
                                                                    t.common
                                                                        .delete
                                                                }
                                                            </DropdownMenuItem>
                                                        )}
                                                    </DropdownMenuContent>
                                                </DropdownMenu>
                                            </div>

                                            {/* {task.state !== TaskState.DONE && (
                                                <div className="space-y-1">
                                                    <Progress
                                                        value={taskProgress}
                                                        className="h-1.5"
                                                    />
                                                    <div className="flex justify-between text-xs text-muted-foreground">
                                                        <span>
                                                            {taskProgress}%
                                                            {task.speed &&
                                                                ` • ${task.speed}`}
                                                        </span>
                                                        <span>
                                                            {taskRemainStr &&
                                                            taskRemainStr != "0"
                                                                ? taskRemainStr
                                                                : `${taskProgress}%`}
                                                        </span>
                                                    </div>
                                                </div>
                                            )} */}

                                            <div className="flex justify-between text-xs text-muted-foreground">
                                                <span>
                                                    {
                                                        plans.find(
                                                            (p) =>
                                                                p.plan_id ===
                                                                task.owner_plan_id
                                                        )?.title
                                                    }
                                                </span>
                                                <span>
                                                    {TaskMgrHelper.formatTime(
                                                        task.create_time ||
                                                            task.update_time
                                                    )}
                                                </span>
                                            </div>
                                        </div>
                                    ) : (
                                        // 桌面端详细布局
                                        <div className="flex items-center gap-4">
                                            <div className="grid grid-cols-5 gap-4 flex-1 items-center">
                                                <div>
                                                    <p className="font-medium">
                                                        {task.name}
                                                    </p>
                                                    <p className="text-sm text-muted-foreground">
                                                        {plans.find(
                                                            (p) =>
                                                                p.plan_id ===
                                                                task.owner_plan_id
                                                        )?.title || "-"}
                                                    </p>
                                                </div>
                                                <div>
                                                    {getTypeBadge(
                                                        task.task_type,
                                                        t
                                                    )}
                                                </div>
                                                {/* <div>
                                                    {getStatusBadge(
                                                        task.state,
                                                        t
                                                    )}
                                                </div> */}
                                                {/* <div>
                                                    {task.state ===
                                                        TaskState.RUNNING && (
                                                        <>
                                                            <Progress
                                                                value={
                                                                    taskProgress
                                                                }
                                                                className="h-2 mb-1"
                                                            />
                                                            <div className="text-sm text-muted-foreground">
                                                                {taskProgress}%
                                                                - {task.speed}
                                                            </div>
                                                        </>
                                                    )}
                                                    {task.state ===
                                                        TaskState.DONE && (
                                                        <div className="text-sm text-green-600">
                                                            100% 完成
                                                        </div>
                                                    )}
                                                    {task.state ===
                                                        TaskState.PAUSED && (
                                                        <>
                                                            <Progress
                                                                value={
                                                                    taskProgress
                                                                }
                                                                className="h-2 mb-1"
                                                            />
                                                            <div className="text-sm text-muted-foreground">
                                                                {taskProgress}%
                                                                已暂停
                                                            </div>
                                                        </>
                                                    )}
                                                    {task.state ===
                                                        TaskState.FAILED && (
                                                        <>
                                                            <Progress
                                                                value={
                                                                    taskProgress
                                                                }
                                                                className="h-2 mb-1"
                                                            />
                                                            <div className="text-sm text-red-600">
                                                                {taskProgress}%
                                                                失败
                                                            </div>
                                                        </>
                                                    )}
                                                    {task.state ===
                                                        TaskState.PENDING && (
                                                        <div className="text-sm text-muted-foreground">
                                                            等待执行
                                                        </div>
                                                    )}
                                                </div> */}
                                                <div className="text-sm">
                                                    <div className="text-muted-foreground">
                                                        {TaskMgrHelper.formatSize(
                                                            task.total_size
                                                        )}
                                                    </div>
                                                </div>
                                                <div className="text-sm">
                                                    <div className="flex items-center gap-1 mb-1">
                                                        <Clock className="w-3 h-3" />
                                                        {TaskMgrHelper.formatTime(
                                                            task.create_time ||
                                                                task.update_time
                                                        )}
                                                    </div>
                                                </div>
                                                <div
                                                    className="flex items-center gap-1 justify-end"
                                                    onClick={(e) =>
                                                        e.stopPropagation()
                                                    }
                                                >
                                                    {getTaskActions(
                                                        task,
                                                        onNavigate,
                                                        t
                                                    )}
                                                </div>
                                            </div>
                                        </div>
                                    )}
                                </CardContent>
                            </Card>
                        );
                    })}
                    {showPagination && (
                        <div className="flex flex-col gap-2 pt-4 sm:flex-row sm:items-center sm:justify-between">
                            <div className="text-sm text-muted-foreground whitespace-nowrap">
                                {`${visibleStart}-${visibleEnd} / ${allTaskCount}`}
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
                                    {paginationItems.map((item, index) => {
                                        if (item === "ellipsis") {
                                            return (
                                                <PaginationItem
                                                    key={`ellipsis-${index}`}
                                                >
                                                    <PaginationEllipsis />
                                                </PaginationItem>
                                            );
                                        }

                                        return (
                                            <PaginationItem
                                                key={`page-${item}`}
                                            >
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
                                        );
                                    })}
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
            )}
        </>
    );
}
