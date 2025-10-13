import React, { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Checkbox } from "./ui/checkbox";
import { RadioGroup, RadioGroupItem } from "./ui/radio-group";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "./ui/select";
import { DirectorySelector } from "./DirectorySelector";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { toast } from "sonner";
import {
    ArrowLeft,
    ChevronLeft,
    ChevronRight,
    Folder,
    FileText,
    HardDrive,
    Check,
} from "lucide-react";

interface RestoreWizardProps {
    onBack: () => void;
    onComplete: () => void;
    data?: { planId?: string; taskId?: string };
}

export function RestoreWizard({
    onBack,
    onComplete,
    data,
}: RestoreWizardProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [currentStep, setCurrentStep] = useState(1);
    const [selectedBackup, setSelectedBackup] = useState("");
    const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
    const [restoreType, setRestoreType] = useState("original");
    const [customPath, setCustomPath] = useState("");
    const [overwriteMode, setOverwriteMode] = useState("skip");

    // 模拟备份数据
    const availableBackups = [
        {
            id: "1",
            planName: "系统文件备份",
            date: "2024-01-15 23:30:00",
            size: "2.1 GB",
            files: [
                "C:\\Windows\\System32\\config",
                "C:\\Windows\\System32\\drivers",
            ],
            status: "completed",
        },
        {
            id: "2",
            planName: "项目文件备份",
            date: "2024-01-15 02:45:00",
            size: "856 MB",
            files: ["D:\\Projects\\WebApp", "D:\\Projects\\Mobile"],
            status: "completed",
        },
        {
            id: "3",
            planName: "文档备份",
            date: "2024-01-14 01:00:00",
            size: "1.2 GB",
            files: ["C:\\Users\\Documents", "C:\\Users\\Desktop"],
            status: "completed",
        },
    ];

    const steps = [
        { number: 1, title: "选择备份", description: "选择要恢复的备份" },
        { number: 2, title: "选择文件", description: "选择要恢复的文件" },
        { number: 3, title: "恢复设置", description: "配置恢复选项" },
        { number: 4, title: "确认恢复", description: "确认恢复设置" },
    ];

    const selectedBackupData = availableBackups.find(
        (b) => b.id === selectedBackup
    );

    const handleNext = () => {
        if (currentStep < 4) {
            setCurrentStep(currentStep + 1);
        }
    };

    const handlePrevious = () => {
        if (currentStep > 1) {
            setCurrentStep(currentStep - 1);
        }
    };

    const handleComplete = () => {
        toast.success("恢复任务已创建");
        onComplete();
    };

    const canProceed = () => {
        switch (currentStep) {
            case 1:
                return selectedBackup !== "";
            case 2:
                return selectedFiles.length > 0;
            case 3:
                return restoreType === "original" || customPath !== "";
            case 4:
                return true;
            default:
                return false;
        }
    };

    const renderStepContent = () => {
        switch (currentStep) {
            case 1:
                return (
                    <div className="space-y-4">
                        <div>
                            <Label className="text-base">
                                选择要恢复的备份
                            </Label>
                            <p className="text-sm text-muted-foreground mb-4">
                                从可用的备份中选择一个进行恢复
                            </p>
                        </div>

                        <div className="space-y-3">
                            {availableBackups.map((backup) => (
                                <Card
                                    key={backup.id}
                                    className={`cursor-pointer transition-all ${
                                        selectedBackup === backup.id
                                            ? "ring-2 ring-primary"
                                            : ""
                                    }`}
                                    onClick={() => setSelectedBackup(backup.id)}
                                >
                                    <CardContent className="p-4">
                                        <div className="flex items-center justify-between">
                                            <div className="flex items-center gap-3">
                                                <div
                                                    className={`w-4 h-4 rounded-full border-2 ${
                                                        selectedBackup ===
                                                        backup.id
                                                            ? "border-primary bg-primary"
                                                            : "border-muted-foreground"
                                                    }`}
                                                >
                                                    {selectedBackup ===
                                                        backup.id && (
                                                        <Check className="w-2 h-2 text-primary-foreground m-auto" />
                                                    )}
                                                </div>
                                                <div>
                                                    <p className="font-medium">
                                                        {backup.planName}
                                                    </p>
                                                    <p className="text-sm text-muted-foreground">
                                                        {backup.date}
                                                    </p>
                                                </div>
                                            </div>
                                            <div className="text-right">
                                                <p className="text-sm font-medium">
                                                    {backup.size}
                                                </p>
                                                <p className="text-xs text-muted-foreground">
                                                    {backup.files.length} 个目录
                                                </p>
                                            </div>
                                        </div>
                                    </CardContent>
                                </Card>
                            ))}
                        </div>
                    </div>
                );

            case 2:
                return (
                    <div className="space-y-4">
                        <div>
                            <Label className="text-base">
                                选择要恢复的文件
                            </Label>
                            <p className="text-sm text-muted-foreground mb-4">
                                选择要从备份中恢复的文件和目录
                            </p>
                        </div>

                        {selectedBackupData && (
                            <div className="space-y-3">
                                <div className="flex items-center space-x-2">
                                    <Checkbox
                                        id="select-all"
                                        checked={
                                            selectedFiles.length ===
                                            selectedBackupData.files.length
                                        }
                                        onCheckedChange={(checked) => {
                                            if (checked) {
                                                setSelectedFiles(
                                                    selectedBackupData.files
                                                );
                                            } else {
                                                setSelectedFiles([]);
                                            }
                                        }}
                                    />
                                    <Label
                                        htmlFor="select-all"
                                        className="font-medium"
                                    >
                                        全选
                                    </Label>
                                </div>

                                {selectedBackupData.files.map((file, index) => (
                                    <div
                                        key={index}
                                        className="flex items-center space-x-2 p-3 border rounded-lg"
                                    >
                                        <Checkbox
                                            id={`file-${index}`}
                                            checked={selectedFiles.includes(
                                                file
                                            )}
                                            onCheckedChange={(checked) => {
                                                if (checked) {
                                                    setSelectedFiles([
                                                        ...selectedFiles,
                                                        file,
                                                    ]);
                                                } else {
                                                    setSelectedFiles(
                                                        selectedFiles.filter(
                                                            (f) => f !== file
                                                        )
                                                    );
                                                }
                                            }}
                                        />
                                        <Folder className="w-4 h-4 text-muted-foreground" />
                                        <Label
                                            htmlFor={`file-${index}`}
                                            className="flex-1"
                                        >
                                            {file}
                                        </Label>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                );

            case 3:
                return (
                    <div className="space-y-6">
                        <div>
                            <Label className="text-base">恢复目标</Label>
                            <p className="text-sm text-muted-foreground mb-4">
                                选择文件的恢复位置
                            </p>
                        </div>

                        <RadioGroup
                            value={restoreType}
                            onValueChange={setRestoreType}
                        >
                            <div className="flex items-center space-x-2">
                                <RadioGroupItem
                                    value="original"
                                    id="original"
                                />
                                <Label htmlFor="original">恢复到原始位置</Label>
                            </div>
                            <div className="flex items-center space-x-2">
                                <RadioGroupItem value="custom" id="custom" />
                                <Label htmlFor="custom">恢复到自定义位置</Label>
                            </div>
                        </RadioGroup>

                        {restoreType === "custom" && (
                            <div className="space-y-2">
                                <Label>目标路径</Label>
                                <DirectorySelector
                                    value={customPath}
                                    onChange={setCustomPath}
                                    placeholder="选择恢复目标路径"
                                />
                            </div>
                        )}

                        <div className="space-y-3">
                            <Label className="text-base">文件冲突处理</Label>
                            <RadioGroup
                                value={overwriteMode}
                                onValueChange={setOverwriteMode}
                            >
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem value="skip" id="skip" />
                                    <Label htmlFor="skip">
                                        跳过已存在的文件
                                    </Label>
                                </div>
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem
                                        value="overwrite"
                                        id="overwrite"
                                    />
                                    <Label htmlFor="overwrite">
                                        覆盖已存在的文件
                                    </Label>
                                </div>
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem
                                        value="rename"
                                        id="rename"
                                    />
                                    <Label htmlFor="rename">重命名新文件</Label>
                                </div>
                            </RadioGroup>
                        </div>
                    </div>
                );

            case 4:
                return (
                    <div className="space-y-6">
                        <div>
                            <Label className="text-base">确认恢复设置</Label>
                            <p className="text-sm text-muted-foreground mb-6">
                                请确认以下恢复设置无误
                            </p>
                        </div>

                        <div className="space-y-4">
                            <Card>
                                <CardContent className="p-4">
                                    <div className="space-y-3">
                                        <div className="flex justify-between">
                                            <span className="text-sm text-muted-foreground">
                                                备份来源:
                                            </span>
                                            <span className="text-sm font-medium">
                                                {selectedBackupData?.planName}
                                            </span>
                                        </div>
                                        <div className="flex justify-between">
                                            <span className="text-sm text-muted-foreground">
                                                备份时间:
                                            </span>
                                            <span className="text-sm font-medium">
                                                {selectedBackupData?.date}
                                            </span>
                                        </div>
                                        <div className="flex justify-between">
                                            <span className="text-sm text-muted-foreground">
                                                选择文件:
                                            </span>
                                            <span className="text-sm font-medium">
                                                {selectedFiles.length} 个项目
                                            </span>
                                        </div>
                                        <div className="flex justify-between">
                                            <span className="text-sm text-muted-foreground">
                                                恢复到:
                                            </span>
                                            <span className="text-sm font-medium">
                                                {restoreType === "original"
                                                    ? "原始位置"
                                                    : customPath}
                                            </span>
                                        </div>
                                        <div className="flex justify-between">
                                            <span className="text-sm text-muted-foreground">
                                                冲突处理:
                                            </span>
                                            <span className="text-sm font-medium">
                                                {overwriteMode === "skip"
                                                    ? "跳过已存在文件"
                                                    : overwriteMode ===
                                                      "overwrite"
                                                    ? "覆盖已存在文件"
                                                    : "重命名新文件"}
                                            </span>
                                        </div>
                                    </div>
                                </CardContent>
                            </Card>

                            <div className="p-4 bg-blue-50 rounded-lg dark:bg-blue-950">
                                <p className="text-sm text-blue-800 dark:text-blue-200">
                                    恢复任务将在后台执行，您可以在任务列表中查看进度。
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
                    <h1 className="text-xl font-semibold">创建恢复任务</h1>
                    <p className="text-sm text-muted-foreground">
                        从备份中恢复文件和目录
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
                    {currentStep < 4 ? (
                        <Button onClick={handleNext} disabled={!canProceed()}>
                            下一步
                            <ChevronRight className="w-4 h-4 ml-2" />
                        </Button>
                    ) : (
                        <Button
                            onClick={handleComplete}
                            disabled={!canProceed()}
                        >
                            创建恢复任务
                        </Button>
                    )}
                </div>
            </div>
        </div>
    );
}
