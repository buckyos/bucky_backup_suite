import React, { useEffect, useState } from "react";
import { Card, CardContent } from "./ui/card";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "./ui/select";
import { Switch } from "./ui/switch";
import {
    Breadcrumb,
    BreadcrumbItem,
    BreadcrumbLink,
    BreadcrumbList,
    BreadcrumbPage,
    BreadcrumbSeparator,
} from "./ui/breadcrumb";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { toast } from "sonner";
import {
    ArrowLeft,
    ChevronLeft,
    ChevronRight,
    Check,
    Folder,
    HardDrive,
    Loader2,
    Network,
} from "lucide-react";
import { DirectoryNode, DirectoryPurpose, TargetType } from "./utils/task_mgr";
import { taskManager } from "./utils/task_mgr_helper";

interface BreadcrumbNode {
    label: string;
    path: string | null;
}

const ROOT_DIRECTORY_BREADCRUMB: BreadcrumbNode = {
    label: "根目录",
    path: null,
};

function joinDirectoryPath(base: string | null, segment: string): string {
    if (!base || base.length === 0) {
        return segment;
    }
    if (base.endsWith("/") || base.endsWith("\\")) {
        return `${base}${segment}`;
    }
    return `${base}/${segment}`;
}

interface AddServiceWizardProps {
    onBack: () => void;
    onComplete: () => void;
}

