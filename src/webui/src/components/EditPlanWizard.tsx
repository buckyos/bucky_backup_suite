import React, { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Textarea } from "./ui/textarea";
import { RadioGroup, RadioGroupItem } from "./ui/radio-group";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "./ui/select";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { toast } from "sonner@2.0.3";
import {
    ArrowLeft,
    ChevronLeft,
    ChevronRight,
    Calendar,
    Clock,
    Check,
} from "lucide-react";

interface EditPlanWizardProps {
    onBack: () => void;
    onComplete: () => void;
    data?: any;
}

export function EditPlanWizard({
    onBack,
    onComplete,
    data,
}: EditPlanWizardProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [currentStep, setCurrentStep] = useState(1);

    // 表单数据
    const [planName, setPlanName] = useState(data?.name || "系统文件备份");
    const [planDescription, setPlanDescription] = useState(
        data?.description || "每日自动备份系统关键文件"
    );
    const [triggerType, setTriggerType] = useState("scheduled");
    const [schedulePeriod, setSchedulePeriod] = useState("daily");
    const [scheduleTime, setScheduleTime] = useState("23:00");
    const [weekDays, setWeekDays] = useState(["1", "3", "5"]);
    const [monthDay, setMonthDay] = useState("1");

    const steps = [
        { number: 1, title: "基本信息", description: "编辑计划名称和描述" },
        { number: 2, title: "调度设置", description: "配置执行时间" },
        { number: 3, title: "确认修改", description: "确认修改内容" },
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
        toast.success("备份计划已更新");
        onComplete();
    };

    const canProceed = () => {
        switch (currentStep) {
            case 1:
                return planName.trim() !== "";
            case 2:
                return true;
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
                            <Label className="text-base">计划信息</Label>
                            <p className="text-sm text-muted-foreground mb-4">
                                修改备份计划的基本信息
                            </p>
                        </div>

                        <div className="space-y-4">
                            <div className="space-y-2">
                                <Label htmlFor="planName">计划名称 *</Label>
                                <Input
                                    id="planName"
                                    value={planName}
                                    onChange={(e) =>
                                        setPlanName(e.target.value)
                                    }
                                    placeholder="输入计划名称"
                                />
                            </div>

                            <div className="space-y-2">
                                <Label htmlFor="planDescription">
                                    计划描述
                                </Label>
                                <Textarea
                                    id="planDescription"
                                    value={planDescription}
                                    onChange={(e) =>
                                        setPlanDescription(e.target.value)
                                    }
                                    placeholder="输入计划描述"
                                    rows={3}
                                />
                            </div>
                        </div>

                        <div className="p-4 bg-amber-50 rounded-lg dark:bg-amber-950">
                            <p className="text-sm text-amber-800 dark:text-amber-200">
                                注意:
                                备份源和目标服务不能在编辑时修改。如需更改，请创建新的备份计划。
                            </p>
                        </div>
                    </div>
                );

            case 2:
                return (
                    <div className="space-y-6">
                        <div>
                            <Label className="text-base">调度设置</Label>
                            <p className="text-sm text-muted-foreground mb-4">
                                配置备份计划的执行时间
                            </p>
                        </div>

                        <div className="space-y-6">
                            <div className="space-y-3">
                                <Label>触发方式</Label>
                                <RadioGroup
                                    value={triggerType}
                                    onValueChange={setTriggerType}
                                >
                                    <div className="flex items-center space-x-2">
                                        <RadioGroupItem
                                            value="scheduled"
                                            id="scheduled"
                                        />
                                        <Label htmlFor="scheduled">
                                            定时执行
                                        </Label>
                                    </div>
                                    <div className="flex items-center space-x-2">
                                        <RadioGroupItem
                                            value="manual"
                                            id="manual"
                                        />
                                        <Label htmlFor="manual">
                                            仅手动执行
                                        </Label>
                                    </div>
                                </RadioGroup>
                            </div>

                            {triggerType === "scheduled" && (
                                <div className="space-y-4">
                                    <div className="space-y-2">
                                        <Label>调度周期</Label>
                                        <Select
                                            value={schedulePeriod}
                                            onValueChange={setSchedulePeriod}
                                        >
                                            <SelectTrigger>
                                                <SelectValue />
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
                                    </div>

                                    <div className="space-y-2">
                                        <Label>执行时间</Label>
                                        <Input
                                            type="time"
                                            value={scheduleTime}
                                            onChange={(e) =>
                                                setScheduleTime(e.target.value)
                                            }
                                        />
                                    </div>

                                    {schedulePeriod === "weekly" && (
                                        <div className="space-y-2">
                                            <Label>星期</Label>
                                            <div className="flex flex-wrap gap-2">
                                                {[
                                                    {
                                                        value: "1",
                                                        label: "周一",
                                                    },
                                                    {
                                                        value: "2",
                                                        label: "周二",
                                                    },
                                                    {
                                                        value: "3",
                                                        label: "周三",
                                                    },
                                                    {
                                                        value: "4",
                                                        label: "周四",
                                                    },
                                                    {
                                                        value: "5",
                                                        label: "周五",
                                                    },
                                                    {
                                                        value: "6",
                                                        label: "周六",
                                                    },
                                                    {
                                                        value: "0",
                                                        label: "周日",
                                                    },
                                                ].map((day) => (
                                                    <Button
                                                        key={day.value}
                                                        variant={
                                                            weekDays.includes(
                                                                day.value
                                                            )
                                                                ? "default"
                                                                : "outline"
                                                        }
                                                        size="sm"
                                                        onClick={() => {
                                                            if (
                                                                weekDays.includes(
                                                                    day.value
                                                                )
                                                            ) {
                                                                setWeekDays(
                                                                    weekDays.filter(
                                                                        (d) =>
                                                                            d !==
                                                                            day.value
                                                                    )
                                                                );
                                                            } else {
                                                                setWeekDays([
                                                                    ...weekDays,
                                                                    day.value,
                                                                ]);
                                                            }
                                                        }}
                                                    >
                                                        {day.label}
                                                    </Button>
                                                ))}
                                            </div>
                                        </div>
                                    )}

                                    {schedulePeriod === "monthly" && (
                                        <div className="space-y-2">
                                            <Label>日期</Label>
                                            <Select
                                                value={monthDay}
                                                onValueChange={setMonthDay}
                                            >
                                                <SelectTrigger>
                                                    <SelectValue />
                                                </SelectTrigger>
                                                <SelectContent>
                                                    {Array.from(
                                                        { length: 31 },
                                                        (_, i) => (
                                                            <SelectItem
                                                                key={i + 1}
                                                                value={(
                                                                    i + 1
                                                                ).toString()}
                                                            >
                                                                {i + 1} 日
                                                            </SelectItem>
                                                        )
                                                    )}
                                                </SelectContent>
                                            </Select>
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    </div>
                );

            case 3:
                return (
                    <div className="space-y-6">
                        <div>
                            <Label className="text-base">确认修改</Label>
                            <p className="text-sm text-muted-foreground mb-6">
                                请确认以下修改内容
                            </p>
                        </div>

                        <div className="space-y-4">
                            <Card>
                                <CardHeader>
                                    <CardTitle className="text-base">
                                        计划信息
                                    </CardTitle>
                                </CardHeader>
                                <CardContent className="space-y-3">
                                    <div className="flex justify-between">
                                        <span className="text-sm text-muted-foreground">
                                            计划名称:
                                        </span>
                                        <span className="text-sm font-medium">
                                            {planName}
                                        </span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span className="text-sm text-muted-foreground">
                                            计划描述:
                                        </span>
                                        <span className="text-sm font-medium">
                                            {planDescription || "无描述"}
                                        </span>
                                    </div>
                                </CardContent>
                            </Card>

                            <Card>
                                <CardHeader>
                                    <CardTitle className="text-base">
                                        调度设置
                                    </CardTitle>
                                </CardHeader>
                                <CardContent className="space-y-3">
                                    <div className="flex justify-between">
                                        <span className="text-sm text-muted-foreground">
                                            触发方式:
                                        </span>
                                        <span className="text-sm font-medium">
                                            {triggerType === "scheduled"
                                                ? "定时执行"
                                                : "仅手动执行"}
                                        </span>
                                    </div>
                                    {triggerType === "scheduled" && (
                                        <>
                                            <div className="flex justify-between">
                                                <span className="text-sm text-muted-foreground">
                                                    调度周期:
                                                </span>
                                                <span className="text-sm font-medium">
                                                    {schedulePeriod === "daily"
                                                        ? "每天"
                                                        : schedulePeriod ===
                                                          "weekly"
                                                        ? "每周"
                                                        : "每月"}
                                                </span>
                                            </div>
                                            <div className="flex justify-between">
                                                <span className="text-sm text-muted-foreground">
                                                    执行时间:
                                                </span>
                                                <span className="text-sm font-medium">
                                                    {scheduleTime}
                                                </span>
                                            </div>
                                            {schedulePeriod === "weekly" && (
                                                <div className="flex justify-between">
                                                    <span className="text-sm text-muted-foreground">
                                                        执行日期:
                                                    </span>
                                                    <span className="text-sm font-medium">
                                                        {weekDays
                                                            .map((day) => {
                                                                const dayLabels =
                                                                    {
                                                                        "0": "周日",
                                                                        "1": "周一",
                                                                        "2": "周二",
                                                                        "3": "周三",
                                                                        "4": "周四",
                                                                        "5": "周五",
                                                                        "6": "周六",
                                                                    };
                                                                return dayLabels[
                                                                    day as keyof typeof dayLabels
                                                                ];
                                                            })
                                                            .join(", ")}
                                                    </span>
                                                </div>
                                            )}
                                            {schedulePeriod === "monthly" && (
                                                <div className="flex justify-between">
                                                    <span className="text-sm text-muted-foreground">
                                                        执行日期:
                                                    </span>
                                                    <span className="text-sm font-medium">
                                                        每月 {monthDay} 日
                                                    </span>
                                                </div>
                                            )}
                                        </>
                                    )}
                                </CardContent>
                            </Card>

                            <div className="p-4 bg-blue-50 rounded-lg dark:bg-blue-950">
                                <p className="text-sm text-blue-800 dark:text-blue-200">
                                    修改将立即生效，下次执行时间将根据新的调度设置计算。
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
                    <h1 className="text-xl font-semibold">编辑备份计划</h1>
                    <p className="text-sm text-muted-foreground">
                        修改备份计划的配置
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
                            保存修改
                        </Button>
                    )}
                </div>
            </div>
        </div>
    );
}
