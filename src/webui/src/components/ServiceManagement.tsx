import React, { useState } from "react";
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
import { toast } from "sonner@2.0.3";
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

interface ServiceManagementProps {
    onNavigate?: (page: string, data?: any) => void;
}

export function ServiceManagement({ onNavigate }: ServiceManagementProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [services, setServices] = useState([
        {
            id: 1,
            name: "本地备份盘",
            type: "local",
            path: "D:\\Backups",
            status: "healthy",
            used: "450 GB",
            total: "2 TB",
            compression: true,
            encryption: true,
        },
        {
            id: 2,
            name: "NDN网络节点1",
            type: "ndn",
            endpoint: "ndn://backup.example.com",
            status: "healthy",
            used: "1.2 TB",
            total: "无限制",
            compression: true,
            encryption: true,
        },
        {
            id: 3,
            name: "外部硬盘",
            type: "local",
            path: "E:\\Backups",
            status: "warning",
            used: "1.8 TB",
            total: "2 TB",
            compression: false,
            encryption: true,
        },
        {
            id: 4,
            name: "NDN网络节点2",
            type: "ndn",
            endpoint: "ndn://backup2.example.com",
            status: "offline",
            used: "0 GB",
            total: "无限制",
            compression: true,
            encryption: false,
        },
    ]);

    const [editingService, setEditingService] = useState<number | null>(null);
    const [editingName, setEditingName] = useState("");

    const getStatusBadge = (status: string) => {
        switch (status) {
            case "healthy":
                return (
                    <Badge className="bg-green-100 text-green-800 gap-1">
                        <CheckCircle className="w-3 h-3" />
                        正常
                    </Badge>
                );
            case "warning":
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 gap-1">
                        <AlertCircle className="w-3 h-3" />
                        警告
                    </Badge>
                );
            case "offline":
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

    const getServiceIcon = (type: string) => {
        return type === "local" ? HardDrive : Network;
    };

    const getUsagePercentage = (used: string, total: string) => {
        if (total === "无限制") return 0;
        const usedNum = parseFloat(used.replace(/[^\d.]/g, ""));
        const totalNum = parseFloat(total.replace(/[^\d.]/g, ""));
        return Math.round((usedNum / totalNum) * 100);
    };

    const handleEdit = (serviceId: number, currentName: string) => {
        setEditingService(serviceId);
        setEditingName(currentName);
    };

    const handleSaveEdit = (serviceId: number) => {
        setServices(
            services.map((service) =>
                service.id === serviceId
                    ? { ...service, name: editingName }
                    : service
            )
        );
        setEditingService(null);
        setEditingName("");
        toast.success("服务名称已更新");
    };

    const handleCancelEdit = () => {
        setEditingService(null);
        setEditingName("");
    };

    const handleDelete = (serviceId: number) => {
        setServices(services.filter((service) => service.id !== serviceId));
        toast.success("服务已删除");
    };

    const handleTestConnection = (service: any) => {
        toast.success(`正在测试 ${service.name} 的连接...`);
    };

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
                {services.map((service) => {
                    const ServiceIcon = getServiceIcon(service.type);
                    const usagePercent = getUsagePercentage(
                        service.used,
                        service.total
                    );

                    return (
                        <Card key={service.id}>
                            <CardHeader>
                                <div className="flex items-start justify-between">
                                    <div className="flex items-center gap-3">
                                        <ServiceIcon className="w-5 h-5 text-muted-foreground" />
                                        <div className="flex-1">
                                            {editingService === service.id ? (
                                                <div className="flex items-center gap-2">
                                                    <Input
                                                        value={editingName}
                                                        onChange={(e) =>
                                                            setEditingName(
                                                                e.target.value
                                                            )
                                                        }
                                                        className="h-8"
                                                        onKeyDown={(e) => {
                                                            if (
                                                                e.key ===
                                                                "Enter"
                                                            ) {
                                                                handleSaveEdit(
                                                                    service.id
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
                                                                service.id
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
                                                                service.id,
                                                                service.name
                                                            )
                                                        }
                                                    >
                                                        <Edit className="w-3 h-3" />
                                                    </Button>
                                                </div>
                                            )}
                                            <CardDescription>
                                                {service.type === "local"
                                                    ? service.path
                                                    : service.endpoint}
                                            </CardDescription>
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-2">
                                        {getStatusBadge(service.status)}
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
                                        {service.total !== "无限制" && (
                                            <div className="mt-2">
                                                <div className="w-full bg-secondary rounded-full h-2">
                                                    <div
                                                        className={`h-2 rounded-full ${
                                                            usagePercent > 90
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
                                            {service.type === "local"
                                                ? "本地目录"
                                                : "NDN网络"}
                                        </p>
                                    </div>
                                    <div>
                                        <p className="text-sm text-muted-foreground mb-1">
                                            压缩
                                        </p>
                                        <p className="font-medium">
                                            {service.compression
                                                ? "已启用"
                                                : "已禁用"}
                                        </p>
                                    </div>
                                    <div>
                                        <p className="text-sm text-muted-foreground mb-1">
                                            加密
                                        </p>
                                        <p className="font-medium">
                                            {service.encryption
                                                ? "已启用"
                                                : "已禁用"}
                                        </p>
                                    </div>
                                </div>

                                <div className="flex items-center justify-end gap-2 pt-4 border-t">
                                    {isMobile ? (
                                        <>
                                            {service.type !== "local" && (
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
                                            )}
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
                                                                    service.id
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
                                            {service.type !== "local" && (
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
                                            )}
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
                                                                    service.id
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
                })}
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
                                        (s) => s.status !== "offline"
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
                                    services.filter((s) => s.type === "local")
                                        .length
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
                                    services.filter((s) => s.type === "ndn")
                                        .length
                                }
                            </p>
                        </CardContent>
                    </Card>
                </div>
            )}
        </div>
    );
}