export function AddServiceWizard({
    onBack,
    onComplete,
}: AddServiceWizardProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [currentStep, setCurrentStep] = useState(1);

    // 表单数据
    const [serviceType, setServiceType] = useState(TargetType.LOCAL);
    const [serviceName, setServiceName] = useState("");
    const [localPath, setLocalPath] = useState("");
    const [ndnUrl, setNdnUrl] = useState("");

    const [directoryBreadcrumbs, setDirectoryBreadcrumbs] = useState<
        BreadcrumbNode[]
    >([ROOT_DIRECTORY_BREADCRUMB]);
    const [directoryEntries, setDirectoryEntries] = useState<DirectoryNode[]>(
        []
    );
    const [directoriesLoading, setDirectoriesLoading] = useState(false);
    const [directoriesError, setDirectoriesError] = useState<string | null>(
        null
    );
    const [directoryRequestId, setDirectoryRequestId] = useState(0);

    const currentDirectoryBreadcrumb =
        directoryBreadcrumbs[directoryBreadcrumbs.length - 1];
    const currentDirectoryPath = currentDirectoryBreadcrumb?.path ?? null;

    const reloadCurrentDirectory = () =>
        setDirectoryRequestId((prev) => prev + 1);

    const handleBreadcrumbNavigate = (index: number) => {
        setDirectoryBreadcrumbs((prev) => prev.slice(0, index + 1));
    };

    const handleDirectorySelect = (dirName: string) => {
        const nextPath = joinDirectoryPath(currentDirectoryPath, dirName);
        setDirectoryBreadcrumbs((prev) => {
            const last = prev[prev.length - 1];
            if (last?.path === nextPath) {
                return prev;
            }
            return [...prev, { label: dirName, path: nextPath }];
        });
    };

    useEffect(() => {
        if (serviceType !== TargetType.LOCAL) {
            return;
        }

        let cancelled = false;

        const fetchDirectories = async () => {
            setDirectoriesLoading(true);
            setDirectoriesError(null);
            try {
                const result = await taskManager.listDirChildren(
                    DirectoryPurpose.BACKUP_TARGET,
                    currentDirectoryPath ?? undefined,
                    { only_dirs: true }
                );
                if (!cancelled) {
                    setDirectoryEntries(result);
                }
            } catch (error) {
                if (!cancelled) {
                    setDirectoryEntries([]);
                    setDirectoriesError(
                        error instanceof Error ? error.message : String(error)
                    );
                }
            } finally {
                if (!cancelled) {
                    setDirectoriesLoading(false);
                }
            }
        };

        fetchDirectories();

        return () => {
            cancelled = true;
        };
    }, [currentDirectoryPath, serviceType, directoryRequestId]);

    useEffect(() => {
        if (serviceType !== TargetType.LOCAL) {
            return;
        }
        const nextPath = currentDirectoryPath ?? "";
        setLocalPath((prev) => (prev === nextPath ? prev : nextPath));
    }, [currentDirectoryPath, serviceType]);

    const steps = [
        { number: 1, title: "服务类型", description: "选择服务类型" },
        { number: 2, title: "基本配置", description: "配置服务信息" },
        { number: 3, title: "确认添加", description: "确认服务配置" },
    ];

    const handleNext = () => {
        if (currentStep < 3) {
            setCurrentStep(currentStep + 1);
        }
    };

    const handlePrevious = () => {
        if (currentStep > 1) {
            setCurrentStep(currentStep - 1);
        }
    };

    const handleComplete = () => {
        let url = "";
        switch (serviceType) {
            case TargetType.LOCAL:
                url = localPath;
                break;
            case TargetType.NDN:
                url = ndnUrl;
        }
        taskManager.createBackupTarget(serviceType, url, serviceName, {});
        toast.success("服务已添加");
        onComplete();
    };

    const canProceed = () => {
        switch (currentStep) {
            case 1:
                return true;
            case 2:
                if (serviceType === TargetType.LOCAL) {
                    return serviceName.trim() !== "" && localPath.trim() !== "";
                } else if (serviceType === TargetType.NDN) {
                    return serviceName.trim() !== "" && ndnUrl.trim() !== "";
                }
                return false;
            case 3:
                return true;
            default:
                return false;
        }
    };

    const renderStepContent = () => {
        switch (currentStep) {
            case 1:
                return (
                    <div className="space-y-6">
                        <div>
                            <Label className="text-base">选择服务类型</Label>
                            <p className="text-sm text-muted-foreground mb-4">
                                选择要添加的备份服务类型
                            </p>
                        </div>

                        <div className="grid gap-4">
                            <Card
                                className={`cursor-pointer transition-all border-2 ${
                                    serviceType === TargetType.LOCAL
                                        ? "border-primary bg-primary/5"
                                        : "border-border hover:border-primary/50"
                                }`}
                                onClick={() => setServiceType(TargetType.LOCAL)}
                            >
                                <CardContent className="p-6">
                                    <div className="flex items-center gap-4">
                                        <div className="p-3 bg-blue-100 rounded-lg">
                                            <HardDrive className="w-6 h-6 text-blue-600" />
                                        </div>
                                        <div>
                                            <h3 className="font-medium text-lg">
                                                本地存储
                                            </h3>
                                            <p className="text-sm text-muted-foreground">
                                                本地硬盘或外部存储设备
                                            </p>
                                        </div>
                                    </div>
                                </CardContent>
                            </Card>

                            <Card
                                className={`cursor-pointer transition-all border-2 ${
                                    serviceType === TargetType.NDN
                                        ? "border-primary bg-primary/5"
                                        : "border-border hover:border-primary/50"
                                }`}
                                onClick={() => setServiceType(TargetType.NDN)}
                            >
                                <CardContent className="p-6">
                                    <div className="flex items-center gap-4">
                                        <div className="p-3 bg-green-100 rounded-lg">
                                            <Network className="w-6 h-6 text-green-600" />
                                        </div>
                                        <div>
                                            <h3 className="font-medium text-lg">
                                                NDN存储
                                            </h3>
                                            <p className="text-sm text-muted-foreground">
                                                基于命名数据网络的存储服务
                                            </p>
                                        </div>
                                    </div>
                                </CardContent>
                            </Card>
                        </div>
                    </div>
                );

            case 2:
                return (
                    <div className="space-y-6">
                        <div>
                            <Label className="text-base">基本配置</Label>
                            <p className="text-sm text-muted-foreground mb-4">
                                配置服务的基本信息
                            </p>
                        </div>

                        <div className="space-y-4">
                            <div className="space-y-2">
                                <Label htmlFor="serviceName">服务名称 *</Label>
                                <Input
                                    id="serviceName"
                                    value={serviceName}
                                    onChange={(e) =>
                                        setServiceName(e.target.value)
                                    }
                                    placeholder="输入服务名称"
                                />
                            </div>

                            {serviceType === TargetType.LOCAL && (
                                <div className="space-y-3">
                                    <Label>目标路径 *</Label>
                                    <div className="space-y-2">
                                        <Breadcrumb>
                                            <BreadcrumbList>
                                                {directoryBreadcrumbs.map(
                                                    (crumb, index) => (
                                                        <React.Fragment
                                                            key={`${crumb.label}-${index}`}
                                                        >
                                                            <BreadcrumbItem>
                                                                {index ===
                                                                directoryBreadcrumbs.length -
                                                                    1 ? (
                                                                    <BreadcrumbPage>
                                                                        {
                                                                            crumb.label
                                                                        }
                                                                    </BreadcrumbPage>
                                                                ) : (
                                                                    <BreadcrumbLink
                                                                        className="cursor-pointer"
                                                                        onClick={() =>
                                                                            handleBreadcrumbNavigate(
                                                                                index
                                                                            )
                                                                        }
                                                                    >
                                                                        {
                                                                            crumb.label
                                                                        }
                                                                    </BreadcrumbLink>
                                                                )}
                                                            </BreadcrumbItem>
                                                            {index <
                                                                directoryBreadcrumbs.length -
                                                                    1 && (
                                                                <BreadcrumbSeparator />
                                                            )}
                                                        </React.Fragment>
                                                    )
                                                )}
                                            </BreadcrumbList>
                                        </Breadcrumb>

                                        <div className="rounded-md border">
                                            {directoriesLoading ? (
                                                <div className="flex h-24 items-center justify-center gap-2 text-sm text-muted-foreground">
                                                    <Loader2 className="h-4 w-4 animate-spin" />
                                                    加载中...
                                                </div>
                                            ) : directoriesError ? (
                                                <div className="flex h-24 flex-col items-center justify-center gap-2 p-4 text-center text-sm text-destructive">
                                                    <span>无法加载目录</span>
                                                    <Button
                                                        type="button"
                                                        size="sm"
                                                        variant="outline"
                                                        onClick={
                                                            reloadCurrentDirectory
                                                        }
                                                    >
                                                        重试
                                                    </Button>
                                                </div>
                                            ) : directoryEntries.length ===
                                              0 ? (
                                                <div className="flex h-24 items-center justify-center px-4 text-sm text-muted-foreground">
                                                    当前目录下没有可用子目录
                                                </div>
                                            ) : (
                                                <div className="divide-y">
                                                    {directoryEntries.map(
                                                        (entry) => (
                                                            <button
                                                                key={joinDirectoryPath(
                                                                    currentDirectoryPath,
                                                                    entry.name
                                                                )}
                                                                type="button"
                                                                className="flex w-full items-center justify-between px-3 py-2 text-left transition-colors hover:bg-accent"
                                                                onClick={() =>
                                                                    handleDirectorySelect(
                                                                        entry.name
                                                                    )
                                                                }
                                                            >
                                                                <span className="flex items-center gap-2">
                                                                    <Folder className="h-4 w-4 text-muted-foreground" />
                                                                    <span className="truncate text-sm">
                                                                        {
                                                                            entry.name
                                                                        }
                                                                    </span>
                                                                </span>
                                                                <ChevronRight className="h-4 w-4 text-muted-foreground" />
                                                            </button>
                                                        )
                                                    )}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                    <p className="text-sm text-muted-foreground">
                                        当前选择:{" "}
                                        {localPath || "未选择路径"}
                                    </p>
                                </div>
                            )}

                            {serviceType === TargetType.NDN && (
                                <div className="space-y-2">
                                    <Label htmlFor="ndnUrl">节点地址 *</Label>
                                    <Input
                                        id="ndnUrl"
                                        value={ndnUrl}
                                        onChange={(e) =>
                                            setNdnUrl(e.target.value)
                                        }
                                        placeholder="例如: ndn://backup.example.com"
                                    />
                                </div>
                            )}
                        </div>
                    </div>
                );
            case 3:
                return (
                    <div className="space-y-6">
                        <div>
                            <Label className="text-base">确认服务配置</Label>
                            <p className="text-sm text-muted-foreground mb-6">
                                请确认以下服务配置无误
                            </p>
                        </div>

                        <div className="space-y-4">
                            <Card>
                                <CardContent className="p-4">
                                    <div className="space-y-3">
                                        <div className="flex justify-between">
                                            <span className="text-sm text-muted-foreground">
                                                服务类型:
                                            </span>
                                            <span className="text-sm font-medium">
                                                {serviceType ===
                                                TargetType.LOCAL
                                                    ? "本地存储"
                                                    : "NDN存储"}
                                            </span>
                                        </div>
                                        <div className="flex justify-between">
                                            <span className="text-sm text-muted-foreground">
                                                服务名称:
                                            </span>
                                            <span className="text-sm font-medium">
                                                {serviceName}
                                            </span>
                                        </div>
                                        <div className="flex justify-between">
                                            <span className="text-sm text-muted-foreground">
                                                {serviceType ===
                                                TargetType.LOCAL
                                                    ? "存储路径:"
                                                    : "节点地址:"}
                                            </span>
                                            <span className="text-sm font-medium">
                                                {serviceType ===
                                                TargetType.LOCAL
                                                    ? localPath
                                                    : ndnUrl}
                                            </span>
                                        </div>
                                    </div>
                                </CardContent>
                            </Card>

                            <div className="p-4 bg-blue-50 rounded-lg dark:bg-blue-950">
                                <p className="text-sm text-blue-800 dark:text-blue-200">
                                    服务添加后将立即可用于新的备份计划。
                                </p>
                            </div>
                        </div>
                    </div>
                );

            default:
                return null;
        }
    };

    return (
        <div className={`${isMobile ? "p-4" : "p-6"} space-y-6`}>
            {/* 头部导航 */}
            <div className="flex items-center gap-4">
                <Button variant="ghost" size="sm" onClick={onBack}>
                    <ArrowLeft className="w-4 h-4" />
                    {!isMobile && <span className="ml-2">返回</span>}
                </Button>
                <div>
                    <h1 className="text-xl font-semibold">添加备份服务</h1>
                    <p className="text-sm text-muted-foreground">
                        步骤 {currentStep} / {steps.length}:{" "}
                        {steps[currentStep - 1].title}
                    </p>
                </div>
            </div>

            {/* 步骤指示器 */}
            <div className="flex items-center justify-between">
                {steps.map((step, index) => (
                    <div key={step.number} className="flex items-center">
                        <div
                            className={`flex items-center justify-center w-8 h-8 rounded-full text-sm font-medium ${
                                currentStep >= step.number
                                    ? "bg-primary text-primary-foreground"
                                    : "bg-muted text-muted-foreground"
                            }`}
                        >
                            {currentStep > step.number ? (
                                <Check className="w-4 h-4" />
                            ) : (
                                step.number
                            )}
                        </div>
                        {!isMobile && (
                            <div className="ml-3">
                                <p
                                    className={`text-sm font-medium ${
                                        currentStep >= step.number
                                            ? "text-foreground"
                                            : "text-muted-foreground"
                                    }`}
                                >
                                    {step.title}
                                </p>
                                <p className="text-xs text-muted-foreground">
                                    {step.description}
                                </p>
                            </div>
                        )}
                        {index < steps.length - 1 && (
                            <div
                                className={`w-8 h-px mx-4 ${
                                    currentStep > step.number
                                        ? "bg-primary"
                                        : "bg-muted"
                                }`}
                            />
                        )}
                    </div>
                ))}
            </div>

            {/* 步骤内容 */}
            <Card>
                <CardContent className="p-6">{renderStepContent()}</CardContent>
            </Card>

            {/* 底部按钮 */}
            <div className="flex items-center justify-between">
                <Button
                    variant="outline"
                    onClick={handlePrevious}
                    disabled={currentStep === 1}
                >
                    <ChevronLeft className="w-4 h-4 mr-2" />
                    上一步
                </Button>

                <div className="flex gap-3">
                    <Button variant="outline" onClick={onBack}>
                        取消
                    </Button>
                    {currentStep < 3 ? (
                        <Button onClick={handleNext} disabled={!canProceed()}>
                            下一步
                            <ChevronRight className="w-4 h-4 ml-2" />
                        </Button>
                    ) : (
                        <Button
                            onClick={handleComplete}
                            disabled={!canProceed()}
                        >
                            添加服务
                        </Button>
                    )}
                </div>
            </div>
        </div>
    );
}
