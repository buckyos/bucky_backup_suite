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
} from "lucide-react";
import { Translations } from "./i18n";
import {
    BackupPlanInfo,
    TaskEventType,
    TaskInfo,
    TaskState,
    TaskType,
} from "./utils/task_mgr";
import { taskManager } from "./utils/fake_task_mgr";
import { PlanState, TaskMgrHelper } from "./utils/task_mgr_helper";

interface TaskListProps {
    onNavigate?: (page: string, data?: any) => void;
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

enum TaskAction {
    PAUSE,
    REMOVE,
    RESUME,
    DETAIL,
}

function getTaskActions(
    task: TaskInfo,
    onShowDetail: (task: TaskInfo) => void,
    t: Translations
) {
    const actions = [
        <Button
            key="view"
            variant="outline"
            size="sm"
            className="gap-1"
            onClick={() => onShowDetail(task)}
        >
            <Eye className="w-3 h-3" />
            详情
        </Button>,
    ];

    if (task.state === TaskState.RUNNING || task.state === TaskState.PENDING) {
        actions.push(
            <Button
                key="pause"
                variant="outline"
                size="sm"
                className="gap-1"
                onClick={() => taskManager.pauseBackupTask(task.taskid)}
            >
                <Pause className="w-3 h-3" />
                暂停
            </Button>
        );
    } else {
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

        if (task.state === TaskState.PAUSED) {
            actions.push(
                <Button
                    key="resume"
                    variant="outline"
                    size="sm"
                    className="gap-1"
                    onClick={() => taskManager.resumeBackupTask(task.taskid)}
                >
                    <Play className="w-3 h-3" />
                    继续
                </Button>
            );
        }
    }

    return actions;
}

export function TaskList({ onNavigate }: TaskListProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [runningTaskCount, setRunningTaskCount] = useState(0);
    const [filterTaskCount, setFilterTaskCount] = useState(0);
    const [allTaskCount, setAllTaskCount] = useState(0);
    const [selectedRunningTasks, setSelectedRunningTasks] = useState<
        TaskInfo[]
    >([]);
    const [selectedAnyTasks, setSelectedAnyTasks] = useState<TaskInfo[]>([]);
    const [showDetailTask, setShowDetailTask] = useState<TaskInfo | null>(null);
    const [plans, setPlans] = useState<BackupPlanInfo[]>([]);
    const [activeTab, setActiveTab] = useState<"running" | "all">("running");

    useEffect(() => {
        const timerId = taskManager.startRefreshUncompleteTaskStateTimer();
        return () => taskManager.stopRefreshUncompleteTaskStateTimer(timerId);
    }, []);

    const selectTasksInCurrentTab = () => {
        if (activeTab === "all") {
            return selectedAnyTasks;
        }
        return selectedRunningTasks;
    };

    return (
        <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-4`}>
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="mb-2">{t.tasks.title}</h1>
                    <p className="text-muted-foreground">{t.tasks.subtitle}</p>
                </div>
            </div>

            {/* {tasks.length === 0 && (
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
                                        onClick={() =>
                                            onNavigate?.("add-service")
                                        }
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
            )} */}

            {/* 筛选器和搜索 */}
            {/* {!isMobile && tasks.length > 0 && (
                <Card>
                    <CardHeader>
                        <CardTitle className="text-lg">筛选和搜索</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                            <div className="space-y-2">
                                <label className="text-sm font-medium">
                                    {t.common.search}
                                </label>
                                <div className="relative">
                                    <Search className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                    <Input
                                        placeholder="搜索任务或计划名称..."
                                        value={searchQuery}
                                        onChange={(e) =>
                                            setSearchQuery(e.target.value)
                                        }
                                        className="pl-10"
                                    />
                                </div>
                            </div>
                            <div className="space-y-2">
                                <label className="text-sm font-medium">
                                    {t.common.status}
                                </label>
                                <Select
                                    value={statusFilter}
                                    onValueChange={setStatusFilter}
                                >
                                    <SelectTrigger>
                                        <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="all">
                                            全部状态
                                        </SelectItem>
                                        <SelectItem value="running">
                                            {t.tasks.running}
                                        </SelectItem>
                                        <SelectItem value="completed">
                                            {t.tasks.completed}
                                        </SelectItem>
                                        <SelectItem value="paused">
                                            {t.tasks.paused}
                                        </SelectItem>
                                        <SelectItem value="failed">
                                            {t.tasks.failed}
                                        </SelectItem>
                                        <SelectItem value="queued">
                                            {t.tasks.queued}
                                        </SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                            <div className="space-y-2">
                                <label className="text-sm font-medium">
                                    {t.common.type}
                                </label>
                                <Select
                                    value={typeFilter}
                                    onValueChange={setTypeFilter}
                                >
                                    <SelectTrigger>
                                        <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="all">
                                            全部类型
                                        </SelectItem>
                                        <SelectItem value="backup">
                                            {t.tasks.backup}任务
                                        </SelectItem>
                                        <SelectItem value="restore">
                                            {t.tasks.restore}任务
                                        </SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                            <div className="space-y-2">
                                <label className="text-sm font-medium">
                                    批量操作
                                </label>
                                <div className="flex gap-2">
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        disabled={selectedTasks.length === 0}
                                    >
                                        批量暂停
                                    </Button>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        disabled={selectedTasks.length === 0}
                                    >
                                        批量删除
                                    </Button>
                                </div>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            )} */}

            {/* 任务列表 */}
            <Tabs
                defaultValue="running"
                className="space-y-4"
                onValueChange={(val) => {
                    console.log("Selected tab:", val);
                    setActiveTab(val as "running" | "all");
                }}
            >
                <div className="flex items-center justify-between">
                    <TabsList>
                        <TabsTrigger value="running">
                            {t.tasks.runningTasks} ({runningTaskCount})
                        </TabsTrigger>
                        <TabsTrigger value="all">
                            {t.tasks.allTasks} ({filterTaskCount}/{allTaskCount}
                            )
                        </TabsTrigger>
                    </TabsList>
                    {/* 
                    {isMobile && (
                        <Sheet open={showFilters} onOpenChange={setShowFilters}>
                            <SheetTrigger asChild>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="gap-2"
                                >
                                    <Filter className="w-4 h-4" />
                                    {t.common.filter}
                                </Button>
                            </SheetTrigger>
                            <SheetContent side="right" className="w-80">
                                <SheetHeader>
                                    <SheetTitle>筛选和搜索</SheetTitle>
                                </SheetHeader>
                                <div className="space-y-4 mt-6">
                                    <div className="space-y-2">
                                        <label className="text-sm font-medium">
                                            {t.common.search}
                                        </label>
                                        <div className="relative">
                                            <Search className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
                                            <Input
                                                placeholder="搜索任务或计划名称..."
                                                value={searchQuery}
                                                onChange={(e) =>
                                                    setSearchQuery(
                                                        e.target.value
                                                    )
                                                }
                                                className="pl-10"
                                            />
                                        </div>
                                    </div>
                                    <div className="space-y-2">
                                        <label className="text-sm font-medium">
                                            {t.common.status}
                                        </label>
                                        <Select
                                            value={statusFilter}
                                            onValueChange={setStatusFilter}
                                        >
                                            <SelectTrigger>
                                                <SelectValue />
                                            </SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="all">
                                                    全部状态
                                                </SelectItem>
                                                <SelectItem value="running">
                                                    {t.tasks.running}
                                                </SelectItem>
                                                <SelectItem value="completed">
                                                    {t.tasks.completed}
                                                </SelectItem>
                                                <SelectItem value="paused">
                                                    {t.tasks.paused}
                                                </SelectItem>
                                                <SelectItem value="failed">
                                                    {t.tasks.failed}
                                                </SelectItem>
                                                <SelectItem value="queued">
                                                    {t.tasks.queued}
                                                </SelectItem>
                                            </SelectContent>
                                        </Select>
                                    </div>
                                    <div className="space-y-2">
                                        <label className="text-sm font-medium">
                                            {t.common.type}
                                        </label>
                                        <Select
                                            value={typeFilter}
                                            onValueChange={setTypeFilter}
                                        >
                                            <SelectTrigger>
                                                <SelectValue />
                                            </SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="all">
                                                    全部类型
                                                </SelectItem>
                                                <SelectItem value="backup">
                                                    {t.tasks.backup}任务
                                                </SelectItem>
                                                <SelectItem value="restore">
                                                    {t.tasks.restore}任务
                                                </SelectItem>
                                            </SelectContent>
                                        </Select>
                                    </div>
                                </div>
                            </SheetContent>
                        </Sheet>
                    )} */}
                </div>

                <TabsContent value="all" className="space-y-4">
                    <AllTaskTabContent
                        t={t}
                        isMobile={isMobile}
                        setFilterTaskCount={setFilterTaskCount}
                        setTaskCount={setAllTaskCount}
                        selectedTasks={selectedAnyTasks}
                        setSelectedTasks={setSelectedAnyTasks}
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
                        selectedTasks={selectedRunningTasks}
                        setSelectedTasks={setSelectedRunningTasks}
                        showDetailTask={setShowDetailTask}
                        plans={plans}
                        setPlans={setPlans}
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

function RunningTaskTabContent({
    isMobile,
    t,
    setTaskCount,
    selectedTasks,
    setSelectedTasks,
    showDetailTask,
    plans,
    setPlans,
}: {
    isMobile?: boolean;
    t: Translations;
    setTaskCount: (count: number) => void;
    selectedTasks: TaskInfo[];
    setSelectedTasks: (task: TaskInfo[]) => void;
    showDetailTask: (task: TaskInfo | null) => void;
    plans: BackupPlanInfo[];
    setPlans: (plans: BackupPlanInfo[]) => void;
}) {
    const [uncompleteTasks, setUncompleteTasks] = useState<TaskInfo[] | null>(
        null
    );

    const refreshUncompleteTasks = () => {
        taskManager
            .listBackupTasks({
                state: [
                    TaskState.RUNNING,
                    TaskState.PENDING,
                    TaskState.PAUSED,
                    TaskState.FAILED,
                ],
            })
            .then(async ({ task_ids }) => {
                const taskInfos = await Promise.all(
                    task_ids.map((taskid) => taskManager.getTaskInfo(taskid))
                );
                for (const task of taskInfos) {
                    if (!plans.find((p) => task.owner_plan_id === p.plan_id)) {
                        const newPlanIds = await taskManager.listBackupPlans();
                        const newPlans = await Promise.all(
                            newPlanIds.map((planId) =>
                                taskManager.getBackupPlan(planId)
                            )
                        );
                        plans.splice(0, plans.length);
                        newPlans.forEach((p) => plans.push(p));
                        setPlans(plans);
                    }
                }
                setUncompleteTasks(taskInfos);
                setTaskCount(taskInfos.length);
            });
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

    return (
        <>
            {uncompleteTasks!.map((task) => {
                const taskProgress = TaskMgrHelper.taskProgress(task);
                const taskRemainStr = TaskMgrHelper.taskRemainingStr(task);
                return (
                    <Card
                        key={task.taskid}
                        className="cursor-pointer hover:bg-accent/50"
                        onClick={() => showDetailTask(task)}
                    >
                        <CardHeader className={`${isMobile ? "pb-2" : "pb-4"}`}>
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
                                                    onClick={(e) => {
                                                        e.stopPropagation();
                                                        showDetailTask(task);
                                                    }}
                                                >
                                                    <Eye className="w-4 h-4 mr-2" />
                                                    {t.common.details}
                                                </DropdownMenuItem>
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
                        <CardContent>
                            <div className="space-y-3">
                                <div>
                                    <div className="flex justify-between text-sm mb-2">
                                        <span>
                                            {t.tasks.progress}: {taskProgress}%
                                        </span>
                                        <span>{task.speed}</span>
                                    </div>
                                    <Progress
                                        value={taskProgress}
                                        className="h-2"
                                    />
                                    <div className="flex justify-between text-sm text-muted-foreground mt-1">
                                        <span>taskProgress</span>
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
                                                showDetailTask,
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

function AllTaskTabContent({
    isMobile,
    t,
    setFilterTaskCount,
    setTaskCount,
    selectedTasks,
    setSelectedTasks,
    showDetailTask,
    plans,
    setPlans,
    onNavigate,
}: {
    isMobile?: boolean;
    t: Translations;
    setFilterTaskCount: (count: number) => void;
    setTaskCount: (count: number) => void;
    selectedTasks: TaskInfo[];
    setSelectedTasks: (task: TaskInfo[]) => void;
    showDetailTask: (task: TaskInfo | null) => void;
    plans: BackupPlanInfo[];
    setPlans: (plans: BackupPlanInfo[]) => void;
    onNavigate?: (page: string, data?: any) => void;
}) {
    // Ensure hooks order is stable on every render
    const [searchPlanFilter, setPlanFilter] = useState("");
    const [statusFilter, setStatusFilter] = useState<TaskState[] | null>(null);
    const [typeFilter, setTypeFilter] = useState<TaskType | null>(null);
    const [filterTasks, setFilterTasks] = useState<TaskInfo[] | null>(null);
    const [isSelectAll, setIsSelectAll] = useState(false);

    const toggleTaskSelection = (task: TaskInfo) => {
        const index = selectedTasks.findIndex((t) => t.taskid === task.taskid);
        if (index === -1) {
            selectedTasks.push(task);
            setSelectedTasks(selectedTasks);
        } else {
            selectedTasks.splice(index, 1);
            setSelectedTasks(selectedTasks);
        }
    };

    const toggleAllSelection = () => {
        if (isSelectAll) {
            setIsSelectAll(false);
            setSelectedTasks([]);
        } else {
            setIsSelectAll(true);
            setSelectedTasks([...filterTasks]);
        }
    };

    const refreshFilterTasks = () => {
        taskManager
            .listBackupTasks({
                owner_plan_title: [searchPlanFilter],
                type: typeFilter ? [typeFilter] : undefined,
                state: statusFilter ? statusFilter : undefined,
            })
            .then(async ({ task_ids, total }) => {
                const taskInfos = await Promise.all(
                    task_ids.map((taskid) => taskManager.getTaskInfo(taskid))
                );
                for (const task of taskInfos) {
                    if (!plans.find((p) => task.owner_plan_id === p.plan_id)) {
                        const newPlanIds = await taskManager.listBackupPlans();
                        const newPlans = await Promise.all(
                            newPlanIds.map((planId) =>
                                taskManager.getBackupPlan(planId)
                            )
                        );
                        plans.splice(0, plans.length);
                        newPlans.forEach((p) => plans.push(p));
                        setPlans(plans);
                    }
                }
                setFilterTasks(taskInfos);
                setFilterTaskCount(total);
                if (isSelectAll) {
                    selectedTasks.splice(0, selectedTasks.length);
                    taskInfos.forEach((t) => selectedTasks.push(t));
                    setSelectedTasks(selectedTasks);
                } else {
                    const newSelectedTasks = selectedTasks.filter((t) =>
                        taskInfos.find((ft) => ft.taskid === t.taskid)
                    );
                    selectedTasks.splice(0, selectedTasks.length);
                    newSelectedTasks.forEach((t) => selectedTasks.push(t));
                    setSelectedTasks(selectedTasks);
                }
            });
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
                refreshFilterTasks();
                break;
        }
    };

    useEffect(() => {
        refreshFilterTasks();

        taskManager.addTaskEventListener(taskEventHandler);
        return () => {
            taskManager.removeTaskEventListener(taskEventHandler);
        };
    }, []);

    if (filterTasks === null) {
        return <Loading isMobile={isMobile} t={t} />;
    }

    return filterTasks.length === 0 ? (
        <Card>
            <CardContent className="py-12 text-center">
                <p className="text-muted-foreground">没有找到匹配的任务</p>
            </CardContent>
        </Card>
    ) : (
        <>
            {!isMobile && (
                <Card>
                    <CardContent className="py-3">
                        <div className="flex items-center gap-4">
                            <Checkbox
                                checked={isSelectAll}
                                onCheckedChange={toggleAllSelection}
                            />
                            <div className="grid grid-cols-6 gap-4 flex-1 items-center">
                                <div className="font-medium">任务名称</div>
                                <div className="font-medium">
                                    {t.common.type}
                                </div>
                                <div className="font-medium">
                                    {t.common.status}
                                </div>
                                <div className="font-medium">
                                    {t.tasks.progress}
                                </div>
                                <div className="font-medium">时间</div>
                                <div className="font-medium text-right">
                                    {t.common.actions}
                                </div>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* 任务列表 */}
            {filterTasks.map((task) => {
                const taskProgress = TaskMgrHelper.taskProgress(task);
                const taskRemainStr = TaskMgrHelper.taskRemainingStr(task);
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
                                            {getStatusBadge(task.state, t)}
                                            {getTypeBadge(task.task_type, t)}
                                        </div>
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
                                                    onClick={(e) => {
                                                        e.stopPropagation();
                                                        showDetailTask(task);
                                                    }}
                                                >
                                                    <Eye className="w-4 h-4 mr-2" />
                                                    {t.common.details}
                                                </DropdownMenuItem>
                                                {(task.state ===
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
                                                )}
                                                {task.state ===
                                                    TaskState.DONE &&
                                                    task.task_type ===
                                                        TaskType.BACKUP && (
                                                        <DropdownMenuItem
                                                            onClick={(e) => {
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
                                                        {t.common.delete}
                                                    </DropdownMenuItem>
                                                )}
                                            </DropdownMenuContent>
                                        </DropdownMenu>
                                    </div>

                                    {task.state !== TaskState.DONE && (
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
                                    )}

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
                                            {task.create_time
                                                ? new Date(
                                                      task.create_time
                                                  ).toLocaleDateString()
                                                : task.update_time
                                                ? new Date(
                                                      task.update_time
                                                  ).toLocaleDateString()
                                                : "--"}
                                        </span>
                                    </div>
                                </div>
                            ) : (
                                // 桌面端详细布局
                                <div className="flex items-center gap-4">
                                    <Checkbox
                                        checked={selectedTasks.find(
                                            (t) => t.taskid === task.taskid
                                        )}
                                        onCheckedChange={() =>
                                            toggleTaskSelection(task)
                                        }
                                        onClick={(e) => e.stopPropagation()}
                                    />
                                    <div className="grid grid-cols-6 gap-4 flex-1 items-center">
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
                                            {getTypeBadge(task.task_type, t)}
                                        </div>
                                        <div>
                                            {getStatusBadge(task.state, t)}
                                        </div>
                                        <div>
                                            {task.state ===
                                                TaskState.RUNNING && (
                                                <>
                                                    <Progress
                                                        value={taskProgress}
                                                        className="h-2 mb-1"
                                                    />
                                                    <div className="text-sm text-muted-foreground">
                                                        {taskProgress}% -{" "}
                                                        {task.speed}
                                                    </div>
                                                </>
                                            )}
                                            {task.state === TaskState.DONE && (
                                                <div className="text-sm text-green-600">
                                                    100% 完成
                                                </div>
                                            )}
                                            {task.state ===
                                                TaskState.PAUSED && (
                                                <>
                                                    <Progress
                                                        value={taskProgress}
                                                        className="h-2 mb-1"
                                                    />
                                                    <div className="text-sm text-muted-foreground">
                                                        {taskProgress}% 已暂停
                                                    </div>
                                                </>
                                            )}
                                            {task.state ===
                                                TaskState.FAILED && (
                                                <>
                                                    <Progress
                                                        value={taskProgress}
                                                        className="h-2 mb-1"
                                                    />
                                                    <div className="text-sm text-red-600">
                                                        {taskProgress}% 失败
                                                    </div>
                                                </>
                                            )}
                                            {task.state ===
                                                TaskState.PENDING && (
                                                <div className="text-sm text-muted-foreground">
                                                    等待执行
                                                </div>
                                            )}
                                        </div>
                                        <div className="text-sm">
                                            <div className="flex items-center gap-1 mb-1">
                                                <Clock className="w-3 h-3" />
                                                {task.create_time
                                                    ? new Date(
                                                          task.create_time
                                                      ).toLocaleString()
                                                    : task.update_time
                                                    ? `计划: ${new Date(
                                                          task.update_time
                                                      ).toLocaleString()}`
                                                    : "-"}
                                            </div>
                                            <div className="text-muted-foreground">
                                                {task.completed_size} /{" "}
                                                {task.total_size}
                                            </div>
                                        </div>
                                        <div
                                            className="flex items-center gap-1 justify-end"
                                            onClick={(e) => e.stopPropagation()}
                                        >
                                            {getTaskActions(
                                                task,
                                                showDetailTask,
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
        </>
    );
}
