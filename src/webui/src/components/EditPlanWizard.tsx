import React, { useEffect, useState } from "react";
import { Card, CardContent } from "./ui/card";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Textarea } from "./ui/textarea";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "./ui/select";
import { RadioGroup, RadioGroupItem } from "./ui/radio-group";
import { Checkbox } from "./ui/checkbox";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { toast } from "sonner";
import { ArrowLeft, ChevronLeft, ChevronRight, Check } from "lucide-react";
import {
    BackupPlanInfo,
    BackupTargetInfo,
    DirectoryPurpose,
    PlanPolicy,
    SourceType,
} from "./utils/task_mgr";
import { taskManager } from "./utils/fake_task_mgr";
import { TaskMgrHelper } from "./utils/task_mgr_helper";

interface EditPlanWizardProps {
    onBack: () => void;
    onComplete: () => void;
    plan: BackupPlanInfo;
}

interface PlanData {
    id: string;
    name: string;
    description: string;
    directories: string[];
    service: string;
    triggerTypes: ("scheduled" | "event")[];
    scheduleType?: "daily" | "weekly" | "monthly";
    scheduleTime?: string;
    scheduleDay?: string;
    scheduleDate?: string;
    eventDelay?: string;
    backupType: "full" | "incremental";
    versions: string;
    priority: "high" | "medium" | "low";
    last_checkpoint_index: number;
    create_time: number;
    update_time: number;
    total_backup: number;
    total_size: number;
}

const STEPS = [
    { id: "basic", title: "基本信息", description: "编辑计划名称和描述" },
    { id: "trigger", title: "触发规则", description: "编辑执行时间和条件" },
    { id: "advanced", title: "高级设置", description: "编辑备份选项" },
    { id: "review", title: "确认修改", description: "确认所有设置" },
];

function dataFromPlan(plan: BackupPlanInfo): PlanData {
    console.log("Converting plan to PlanData:", plan);
    let triggerTypes: ("scheduled" | "event")[] = [];
    let scheduleType: "daily" | "weekly" | "monthly" | undefined;
    let scheduleTime: string | undefined;
    let scheduleDay: string | undefined;
    let scheduleDate: string | undefined;
    let eventDelay: string | undefined;
    plan.policy.forEach((p) => {
        if ("minutes" in p && p.minutes !== undefined) {
            triggerTypes.push("scheduled");
            if ("week" in p && p.week !== undefined) {
                scheduleType = "weekly";
                scheduleDay = p.week.toString();
            } else if ("date" in p && p.date !== undefined) {
                scheduleType = "monthly";
                scheduleDate = p.date.toString();
            } else {
                scheduleType = "daily";
            }
            scheduleTime = TaskMgrHelper.formatMinutesToHHMM(p.minutes);
        } else if ("update_delay" in p && p.update_delay) {
            triggerTypes.push("event");
            eventDelay = p.update_delay.toString();
        }
    });
    if (plan.policy_disabled) {
        triggerTypes = [];
    }
    const planData: PlanData = {
        id: plan.plan_id,
        name: plan.title,
        description: plan.description,
        directories: [plan.source],
        service: plan.target,
        triggerTypes,
        scheduleType,
        scheduleTime,
        scheduleDay,
        scheduleDate,
        eventDelay,
        backupType: "incremental", // TODO: support full backup
        versions: plan.reserved_versions?.toString() || "0",
        priority:
            plan.priority >= 10
                ? "high"
                : plan.priority >= 5
                ? "medium"
                : "low",
        last_checkpoint_index: plan.last_checkpoint_index,
        create_time: plan.create_time,
        update_time: plan.update_time,
        total_backup: plan.total_backup,
        total_size: plan.total_size,
    };
    return planData;
}

function planFromData(
    plan: PlanData,
    services: BackupTargetInfo[]
): BackupPlanInfo {
    console.log("Converting PlanData to BackupPlanInfo:", plan);
    let policy: PlanPolicy[] = [];
    if (plan.scheduleTime) {
        const minutes = TaskMgrHelper.minutesFromHHMM(plan.scheduleTime!);
        if (plan.scheduleDay) {
            policy.push({
                minutes: minutes!,
                week: parseInt(plan.scheduleDay!),
            });
        } else if (plan.scheduleDate) {
            policy.push({
                minutes: minutes!,
                date: parseInt(plan.scheduleDate!),
            });
        } else {
            policy.push({ minutes: minutes! });
        }
    }
    if (plan.eventDelay) {
        policy.push({
            update_delay: parseInt(plan.eventDelay!),
        });
    }

    const backupPlan: BackupPlanInfo = {
        plan_id: plan.id,
        title: plan.name,
        description: plan.description,
        type_str: "",
        last_checkpoint_index: plan.last_checkpoint_index,
        source_type: SourceType.DIRECTORY,
        source: plan.directories[0],
        target_type: services.find((s) => s.target_id === plan.service)!
            .target_type,
        target: plan.service,
        policy_disabled: plan.triggerTypes.length === 0,
        policy,
        priority: { high: 10, medium: 5, low: 1 }[plan.priority],
        reserved_versions: parseInt(plan.versions) || 0,
        create_time: plan.create_time,
        update_time: plan.update_time,
        total_backup: plan.total_backup,
        total_size: plan.total_size,
    };
    return backupPlan;
}

