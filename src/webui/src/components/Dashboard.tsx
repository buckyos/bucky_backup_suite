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
import { toast } from "sonner@2.0.3";
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
import { taskManager } from "./utils/task_mgr";
import { LoadingPage } from "./LoadingPage";

interface DashboardProps {
    onNavigate?: (page: string, data?: any) => void;
}

export function Dashboard({ onNavigate }: DashboardProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [selectedPlan, setSelectedPlan] = useState("");
    const [loading, setLoading] = useState(true);
    const [loadingText, setLoadingText] = useState<string>(`${t.common.loading} ${t.nav.dashboard}...`);

    useEffect(() => {
        // 示例：按阶段更新加载文案，真实项目中可替换为实际数据加载步骤
        setLoading(true);
        setLoadingText(`${t.common.loading} ${t.nav.dashboard}...`);
        const timers: number[] = [];
        timers.push(window.setTimeout(() => setLoadingText(t.dashboard.currentTasks), 200));
        timers.push(window.setTimeout(() => setLoadingText(t.dashboard.recentActivities), 450));
        timers.push(window.setTimeout(() => setLoading(false), 700));
        return () => {
            timers.forEach((id) => window.clearTimeout(id));
        };
    }, [t]);

    // Read persisted counts to decide whether to show guidance panel
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

    // 模拟数据
    const currentTasks = [
        // {
        //     id: 1,
        //     name: "系统文件夜间备份",
        //     plan: "系统备份",
        //     progress: 65,
        //     speed: "12.5 MB/s",
        //     remaining: "约 15 分钟",
        //     status: "running",
        // },
        // {
        //     id: 2,
        //     name: "项目文件增量备份",
        //     plan: "项目备份",
        //     progress: 100,
        //     speed: "",
        //     remaining: "",
        //     status: "completed",
        // },
    ];

    const recentActivities = [
        {
            id: 1,
            name: "文档备份计划",
            status: "success",
            time: "2h",
            size: "2.1 GB",
        },
        {
            id: 2,
            name: "系统备份计划",
            status: "warning",
            time: "4h",
            size: "15.6 GB",
        },
        {
            id: 3,
            name: "媒体文件备份",
            status: "error",
            time: "6h",
            size: "8.3 GB",
        },
    ];

    const backupServices = [
        {
            id: 1,
            name: "本地备份盘",
            type: "local",
            status: "healthy",
            used: "450 GB",
            total: "2 TB",
            usagePercent: 22,
        },
        {
            id: 2,
            name: "NDN网络节点1",
            type: "ndn",
            status: "healthy",
            used: "1.2 TB",
            total: "无限制",
            usagePercent: 0,
        },
        {
            id: 3,
            name: "外部硬盘",
            type: "local",
            status: "warning",
            used: "1.8 TB",
            total: "2 TB",
            usagePercent: 90,
        },
    ];

    const backupPlans = [
        {
            id: 1,
            name: "系统文件备份",
            enabled: true,
            nextRun: "今天 23:00",
            status: "healthy",
        },
        {
            id: 2,
            name: "项目文件备份",
            enabled: true,
            nextRun: "明天 02:00",
            status: "healthy",
        },
        {
            id: 3,
            name: "媒体文件备份",
            enabled: false,
            nextRun: "已禁用",
            status: "disabled",
        },
    ];

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

    const getStatusBadge = (status: string) => {
        switch (status) {
            case "running":
                return (
                    <Badge variant="default" className="bg-blue-500 text-xs">
                        执行中
                    </Badge>
                );
            case "completed":
                return (
                    <Badge variant="secondary" className="bg-green-500 text-xs">
                        已完成
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

    const getPlanStatusBadge = (status: string) => {
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
            case "disabled":
                return (
                    <Badge variant="secondary" className="text-xs">
                        已禁用
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
        const plan = backupPlans.find((p) => p.id.toString() === selectedPlan);
        if (plan) {
            toast.success(`已启动备份计划: ${plan.name}`);
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
                            <p className="text-muted-foreground">{t.dashboard.subtitle}</p>
                        </div>
                    </div>
                )}
                <LoadingPage status={loadingText} />
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
                        <div className="flex gap-3">
                            <Button
                                variant="outline"
                                className="gap-2"
                                onClick={() => onNavigate?.("create-plan")}
                            >
                                <Plus className="w-4 h-4" />
                                {t.dashboard.createNewPlan}
                            </Button>
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
                                            onValueChange={setSelectedPlan}
                                        >
                                            <SelectTrigger>
                                                <SelectValue placeholder="选择备份计划" />
                                            </SelectTrigger>
                                            <SelectContent>
                                                {backupPlans
                                                    .filter(
                                                        (plan) => plan.enabled
                                                    )
                                                    .map((plan) => (
                                                        <SelectItem
                                                            key={plan.id}
                                                            value={plan.id.toString()}
                                                        >
                                                            {plan.name}
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
                        </div>
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
                                2
                            </div>
                            <p className="text-xs text-muted-foreground">
                                1个执行中
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
                                1.2TB
                            </div>
                            <p className="text-xs text-muted-foreground">
                                +180GB 今日
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
                                8
                            </div>
                            <p className="text-xs text-muted-foreground">
                                6个已启用
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
                                98.5%
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
                            {isMobile ? (
                                <ScrollArea className="h-20">
                                    <div className="space-y-2">
                                        {currentTasks
                                            .filter(
                                                (task) =>
                                                    task.status === "running"
                                            )
                                            .map((task) => (
                                                <div
                                                    key={task.id}
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
                                                                task.status
                                                            )}
                                                        </div>
                                                        <div className="space-y-1">
                                                            <Progress
                                                                value={
                                                                    task.progress
                                                                }
                                                                className="h-1"
                                                            />
                                                            <div className="flex justify-between text-xs text-muted-foreground">
                                                                <span>
                                                                    {
                                                                        task.progress
                                                                    }
                                                                    %
                                                                </span>
                                                                <span>
                                                                    {
                                                                        task.remaining
                                                                    }
                                                                </span>
                                                            </div>
                                                        </div>
                                                    </div>
                                                </div>
                                            ))}
                                        {currentTasks.filter(
                                            (task) => task.status === "running"
                                        ).length === 0 && (
                                            <div className="text-center py-4">
                                                {plansCount === 0 ? (
                                                    <div
                                                        className={`flex ${
                                                            isMobile
                                                                ? "flex-col gap-2"
                                                                : "items-center gap-3"
                                                        } justify-center`}
                                                    >
                                                        {servicesCount === 0 ? (
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
                                        )}
                                    </div>
                                </ScrollArea>
                            ) : (
                                <div className="space-y-3">
                                    {currentTasks.filter(
                                        (t) => t.status === "running"
                                    ).length === 0 && (
                                        <div className="text-center py-2">
                                            {plansCount === 0 ? (
                                                <div
                                                    className={`flex ${
                                                        isMobile
                                                            ? "flex-col gap-2"
                                                            : "items-center gap-3"
                                                    } justify-center`}
                                                >
                                                    {servicesCount === 0 ? (
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
                                    )}
                                    {currentTasks.slice(0, 3).map((task) => (
                                        <div
                                            key={task.id}
                                            className="flex items-center gap-3"
                                        >
                                            <div className="flex-1 min-w-0">
                                                <div className="flex items-center gap-2 mb-1">
                                                    <span className="font-medium truncate">
                                                        {task.name}
                                                    </span>
                                                    {getStatusBadge(
                                                        task.status
                                                    )}
                                                </div>
                                                {task.status === "running" && (
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
                                                                • {task.speed}
                                                            </span>
                                                            <span>
                                                                {task.remaining}
                                                            </span>
                                                        </div>
                                                    </div>
                                                )}
                                                {task.status ===
                                                    "completed" && (
                                                    <div className="text-xs text-muted-foreground">
                                                        已完成 • {task.plan}
                                                    </div>
                                                )}
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
                            {servicesCount === 0 ? (
                                <div
                                    className={`flex ${
                                        isMobile
                                            ? "flex-col gap-2"
                                            : "items-center gap-3"
                                    }`}
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
                                        {backupServices
                                            .slice(0, 3)
                                            .map((service) => {
                                                const ServiceIcon =
                                                    getServiceIcon(
                                                        service.type
                                                    );
                                                return (
                                                    <div
                                                        key={service.id}
                                                        className="flex items-center gap-2 text-sm cursor-pointer"
                                                        onClick={() =>
                                                            onNavigate?.(
                                                                "services"
                                                            )
                                                        }
                                                    >
                                                        <ServiceIcon className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                                                        <div className="flex-1 min-w-0">
                                                            <div className="flex items-center gap-2 mb-1">
                                                                <span className="font-medium truncate">
                                                                    {
                                                                        service.name
                                                                    }
                                                                </span>
                                                                {getServiceStatusBadge(
                                                                    service.status
                                                                )}
                                                            </div>
                                                            <div className="flex items-center gap-2 text-xs text-muted-foreground">
                                                                <span>
                                                                    {
                                                                        service.used
                                                                    }
                                                                </span>
                                                                {service.usagePercent >
                                                                    0 && (
                                                                    <>
                                                                        <span>
                                                                            •
                                                                        </span>
                                                                        <div className="flex items-center gap-1">
                                                                            <div className="w-8 bg-secondary rounded-full h-1">
                                                                                <div
                                                                                    className={`h-1 rounded-full ${
                                                                                        service.usagePercent >
                                                                                        90
                                                                                            ? "bg-red-500"
                                                                                            : service.usagePercent >
                                                                                              70
                                                                                            ? "bg-yellow-500"
                                                                                            : "bg-green-500"
                                                                                    }`}
                                                                                    style={{
                                                                                        width: `${service.usagePercent}%`,
                                                                                    }}
                                                                                />
                                                                            </div>
                                                                            <span>
                                                                                {
                                                                                    service.usagePercent
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
                                <div className="space-y-3">
                                    {backupServices
                                        .slice(0, 4)
                                        .map((service) => {
                                            const ServiceIcon = getServiceIcon(
                                                service.type
                                            );
                                            return (
                                                <div
                                                    key={service.id}
                                                    className="flex items-center gap-3"
                                                >
                                                    <ServiceIcon className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                                                    <div className="flex-1 min-w-0">
                                                        <div className="flex items-center gap-2 mb-1">
                                                            <span className="font-medium truncate">
                                                                {service.name}
                                                            </span>
                                                            {getServiceStatusBadge(
                                                                service.status
                                                            )}
                                                        </div>
                                                        <div className="flex items-center gap-2 text-xs text-muted-foreground">
                                                            <span>
                                                                {service.used} /{" "}
                                                                {service.total}
                                                            </span>
                                                            {service.usagePercent >
                                                                0 && (
                                                                <>
                                                                    <span>
                                                                        •
                                                                    </span>
                                                                    <div className="flex items-center gap-1">
                                                                        <div className="w-12 bg-secondary rounded-full h-1">
                                                                            <div
                                                                                className={`h-1 rounded-full ${
                                                                                    service.usagePercent >
                                                                                    90
                                                                                        ? "bg-red-500"
                                                                                        : service.usagePercent >
                                                                                          70
                                                                                        ? "bg-yellow-500"
                                                                                        : "bg-green-500"
                                                                                }`}
                                                                                style={{
                                                                                    width: `${service.usagePercent}%`,
                                                                                }}
                                                                            />
                                                                        </div>
                                                                        <span>
                                                                            {
                                                                                service.usagePercent
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
                            {plansCount === 0 ? (
                                <div
                                    className={`flex ${
                                        isMobile
                                            ? "flex-col gap-2"
                                            : "items-center gap-3"
                                    }`}
                                >
                                    {servicesCount === 0 ? (
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
                            ) : isMobile ? (
                                <ScrollArea className="h-24">
                                    <div className="space-y-2">
                                        {backupPlans.slice(0, 4).map((plan) => (
                                            <div
                                                key={plan.id}
                                                className="flex items-center justify-between text-sm cursor-pointer"
                                                onClick={() =>
                                                    onNavigate?.("plans")
                                                }
                                            >
                                                <div className="flex items-center gap-2 flex-1 min-w-0">
                                                    <div className="flex-1">
                                                        <div className="flex items-center gap-2 mb-1">
                                                            <span className="font-medium truncate">
                                                                {plan.name}
                                                            </span>
                                                            {getPlanStatusBadge(
                                                                plan.status
                                                            )}
                                                        </div>
                                                        <div className="text-xs text-muted-foreground">
                                                            {plan.nextRun}
                                                        </div>
                                                    </div>
                                                </div>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    className="p-1 h-6 w-6 flex-shrink-0"
                                                    disabled={!plan.enabled}
                                                    onClick={(e) => {
                                                        e.stopPropagation();
                                                        toast.success(
                                                            `正在启动计划: ${plan.name}`
                                                        );
                                                    }}
                                                >
                                                    <Play className="w-3 h-3" />
                                                </Button>
                                            </div>
                                        ))}
                                    </div>
                                </ScrollArea>
                            ) : (
                                <div className="space-y-3">
                                    {backupPlans.slice(0, 4).map((plan) => (
                                        <div
                                            key={plan.id}
                                            className="flex items-center justify-between"
                                        >
                                            <div className="flex items-center gap-3 flex-1 min-w-0">
                                                <div className="flex-1">
                                                    <div className="flex items-center gap-2 mb-1">
                                                        <span className="font-medium truncate">
                                                            {plan.name}
                                                        </span>
                                                        {getPlanStatusBadge(
                                                            plan.status
                                                        )}
                                                    </div>
                                                    <div className="text-xs text-muted-foreground">
                                                        下次执行: {plan.nextRun}
                                                    </div>
                                                </div>
                                            </div>
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                className="gap-1 text-xs flex-shrink-0"
                                                disabled={!plan.enabled}
                                                onClick={() =>
                                                    toast.success(
                                                        `正在启动计划: ${plan.name}`
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
                                {recentActivities.map((activity) => (
                                    <div
                                        key={activity.id}
                                        className={`flex items-center justify-between ${
                                            isMobile ? "text-sm" : ""
                                        }`}
                                    >
                                        <div className="flex items-center gap-3 flex-1 min-w-0">
                                            {getStatusIcon(activity.status)}
                                            <div className="flex-1 min-w-0">
                                                <p className="font-medium truncate">
                                                    {activity.name}
                                                </p>
                                                <p className="text-xs text-muted-foreground">
                                                    {activity.time} •{" "}
                                                    {activity.size}
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
                                            {backupPlans
                                                .filter((plan) => plan.enabled)
                                                .map((plan) => (
                                                    <SelectItem
                                                        key={plan.id}
                                                        value={plan.id.toString()}
                                                    >
                                                        {plan.name}
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
