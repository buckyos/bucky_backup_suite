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

interface TaskListProps {
    onNavigate?: (page: string, data?: any) => void;
}

export function TaskList({ onNavigate }: TaskListProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [loading, setLoading] = useState(true);
    const [loadingText, setLoadingText] = useState<string>(`${t.common.loading} ${t.nav.tasks}...`);
    // Ensure hooks order is stable on every render
    const [selectedTasks, setSelectedTasks] = useState<number[]>([]);
    const [searchQuery, setSearchQuery] = useState("");
    const [statusFilter, setStatusFilter] = useState("all");
    const [typeFilter, setTypeFilter] = useState("all");
    const [showFilters, setShowFilters] = useState(false);
    const [selectedTask, setSelectedTask] = useState<any>(null);

    useEffect(() => {
        setLoading(true);
        setLoadingText(`${t.common.loading} ${t.nav.tasks}...`);
        const id = window.setTimeout(() => setLoading(false), 650);
        return () => window.clearTimeout(id);
    }, [t]);

    if (loading) {
        return (
            <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-4`}>
                <div>
                    <h1 className="mb-2">{t.tasks.title}</h1>
                    <p className="text-muted-foreground">{t.tasks.subtitle}</p>
                </div>
                <LoadingPage status={loadingText} />
            </div>
        );
    }

    // Read counts to guide empty states
    const plansCount =
        0 &&
        (() => {
            try {
                const raw = localStorage.getItem("plans");
                if (!raw) return 0;
                const arr = JSON.parse(raw);
                return Array.isArray(arr) ? arr.length : 0;
            } catch {
                return 0;
            }
        })();
    const servicesCount =
        0 &&
        (() => {
            try {
                const raw = localStorage.getItem("services");
                if (!raw) return 0;
                const arr = JSON.parse(raw);
                return Array.isArray(arr) ? arr.length : 0;
            } catch {
                return 0;
            }
        })();

    const tasks = [
        // {
        //     id: 1,
        //     name: "系统文件夜间备份",
        //     type: "backup",
        //     status: "running",
        //     progress: 65,
        //     speed: "12.5 MB/s",
        //     remaining: "约 15 分钟",
        //     plan: "系统文件备份",
        //     startTime: "2024-01-15 23:00:00",
        //     estimatedEnd: "2024-01-15 23:30:00",
        //     totalSize: "2.1 GB",
        //     processedSize: "1.4 GB",
        // },
        // {
        //     id: 2,
        //     name: "项目文件增量备份",
        //     type: "backup",
        //     status: "completed",
        //     progress: 100,
        //     speed: "",
        //     remaining: "",
        //     plan: "项目文件备份",
        //     startTime: "2024-01-15 02:00:00",
        //     completedTime: "2024-01-15 02:45:00",
        //     totalSize: "856 MB",
        //     processedSize: "856 MB",
        // },
        // {
        //     id: 3,
        //     name: "恢复用户文档",
        //     type: "restore",
        //     status: "paused",
        //     progress: 35,
        //     speed: "",
        //     remaining: "已暂停",
        //     plan: "手动恢复",
        //     startTime: "2024-01-15 14:30:00",
        //     totalSize: "1.5 GB",
        //     processedSize: "525 MB",
        // },
        // {
        //     id: 4,
        //     name: "媒体文件备份",
        //     type: "backup",
        //     status: "failed",
        //     progress: 23,
        //     speed: "",
        //     remaining: "已失败",
        //     plan: "媒体文件备份",
        //     startTime: "2024-01-14 20:00:00",
        //     errorTime: "2024-01-14 20:15:00",
        //     totalSize: "8.3 GB",
        //     processedSize: "1.9 GB",
        //     error: "目标磁盘空间不足",
        // },
        // {
        //     id: 5,
        //     name: "配置文件备份",
        //     type: "backup",
        //     status: "queued",
        //     progress: 0,
        //     speed: "",
        //     remaining: "等待中",
        //     plan: "系统配置备份",
        //     scheduledTime: "2024-01-16 01:00:00",
        //     totalSize: "125 MB",
        //     processedSize: "0 MB",
        // },
    ];

    const getStatusBadge = (status: string) => {
        switch (status) {
            case "running":
                return (
                    <Badge className="bg-blue-100 text-blue-800 text-xs">
                        {t.tasks.running}
                    </Badge>
                );
            case "completed":
                return (
                    <Badge className="bg-green-100 text-green-800 text-xs">
                        {t.tasks.completed}
                    </Badge>
                );
            case "paused":
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                        {t.tasks.paused}
                    </Badge>
                );
            case "failed":
                return (
                    <Badge className="bg-red-100 text-red-800 text-xs">
                        {t.tasks.failed}
                    </Badge>
                );
            case "queued":
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
    };

    const getTypeBadge = (type: string) => {
        return type === "backup" ? (
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
    };

    const filteredTasks = tasks.filter((task) => {
        const matchesSearch =
            task.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
            task.plan.toLowerCase().includes(searchQuery.toLowerCase());
        const matchesStatus =
            statusFilter === "all" || task.status === statusFilter;
        const matchesType = typeFilter === "all" || task.type === typeFilter;

        return matchesSearch && matchesStatus && matchesType;
    });

    const runningTasks = filteredTasks.filter(
        (task) => task.status === "running"
    );
    const otherTasks = filteredTasks.filter(
        (task) => task.status !== "running"
    );

    const toggleTaskSelection = (taskId: number) => {
        setSelectedTasks((prev) =>
            prev.includes(taskId)
                ? prev.filter((id) => id !== taskId)
                : [...prev, taskId]
        );
    };

    const toggleAllSelection = () => {
        if (selectedTasks.length === filteredTasks.length) {
            setSelectedTasks([]);
        } else {
            setSelectedTasks(filteredTasks.map((task) => task.id));
        }
    };

    const getTaskActions = (task: any) => {
        const actions = [];

        if (task.status === "running") {
            actions.push(
                <Button
                    key="pause"
                    variant="outline"
                    size="sm"
                    className="gap-1"
                >
                    <Pause className="w-3 h-3" />
                    暂停
                </Button>
            );
        } else if (task.status === "paused") {
            actions.push(
                <Button
                    key="resume"
                    variant="outline"
                    size="sm"
                    className="gap-1"
                >
                    <Play className="w-3 h-3" />
                    继续
                </Button>
            );
        }

        if (task.status === "running" || task.status === "paused") {
            actions.push(
                <Button
                    key="stop"
                    variant="outline"
                    size="sm"
                    className="gap-1 text-destructive"
                >
                    <Square className="w-3 h-3" />
                    停止
                </Button>
            );
        }

        actions.push(
            <Button key="view" variant="outline" size="sm" className="gap-1">
                <Eye className="w-3 h-3" />
                详情
            </Button>
        );

        if (task.status !== "running") {
            actions.push(
                <Button
                    key="delete"
                    variant="outline"
                    size="sm"
                    className="gap-1 text-destructive hover:text-destructive-foreground hover:bg-destructive"
                >
                    <Trash2 className="w-3 h-3" />
                    删除
                </Button>
            );
        }

        return actions;
    };

    // Empty-state for no tasks at all
    if (false && tasks.length === 0) {
        return (
            <div
                className={`${
                    isMobile ? "p-4 pt-16" : "p-6"
                } flex items-center justify-center`}
            >
                <Card
                    className={`w-full ${
                        isMobile ? "" : "max-w-2xl"
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
            </div>
        );
    }

    return (
        <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-4`}>
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="mb-2">{t.tasks.title}</h1>
                    <p className="text-muted-foreground">{t.tasks.subtitle}</p>
                </div>
            </div>

            {tasks.length === 0 && (
                <Card className={`w-full ${isMobile ? "" : "max-w-2xl mx-auto"} text-center`}>
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
                        <div className={`flex ${isMobile ? "flex-col gap-2" : "items-center justify-center gap-3"}`}>
                            {servicesCount === 0 ? (
                                <>
                                    <Button onClick={() => onNavigate?.("add-service")} className="gap-2">去配置备份服务</Button>
                                    {!isMobile && <span className="text-muted-foreground">或</span>}
                                    <Button variant="outline" disabled className="gap-2">新建备份计划</Button>
                                </>
                            ) : plansCount === 0 ? (
                                <Button onClick={() => onNavigate?.("create-plan")} className="gap-2">新建备份计划</Button>
                            ) : (
                                <Button onClick={() => onNavigate?.("plans")} className="gap-2">前往计划列表执行一次备份</Button>
                            )}
                        </div>
                    </CardContent>
                </Card>
            )}

            {/* 筛选器和搜索 */}
            {!isMobile && tasks.length > 0 && (
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
            )}

            {/* 任务列表 */}
            {tasks.length > 0 && (
            <Tabs defaultValue="running" className="space-y-4">
                <div className="flex items-center justify-between">
                    <TabsList>
                        <TabsTrigger value="running">
                            {t.tasks.runningTasks} ({runningTasks.length})
                        </TabsTrigger>
                        <TabsTrigger value="all">
                            {t.tasks.allTasks} ({filteredTasks.length})
                        </TabsTrigger>
                    </TabsList>

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
                    )}
                </div>

                <TabsContent value="all" className="space-y-4">
                    {filteredTasks.length === 0 ? (
                        <Card>
                            <CardContent className="py-12 text-center">
                                <p className="text-muted-foreground">
                                    没有找到匹配的任务
                                </p>
                            </CardContent>
                        </Card>
                    ) : (
                        <>
                            {!isMobile && (
                                <Card>
                                    <CardContent className="py-3">
                                        <div className="flex items-center gap-4">
                                            <Checkbox
                                                checked={
                                                    selectedTasks.length ===
                                                        filteredTasks.length &&
                                                    filteredTasks.length > 0
                                                }
                                                onCheckedChange={
                                                    toggleAllSelection
                                                }
                                            />
                                            <div className="grid grid-cols-6 gap-4 flex-1 items-center">
                                                <div className="font-medium">
                                                    任务名称
                                                </div>
                                                <div className="font-medium">
                                                    {t.common.type}
                                                </div>
                                                <div className="font-medium">
                                                    {t.common.status}
                                                </div>
                                                <div className="font-medium">
                                                    {t.tasks.progress}
                                                </div>
                                                <div className="font-medium">
                                                    时间
                                                </div>
                                                <div className="font-medium text-right">
                                                    {t.common.actions}
                                                </div>
                                            </div>
                                        </div>
                                    </CardContent>
                                </Card>
                            )}

                            {/* 任务列表 */}
                            {filteredTasks.map((task) => (
                                <Card
                                    key={task.id}
                                    className="cursor-pointer hover:bg-accent/50"
                                    onClick={() => setSelectedTask(task)}
                                >
                                    <CardContent
                                        className={`${
                                            isMobile ? "py-3" : "py-4"
                                        }`}
                                    >
                                        {isMobile ? (
                                            // 移动端紧凑布局
                                            <div className="space-y-2">
                                                <div className="flex items-center justify-between">
                                                    <div className="flex items-center gap-2 flex-1 min-w-0">
                                                        <span className="font-medium text-sm truncate">
                                                            {task.name}
                                                        </span>
                                                        {getStatusBadge(
                                                            task.status
                                                        )}
                                                        {getTypeBadge(
                                                            task.type
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
                                                            <DropdownMenuItem
                                                                onClick={(
                                                                    e
                                                                ) => {
                                                                    e.stopPropagation();
                                                                    setSelectedTask(
                                                                        task
                                                                    );
                                                                }}
                                                            >
                                                                <Eye className="w-4 h-4 mr-2" />
                                                                {
                                                                    t.common
                                                                        .details
                                                                }
                                                            </DropdownMenuItem>
                                                            {task.status ===
                                                                "running" && (
                                                                <DropdownMenuItem
                                                                    onClick={(
                                                                        e
                                                                    ) =>
                                                                        e.stopPropagation()
                                                                    }
                                                                >
                                                                    <Pause className="w-4 h-4 mr-2" />
                                                                    {
                                                                        t.tasks
                                                                            .pause
                                                                    }
                                                                </DropdownMenuItem>
                                                            )}
                                                            {task.status ===
                                                                "paused" && (
                                                                <DropdownMenuItem
                                                                    onClick={(
                                                                        e
                                                                    ) =>
                                                                        e.stopPropagation()
                                                                    }
                                                                >
                                                                    <Play className="w-4 h-4 mr-2" />
                                                                    {
                                                                        t.tasks
                                                                            .resume
                                                                    }
                                                                </DropdownMenuItem>
                                                            )}
                                                            {task.status ===
                                                                "completed" &&
                                                                task.type ===
                                                                    "backup" && (
                                                                    <DropdownMenuItem
                                                                        onClick={(
                                                                            e
                                                                        ) => {
                                                                            e.stopPropagation();
                                                                            onNavigate?.(
                                                                                "restore",
                                                                                {
                                                                                    taskId: task.id,
                                                                                }
                                                                            );
                                                                        }}
                                                                    >
                                                                        <FileText className="w-4 h-4 mr-2" />
                                                                        恢复
                                                                    </DropdownMenuItem>
                                                                )}
                                                            {task.status !==
                                                                "running" && (
                                                                <DropdownMenuItem
                                                                    onClick={(
                                                                        e
                                                                    ) =>
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

                                                {task.status !==
                                                    "completed" && (
                                                    <div className="space-y-1">
                                                        <Progress
                                                            value={
                                                                task.progress
                                                            }
                                                            className="h-1.5"
                                                        />
                                                        <div className="flex justify-between text-xs text-muted-foreground">
                                                            <span>
                                                                {task.progress}%
                                                                {task.speed &&
                                                                    ` • ${task.speed}`}
                                                            </span>
                                                            <span>
                                                                {task.remaining ||
                                                                    `${task.processedSize}/${task.totalSize}`}
                                                            </span>
                                                        </div>
                                                    </div>
                                                )}

                                                <div className="flex justify-between text-xs text-muted-foreground">
                                                    <span>{task.plan}</span>
                                                    <span>
                                                        {task.startTime
                                                            ? new Date(
                                                                  task.startTime
                                                              ).toLocaleDateString()
                                                            : task.scheduledTime
                                                            ? new Date(
                                                                  task.scheduledTime
                                                              ).toLocaleDateString()
                                                            : "-"}
                                                    </span>
                                                </div>

                                                {task.error && (
                                                    <div className="p-2 bg-red-50 text-red-700 rounded-md text-xs">
                                                        {task.error}
                                                    </div>
                                                )}
                                            </div>
                                        ) : (
                                            // 桌面端详细布局
                                            <div className="flex items-center gap-4">
                                                <Checkbox
                                                    checked={selectedTasks.includes(
                                                        task.id
                                                    )}
                                                    onCheckedChange={() =>
                                                        toggleTaskSelection(
                                                            task.id
                                                        )
                                                    }
                                                    onClick={(e) =>
                                                        e.stopPropagation()
                                                    }
                                                />
                                                <div className="grid grid-cols-6 gap-4 flex-1 items-center">
                                                    <div>
                                                        <p className="font-medium">
                                                            {task.name}
                                                        </p>
                                                        <p className="text-sm text-muted-foreground">
                                                            {task.plan}
                                                        </p>
                                                    </div>
                                                    <div>
                                                        {getTypeBadge(
                                                            task.type
                                                        )}
                                                    </div>
                                                    <div>
                                                        {getStatusBadge(
                                                            task.status
                                                        )}
                                                    </div>
                                                    <div>
                                                        {task.status ===
                                                            "running" && (
                                                            <>
                                                                <Progress
                                                                    value={
                                                                        task.progress
                                                                    }
                                                                    className="h-2 mb-1"
                                                                />
                                                                <div className="text-sm text-muted-foreground">
                                                                    {
                                                                        task.progress
                                                                    }
                                                                    % -{" "}
                                                                    {task.speed}
                                                                </div>
                                                            </>
                                                        )}
                                                        {task.status ===
                                                            "completed" && (
                                                            <div className="text-sm text-green-600">
                                                                100% 完成
                                                            </div>
                                                        )}
                                                        {task.status ===
                                                            "paused" && (
                                                            <>
                                                                <Progress
                                                                    value={
                                                                        task.progress
                                                                    }
                                                                    className="h-2 mb-1"
                                                                />
                                                                <div className="text-sm text-muted-foreground">
                                                                    {
                                                                        task.progress
                                                                    }
                                                                    % 已暂停
                                                                </div>
                                                            </>
                                                        )}
                                                        {task.status ===
                                                            "failed" && (
                                                            <>
                                                                <Progress
                                                                    value={
                                                                        task.progress
                                                                    }
                                                                    className="h-2 mb-1"
                                                                />
                                                                <div className="text-sm text-red-600">
                                                                    {
                                                                        task.progress
                                                                    }
                                                                    % 失败
                                                                </div>
                                                            </>
                                                        )}
                                                        {task.status ===
                                                            "queued" && (
                                                            <div className="text-sm text-muted-foreground">
                                                                等待执行
                                                            </div>
                                                        )}
                                                    </div>
                                                    <div className="text-sm">
                                                        <div className="flex items-center gap-1 mb-1">
                                                            <Clock className="w-3 h-3" />
                                                            {task.startTime
                                                                ? new Date(
                                                                      task.startTime
                                                                  ).toLocaleString()
                                                                : task.scheduledTime
                                                                ? `计划: ${new Date(
                                                                      task.scheduledTime
                                                                  ).toLocaleString()}`
                                                                : "-"}
                                                        </div>
                                                        <div className="text-muted-foreground">
                                                            {task.processedSize}{" "}
                                                            / {task.totalSize}
                                                        </div>
                                                    </div>
                                                    <div
                                                        className="flex items-center gap-1 justify-end"
                                                        onClick={(e) =>
                                                            e.stopPropagation()
                                                        }
                                                    >
                                                        {getTaskActions(task)}
                                                    </div>
                                                </div>
                                            </div>
                                        )}

                                        {!isMobile && task.error && (
                                            <div className="mt-3 p-3 bg-red-50 text-red-700 rounded-md text-sm">
                                                错误: {task.error}
                                            </div>
                                        )}
                                    </CardContent>
                                </Card>
                            ))}
                        </>
                    )}
                </TabsContent>

                <TabsContent value="running" className="space-y-4">
                    {runningTasks.map((task) => (
                        <Card
                            key={task.id}
                            className="cursor-pointer hover:bg-accent/50"
                            onClick={() => setSelectedTask(task)}
                        >
                            <CardHeader
                                className={`${isMobile ? "pb-2" : "pb-4"}`}
                            >
                                <div className="flex items-center justify-between">
                                    <div className="flex-1 min-w-0">
                                        <CardTitle
                                            className={`${
                                                isMobile
                                                    ? "text-base"
                                                    : "text-lg"
                                            } truncate`}
                                        >
                                            {task.name}
                                        </CardTitle>
                                        <CardDescription className="text-sm">
                                            {task.plan}
                                        </CardDescription>
                                    </div>
                                    <div className="flex items-center gap-2 flex-shrink-0">
                                        {getStatusBadge(task.status)}
                                        {getTypeBadge(task.type)}
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
                                                            setSelectedTask(
                                                                task
                                                            );
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
                                                {t.tasks.progress}:{" "}
                                                {task.progress}%
                                            </span>
                                            <span>{task.speed}</span>
                                        </div>
                                        <Progress
                                            value={task.progress}
                                            className="h-2"
                                        />
                                        <div className="flex justify-between text-sm text-muted-foreground mt-1">
                                            <span>
                                                {task.processedSize} /{" "}
                                                {task.totalSize}
                                            </span>
                                            <span>{task.remaining}</span>
                                        </div>
                                    </div>
                                    <div className="flex items-center justify-between pt-2 border-t">
                                        <div className="text-xs text-muted-foreground">
                                            开始时间:{" "}
                                            {new Date(
                                                task.startTime
                                            ).toLocaleString()}
                                        </div>
                                        {!isMobile && (
                                            <div
                                                className="flex items-center gap-2"
                                                onClick={(e) =>
                                                    e.stopPropagation()
                                                }
                                            >
                                                {getTaskActions(task)}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            </CardContent>
                        </Card>
                    ))}
                    {runningTasks.length === 0 && (
                        <Card>
                            <CardContent className="py-12 text-center">
                                <p className="text-muted-foreground">
                                    当前没有执行中的任务
                                </p>
                            </CardContent>
                        </Card>
                    )}
                </TabsContent>
            </Tabs>
            )}

            {/* 任务详情对话框 */}
            {selectedTask && (
                <div className="fixed inset-0 bg-background z-50">
                    <TaskDetail
                        task={selectedTask}
                        onBack={() => setSelectedTask(null)}
                    />
                </div>
            )}
        </div>
    );
}