export function EditPlanWizard({
    onBack,
    onComplete,
    plan,
}: EditPlanWizardProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [currentStep, setCurrentStep] = useState(0);
    const [planData, setPlanData] = useState<PlanData>(dataFromPlan(plan));

    const [errors, setErrors] = useState<Record<string, string>>({});

    const [services, setServices] = useState<BackupTargetInfo[]>([]); // Mocked available services

    useEffect(() => {
        // load services from backend
        taskManager.listBackupTargets().then(async (targetIds) => {
            const targets = await Promise.all(
                targetIds.map((id) => taskManager.getBackupTarget(id))
            );
            setServices(targets);
        });
        const timerId = taskManager.startRefreshTargetStateTimer();
        return () => {
            taskManager.stopRefreshTargetStateTimer(timerId);
        };
    }, []);

    const validateStep = (
        step: number,
        updateErrors: boolean = true
    ): boolean => {
        const newErrors: Record<string, string> = {};

        switch (step) {
            case 0: // Basic info
                if (!planData.name.trim()) {
                    newErrors.name = "计划名称不能为空";
                }
                break;
            // case 1: // Directories
            //     if (planData.directories.length === 0) {
            //         newErrors.directories = "请至少选择一个目录";
            //     }
            //     break;
            // case 2: // Service
            //     if (!planData.service) {
            //         newErrors.service = "请选择备份服务";
            //     }
            //     break;
            case 1: // Trigger
                if (planData.triggerTypes.includes("scheduled")) {
                    if (!planData.scheduleType) {
                        newErrors.scheduleType = "请选择调度周期";
                    }
                    if (!planData.scheduleTime) {
                        newErrors.scheduleTime = "请设置执行时间";
                    }
                    if (
                        planData.scheduleType === "weekly" &&
                        (!planData.scheduleDay || !planData.scheduleDay.length)
                    ) {
                        newErrors.scheduleDay = "请选择执行日期";
                    }
                    if (
                        planData.scheduleType === "monthly" &&
                        !planData.scheduleDate
                    ) {
                        newErrors.scheduleDate = "请选择执行日期";
                    }
                }
                if (planData.triggerTypes.includes("event")) {
                    if (!planData.eventDelay) {
                        newErrors.eventDelay = "请设置触发延迟时间";
                    }
                }
                break;
        }

        if (updateErrors) {
            setErrors(newErrors);
        }
        return Object.keys(newErrors).length === 0;
    };

    const nextStep = () => {
        if (validateStep(currentStep, true)) {
            setCurrentStep((prev) => Math.min(prev + 1, STEPS.length - 1));
        }
    };

    const prevStep = () => {
        setCurrentStep((prev) => Math.max(prev - 1, 0));
    };

    const handleFinish = async () => {
        if (validateStep(currentStep, true)) {
            try {
                await taskManager.updateBackupPlan(
                    planFromData(planData, services)
                );
                toast.success("备份计划已更新");
                onComplete();
            } catch (error) {
                console.error("Error updating backup plan:", error);
                toast.error("更新备份计划失败");
            }
        }
    };

    const updatePlanData = (updates: Partial<PlanData>) => {
        setPlanData((prev) => ({ ...prev, ...updates }));
        // Clear related errors
        const newErrors = { ...errors };
        Object.keys(updates).forEach((key) => {
            delete newErrors[key];
        });
        setErrors(newErrors);
    };

    const renderStepContent = () => {
        switch (currentStep) {
            case 0:
                return (
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label htmlFor="planName">计划名称 *</Label>
                            <Input
                                id="planName"
                                value={planData.name}
                                onChange={(e) =>
                                    updatePlanData({ name: e.target.value })
                                }
                                placeholder="输入备份计划名称"
                                className={errors.name ? "border-red-500" : ""}
                            />
                            {errors.title && (
                                <p className="text-sm text-red-500">
                                    {errors.title}
                                </p>
                            )}
                        </div>
                        <div className="space-y-2">
                            <Label htmlFor="planDescription">计划描述</Label>
                            <Textarea
                                id="planDescription"
                                value={planData.description}
                                onChange={(e) =>
                                    updatePlanData({
                                        description: e.target.value,
                                    })
                                }
                                placeholder="描述这个备份计划的用途"
                                rows={3}
                            />
                        </div>

                        <div className="p-4 bg-amber-50 rounded-lg dark:bg-amber-950">
                            <p className="text-sm text-amber-800 dark:text-amber-200">
                                注意:
                                备份源和目标服务不能在编辑时修改。如需更改，请创建新的备份计划。
                            </p>
                        </div>
                    </div>
                );

            case 1:
                return (
                    <div className="space-y-4">
                        <div className="space-y-3">
                            <Label>触发方式</Label>
                            <p className="text-sm text-muted-foreground">
                                可以选择多种触发方式，如果都不选择则只能手动执行
                            </p>
                            <div className="space-y-3">
                                <div className="flex items-center space-x-2">
                                    <Checkbox
                                        id="scheduled"
                                        checked={planData.triggerTypes.includes(
                                            "scheduled"
                                        )}
                                        onCheckedChange={(checked) => {
                                            const types = planData.triggerTypes;
                                            if (checked) {
                                                updatePlanData({
                                                    triggerTypes: [
                                                        ...types,
                                                        "scheduled",
                                                    ],
                                                });
                                            } else {
                                                updatePlanData({
                                                    triggerTypes: types.filter(
                                                        (t) => t !== "scheduled"
                                                    ),
                                                });
                                            }
                                        }}
                                    />
                                    <Label htmlFor="scheduled">定时执行</Label>
                                </div>
                                <div className="flex items-center space-x-2">
                                    <Checkbox
                                        id="event"
                                        checked={planData.triggerTypes.includes(
                                            "event"
                                        )}
                                        onCheckedChange={(checked) => {
                                            const types = planData.triggerTypes;
                                            if (checked) {
                                                updatePlanData({
                                                    triggerTypes: [
                                                        ...types,
                                                        "event",
                                                    ],
                                                });
                                            } else {
                                                updatePlanData({
                                                    triggerTypes: types.filter(
                                                        (t) => t !== "event"
                                                    ),
                                                });
                                            }
                                        }}
                                    />
                                    <Label htmlFor="event">事件触发</Label>
                                </div>
                            </div>
                        </div>

                        {planData.triggerTypes.includes("scheduled") && (
                            <div className="space-y-4 p-4 border rounded-md">
                                <h4 className="font-medium">定时执行设置</h4>
                                <div className="space-y-2">
                                    <Label>调度周期 *</Label>
                                    <Select
                                        value={planData.scheduleType}
                                        onValueChange={(
                                            value:
                                                | "daily"
                                                | "weekly"
                                                | "monthly"
                                        ) =>
                                            updatePlanData({
                                                scheduleType: value,
                                            })
                                        }
                                    >
                                        <SelectTrigger
                                            className={
                                                errors.scheduleType
                                                    ? "border-red-500"
                                                    : ""
                                            }
                                        >
                                            <SelectValue placeholder="选择调度周期" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="daily">
                                                每天
                                            </SelectItem>
                                            <SelectItem value="weekly">
                                                每周
                                            </SelectItem>
                                            <SelectItem value="monthly">
                                                每月
                                            </SelectItem>
                                        </SelectContent>
                                    </Select>
                                    {errors.scheduleType && (
                                        <p className="text-sm text-red-500">
                                            {errors.scheduleType}
                                        </p>
                                    )}
                                </div>

                                <div className="space-y-2">
                                    <Label>执行时间 *</Label>
                                    <Input
                                        type="time"
                                        value={planData.scheduleTime || ""}
                                        onChange={(e) =>
                                            updatePlanData({
                                                scheduleTime: e.target.value,
                                            })
                                        }
                                        className={
                                            errors.scheduleTime
                                                ? "border-red-500"
                                                : ""
                                        }
                                    />
                                    {errors.scheduleTime && (
                                        <p className="text-sm text-red-500">
                                            {errors.scheduleTime}
                                        </p>
                                    )}
                                </div>

                                {planData.scheduleType === "weekly" && (
                                    <div className="space-y-2">
                                        <Label>执行日期 *</Label>
                                        <RadioGroup
                                            value={planData.scheduleDay}
                                            onValueChange={(value) =>
                                                updatePlanData({
                                                    scheduleDay: value,
                                                })
                                            }
                                            className="grid grid-cols-2 gap-2 sm:grid-cols-4"
                                        >
                                            {[
                                                "周一",
                                                "周二",
                                                "周三",
                                                "周四",
                                                "周五",
                                                "周六",
                                                "周日",
                                            ].map((day, index) => (
                                                <div
                                                    key={day}
                                                    className="flex items-center space-x-2"
                                                >
                                                    <RadioGroupItem
                                                        id={`day-${index}`}
                                                        value={`${index + 1}`}
                                                    />
                                                    <Label
                                                        htmlFor={`day-${index}`}
                                                        className="text-sm"
                                                    >
                                                        {day}
                                                    </Label>
                                                </div>
                                            ))}
                                        </RadioGroup>
                                        {errors.scheduleDay && (
                                            <p className="text-sm text-red-500">
                                                {errors.scheduleDay}
                                            </p>
                                        )}
                                    </div>
                                )}

                                {planData.scheduleType === "monthly" && (
                                    <div className="space-y-2">
                                        <Label>执行日期 *</Label>
                                        <Select
                                            value={planData.scheduleDate}
                                            onValueChange={(value) =>
                                                updatePlanData({
                                                    scheduleDate: value,
                                                })
                                            }
                                        >
                                            <SelectTrigger
                                                className={
                                                    errors.scheduleDate
                                                        ? "border-red-500"
                                                        : ""
                                                }
                                            >
                                                <SelectValue placeholder="选择每月执行日期" />
                                            </SelectTrigger>
                                            <SelectContent>
                                                {Array.from(
                                                    { length: 31 },
                                                    (_, i) => (
                                                        <SelectItem
                                                            key={i + 1}
                                                            value={String(
                                                                i + 1
                                                            )}
                                                        >
                                                            {i + 1}日
                                                        </SelectItem>
                                                    )
                                                )}
                                            </SelectContent>
                                        </Select>
                                        {errors.scheduleDate && (
                                            <p className="text-sm text-red-500">
                                                {errors.scheduleDate}
                                            </p>
                                        )}
                                    </div>
                                )}
                            </div>
                        )}

                        {planData.triggerTypes.includes("event") && (
                            <div className="space-y-4 p-4 border rounded-md">
                                <h4 className="font-medium">事件触发设置</h4>
                                <div className="space-y-2">
                                    <Label>触发延迟 *</Label>
                                    <div className="flex items-center gap-2">
                                        <Input
                                            type="number"
                                            value={planData.eventDelay || ""}
                                            onChange={(e) =>
                                                updatePlanData({
                                                    eventDelay: e.target.value,
                                                })
                                            }
                                            min="1"
                                            max="60"
                                            className={
                                                errors.eventDelay
                                                    ? "border-red-500"
                                                    : ""
                                            }
                                        />
                                        <span className="text-sm text-muted-foreground">
                                            秒
                                        </span>
                                    </div>
                                    <p className="text-sm text-muted-foreground">
                                        文件变更后等待指定时间内无新变更时触发备份
                                    </p>
                                    {errors.eventDelay && (
                                        <p className="text-sm text-red-500">
                                            {errors.eventDelay}
                                        </p>
                                    )}
                                </div>
                            </div>
                        )}

                        {planData.triggerTypes.length === 0 && (
                            <div className="p-4 border rounded-md bg-muted">
                                <p className="text-sm text-muted-foreground">
                                    当前设置为仅手动执行模式，需要手动启动备份任务
                                </p>
                            </div>
                        )}
                    </div>
                );

            case 2:
                return (
                    <div className="space-y-4">
                        {/* <div className="space-y-3">
                            <Label>备份方式</Label>
                            <RadioGroup
                                value={planData.backupType}
                                onValueChange={(
                                    value: "full" | "incremental"
                                ) => updatePlanData({ backupType: value })}
                            >
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem value="full" id="full" />
                                    <Label htmlFor="full">完全备份</Label>
                                </div>
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem
                                        value="incremental"
                                        id="incremental"
                                    />
                                    <Label htmlFor="incremental">
                                        增量备份
                                    </Label>
                                </div>
                            </RadioGroup>
                        </div> */}

                        <div className="space-y-2">
                            <Label>版本保留数量</Label>
                            <Input
                                type="number"
                                value={planData.versions}
                                onChange={(e) =>
                                    updatePlanData({ versions: e.target.value })
                                }
                                min="1"
                                max="100"
                            />
                            <p className="text-sm text-muted-foreground">
                                保留最近几个版本的备份
                            </p>
                        </div>

                        <div className="space-y-2">
                            <Label>任务优先级</Label>
                            <Select
                                value={planData.priority}
                                onValueChange={(
                                    value: "high" | "medium" | "low"
                                ) => updatePlanData({ priority: value })}
                            >
                                <SelectTrigger>
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="high">高</SelectItem>
                                    <SelectItem value="medium">中</SelectItem>
                                    <SelectItem value="low">低</SelectItem>
                                </SelectContent>
                            </Select>
                        </div>
                    </div>
                );

            case 3:
                return (
                    <div className="space-y-4">
                        <div className="bg-muted p-4 rounded-md space-y-3">
                            <h4 className="font-medium">备份计划概览</h4>
                            <div className="space-y-2 text-sm">
                                <div>
                                    <span className="font-medium">
                                        计划名称:
                                    </span>{" "}
                                    {planData.name}
                                </div>
                                {planData.description && (
                                    <div>
                                        <span className="font-medium">
                                            描述:
                                        </span>{" "}
                                        {planData.description}
                                    </div>
                                )}
                                <div>
                                    <span className="font-medium">
                                        备份目录:
                                    </span>{" "}
                                    {planData.directories.length} 个目录
                                </div>
                                <div>
                                    <span className="font-medium">
                                        备份服务:
                                    </span>{" "}
                                    {planData.service}
                                </div>
                                <div>
                                    <span className="font-medium">
                                        触发方式:
                                    </span>{" "}
                                    {planData.triggerTypes.length === 0
                                        ? "仅手动执行"
                                        : planData.triggerTypes
                                              .map((type) =>
                                                  type === "scheduled"
                                                      ? "定时执行"
                                                      : "事件触发"
                                              )
                                              .join(", ")}
                                </div>
                                {/* <div>
                                    <span className="font-medium">
                                        备份类型:
                                    </span>{" "}
                                    {planData.backupType === "full"
                                        ? "完全备份"
                                        : "增量备份"}
                                </div> */}
                                <div>
                                    <span className="font-medium">
                                        版本保留:
                                    </span>{" "}
                                    {planData.versions} 个版本
                                </div>
                                <div>
                                    <span className="font-medium">优先级:</span>{" "}
                                    {planData.priority === "high"
                                        ? "高"
                                        : planData.priority === "medium"
                                        ? "中"
                                        : "低"}
                                </div>
                            </div>
                        </div>
                        <div className="p-4 bg-blue-50 rounded-lg dark:bg-blue-950">
                            <p className="text-sm text-blue-800 dark:text-blue-200">
                                修改将立即生效，下次执行时间将根据新的调度设置计算。
                            </p>
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
                    <h1 className="text-xl font-semibold">编辑备份计划</h1>
                    <p className="text-sm text-muted-foreground">
                        步骤 {currentStep + 1} / {STEPS.length}:{" "}
                        {STEPS[currentStep].title}
                    </p>
                </div>
            </div>

            {/* 步骤指示器 */}
            <div className="flex items-center justify-between">
                {STEPS.map((step, index) => (
                    <div key={step.id} className="flex items-center">
                        <div
                            className={`flex items-center justify-center w-8 h-8 rounded-full text-sm font-medium ${
                                currentStep >= index
                                    ? "bg-primary text-primary-foreground"
                                    : "bg-muted text-muted-foreground"
                            }`}
                        >
                            {currentStep > index ? (
                                <Check className="w-4 h-4" />
                            ) : (
                                index + 1
                            )}
                        </div>
                        {!isMobile && (
                            <div className="ml-3">
                                <p
                                    className={`text-sm font-medium ${
                                        currentStep >= index
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
                        {index < STEPS.length - 1 && (
                            <div
                                className={`w-8 h-px mx-4 ${
                                    currentStep > index
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
                    onClick={prevStep}
                    disabled={currentStep === 0}
                >
                    <ChevronLeft className="w-4 h-4 mr-2" />
                    上一步
                </Button>

                <div className="flex gap-3">
                    <Button variant="outline" onClick={onBack}>
                        取消
                    </Button>
                    {currentStep < STEPS.length - 1 ? (
                        <Button onClick={nextStep}>
                            下一步
                            <ChevronRight className="w-4 h-4 ml-2" />
                        </Button>
                    ) : (
                        <Button onClick={handleFinish}>
                            <Check className="w-4 h-4 mr-2" />
                            完成更新
                        </Button>
                    )}
                </div>
            </div>
        </div>
    );
}
