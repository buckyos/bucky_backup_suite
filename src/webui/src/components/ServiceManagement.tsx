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
import { Input } from "./ui/input";
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
    Edit,
    Trash2,
    HardDrive,
    Network,
    CheckCircle,
    AlertCircle,
    Settings,
    Check,
    X,
} from "lucide-react";
import { taskManager } from "./utils/fake_task_mgr";
import { BackupTargetInfo, TargetState, TargetType } from "./utils/task_mgr";
import { TaskMgrHelper } from "./utils/task_mgr_helper";

interface ServiceManagementProps {
    onNavigate?: (page: string, data?: any) => void;
}

export function ServiceManagement({ onNavigate }: ServiceManagementProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [loading, setLoading] = useState(true);
    // Move all hooks before any conditional returns to keep stable order
    const [services, setServices] = useState<BackupTargetInfo[]>([]);
    const [editingService, setEditingService] =
        useState<BackupTargetInfo | null>(null);
    const [editingName, setEditingName] = useState("");

    useEffect(() => {
        // load services from backend
        taskManager.listBackupTargets().then(async (targetIds) => {
            const targets = await Promise.all(
                targetIds.map((id) => taskManager.getBackupTarget(id))
            );
            setServices(targets);
            setLoading(false);
        });
        const timerId = taskManager.startRefreshTargetStateTimer();
        return () => {
            taskManager.stopRefreshTargetStateTimer(timerId);
        };
    }, []);

    if (loading) {
        return (
            <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-6`}>
                <div>
                    <h1 className="mb-2">{t.services.title}</h1>
                    <p className="text-muted-foreground">
                        {t.services.subtitle}
                    </p>
                </div>
                <LoadingPage
                    status={`${t.common.loading} ${t.nav.services}...`}
                />
            </div>
        );
    }

    const getStatusBadge = (state: TargetState) => {
        switch (state) {
            case TargetState.ONLINE:
                return (
                    <Badge className="bg-green-100 text-green-800 gap-1">
                        <CheckCircle className="w-3 h-3" />
                        正常
                    </Badge>
                );
            case TargetState.ERROR:
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 gap-1">
                        <AlertCircle className="w-3 h-3" />
                        警告
                    </Badge>
                );
            case TargetState.OFFLINE:
                return (
                    <Badge variant="destructive" className="gap-1">
                        <AlertCircle className="w-3 h-3" />
                        离线
                    </Badge>
                );
            default:
                return <Badge variant="outline">未知</Badge>;
        }
    };

    const getServiceIcon = (type: TargetType) => {
        return type === TargetType.LOCAL ? HardDrive : Network;
    };

    const handleEdit = (serviceInfo: BackupTargetInfo, currentName: string) => {
        setEditingService(serviceInfo);
        setEditingName(currentName);
    };

    const handleSaveEdit = async (serviceInfo: BackupTargetInfo) => {
        const oldName = serviceInfo.name;
        serviceInfo.name = editingName;
        const success = await taskManager.updateBackupTarget(serviceInfo);
        if (success) {
            setServices(
                services.map((service) =>
                    service.target_id === serviceInfo.target_id
                        ? serviceInfo
                        : service
                )
            );
            toast.success("服务名称已更新");
        } else {
            serviceInfo.name = oldName; // revert
            toast.error("更新服务名称失败");
        }
        setEditingService(null);
        setEditingName("");
    };

    const handleCancelEdit = () => {
        setEditingService(null);
        setEditingName("");
    };

    const handleDelete = async (serviceId: string) => {
        const success = await taskManager.removeBackupTarget(serviceId);
        if (!success) {
            toast.error("删除服务失败");
            return;
        }
        setServices(
            services.filter((service) => service.target_id !== serviceId)
        );
        toast.success("服务已删除");
    };

    // const handleTestConnection = (service: any) => {
    //     toast.success(`正在测试 ${service.name} 的连接...`);
    // };

    return (
        <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-6`}>
            <div className="flex items-center justify-between">
                <div>
                    {!isMobile && (
                        <>
                            <h1 className="mb-2">服务管理</h1>
                            <p className="text-muted-foreground">
                                配置和管理备份目标位置
                            </p>
                        </>
                    )}
                </div>
                <Button
                    className={`gap-2 ${isMobile ? "px-3" : ""}`}
                    onClick={() => onNavigate?.("add-service")}
                >
                    <Plus className="w-4 h-4" />
                    {isMobile ? "" : "添加服务"}
                </Button>
            </div>

            {/* 服务列表 */}
            <div className="grid gap-4">
                {services.length === 0 ? (
                    <Card
                        className={`w-full ${
                            isMobile ? "" : "max-w-2xl"
                        } text-center m-auto`}
                    >
                        <CardHeader>
                            <CardTitle>还没有可用的备份服务</CardTitle>
                            <CardDescription>
                                创建备份计划前，请先配置一个备份服务
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
                                <Button
                                    onClick={() => onNavigate?.("add-service")}
                                    className="gap-2"
                                >
                                    <Plus className="w-4 h-4" />
                                    去配置备份服务
                                </Button>
                            </div>
                        </CardContent>
                    </Card>
                ) : (
                    services.map((service) => {
                        const ServiceIcon = getServiceIcon(service.target_type);
                        const usagePercent =
                            TaskMgrHelper.targetUsagePercent(service);

                        return (
                            <Card key={service.target_id}>
                                <CardHeader>
                                    <div className="flex items-start justify-between">
                                        <div className="flex items-center gap-3">
                                            <ServiceIcon className="w-5 h-5 text-muted-foreground" />
                                            <div className="flex-1">
                                                {editingService === service ? (
                                                    <div className="flex items-center gap-2">
                                                        <Input
                                                            value={editingName}
                                                            onChange={(e) =>
                                                                setEditingName(
                                                                    e.target
                                                                        .value
                                                                )
                                                            }
                                                            className="h-8"
                                                            onKeyDown={(e) => {
                                                                if (
                                                                    e.key ===
                                                                    "Enter"
                                                                ) {
                                                                    handleSaveEdit(
                                                                        service
                                                                    );
                                                                } else if (
                                                                    e.key ===
                                                                    "Escape"
                                                                ) {
                                                                    handleCancelEdit();
                                                                }
                                                            }}
                                                        />
                                                        <Button
                                                            size="sm"
                                                            variant="outline"
                                                            className="h-8 w-8 p-0"
                                                            onClick={() =>
                                                                handleSaveEdit(
                                                                    service
                                                                )
                                                            }
                                                        >
                                                            <Check className="w-3 h-3" />
                                                        </Button>
                                                        <Button
                                                            size="sm"
                                                            variant="outline"
                                                            className="h-8 w-8 p-0"
                                                            onClick={
                                                                handleCancelEdit
                                                            }
                                                        >
                                                            <X className="w-3 h-3" />
                                                        </Button>
                                                    </div>
                                                ) : (
                                                    <div className="flex items-center gap-2">
                                                        <CardTitle
                                                            className={`${
                                                                isMobile
                                                                    ? "text-base"
                                                                    : "text-lg"
                                                            }`}
                                                        >
                                                            {service.name}
                                                        </CardTitle>
                                                        <Button
                                                            size="sm"
                                                            variant="ghost"
                                                            className="h-6 w-6 p-0"
                                                            onClick={() =>
                                                                handleEdit(
                                                                    service,
                                                                    service.name
                                                                )
                                                            }
                                                        >
                                                            <Edit className="w-3 h-3" />
                                                        </Button>
                                                    </div>
                                                )}
                                                <CardDescription>
                                                    {service.url}
                                                </CardDescription>
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            {getStatusBadge(service.state)}
                                        </div>
                                    </div>
                                </CardHeader>
                                <CardContent>
                                    <div
                                        className={`grid ${
                                            isMobile
                                                ? "grid-cols-1"
                                                : "grid-cols-1 lg:grid-cols-4"
                                        } gap-4 mb-4`}
                                    >
                                        <div>
                                            <p className="text-sm text-muted-foreground mb-1">
                                                存储使用
                                            </p>
                                            <p className="font-medium">
                                                {service.used} / {service.total}
                                            </p>
                                            {service.total >= 0 && (
                                                <div className="mt-2">
                                                    <div className="w-full bg-secondary rounded-full h-2">
                                                        <div
                                                            className={`h-2 rounded-full ${
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
                                                    <p className="text-xs text-muted-foreground mt-1">
                                                        {usagePercent}% 已使用
                                                    </p>
                                                </div>
                                            )}
                                        </div>
                                        <div>
                                            <p className="text-sm text-muted-foreground mb-1">
                                                类型
                                            </p>
                                            <p className="font-medium">
                                                {service.target_type ===
                                                TargetType.LOCAL
                                                    ? "本地目录"
                                                    : "NDN网络"}
                                            </p>
                                        </div>
                                    </div>

                                    <div className="flex items-center justify-end gap-2 pt-4 border-t">
                                        {isMobile ? (
                                            <>
                                                {/* {service.type !== "local" && (
                                                    <Button
                                                        variant="outline"
                                                        size="sm"
                                                        className="p-2"
                                                        onClick={() =>
                                                            handleTestConnection(
                                                                service
                                                            )
                                                        }
                                                    >
                                                        <Settings className="w-3 h-3" />
                                                    </Button>
                                                )} */}
                                                <AlertDialog>
                                                    <AlertDialogTrigger asChild>
                                                        <Button
                                                            variant="outline"
                                                            size="sm"
                                                            className="p-2 text-destructive hover:text-destructive-foreground hover:bg-destructive"
                                                        >
                                                            <Trash2 className="w-3 h-3" />
                                                        </Button>
                                                    </AlertDialogTrigger>
                                                    <AlertDialogContent>
                                                        <AlertDialogHeader>
                                                            <AlertDialogTitle>
                                                                删除备份服务
                                                            </AlertDialogTitle>
                                                            <AlertDialogDescription>
                                                                确定要删除服务 "
                                                                {service.name}"
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
                                                                    handleDelete(
                                                                        service.target_id
                                                                    )
                                                                }
                                                            >
                                                                删除
                                                            </AlertDialogAction>
                                                        </AlertDialogFooter>
                                                    </AlertDialogContent>
                                                </AlertDialog>
                                            </>
                                        ) : (
                                            <>
                                                {/* {service.type !== "local" && (
                                                    <Button
                                                        variant="outline"
                                                        size="sm"
                                                        className="gap-1"
                                                        onClick={() =>
                                                            handleTestConnection(
                                                                service
                                                            )
                                                        }
                                                    >
                                                        <Settings className="w-3 h-3" />
                                                        测试连接
                                                    </Button>
                                                )} */}
                                                <AlertDialog>
                                                    <AlertDialogTrigger asChild>
                                                        <Button
                                                            variant="outline"
                                                            size="sm"
                                                            className="gap-1 text-destructive hover:text-destructive-foreground hover:bg-destructive"
                                                        >
                                                            <Trash2 className="w-3 h-3" />
                                                            删除
                                                        </Button>
                                                    </AlertDialogTrigger>
                                                    <AlertDialogContent>
                                                        <AlertDialogHeader>
                                                            <AlertDialogTitle>
                                                                删除备份服务
                                                            </AlertDialogTitle>
                                                            <AlertDialogDescription>
                                                                确定要删除服务 "
                                                                {service.name}"
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
                                                                    handleDelete(
                                                                        service.target_id
                                                                    )
                                                                }
                                                            >
                                                                删除
                                                            </AlertDialogAction>
                                                        </AlertDialogFooter>
                                                    </AlertDialogContent>
                                                </AlertDialog>
                                            </>
                                        )}
                                    </div>
                                </CardContent>
                            </Card>
                        );
                    })
                )}
            </div>

            {/* 统计信息 */}
            {!isMobile && (
                <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-base">
                                总服务数
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <p className="text-2xl">{services.length}</p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-base">
                                在线服务
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <p className="text-2xl">
                                {
                                    services.filter(
                                        (s) => s.state !== TargetState.OFFLINE
                                    ).length
                                }
                            </p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-base">
                                本地服务
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <p className="text-2xl">
                                {
                                    services.filter(
                                        (s) =>
                                            s.target_type === TargetType.LOCAL
                                    ).length
                                }
                            </p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader className="pb-3">
                            <CardTitle className="text-base">
                                网络服务
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <p className="text-2xl">
                                {
                                    services.filter(
                                        (s) => s.target_type === TargetType.NDN
                                    ).length
                                }
                            </p>
                        </CardContent>
                    </Card>
                </div>
            )}
        </div>
    );
}
