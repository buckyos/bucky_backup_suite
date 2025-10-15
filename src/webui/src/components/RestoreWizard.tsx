import React, { useEffect, useState } from "react";
import { Card, CardContent } from "./ui/card";
import { Button } from "./ui/button";
import { Label } from "./ui/label";
import { Checkbox } from "./ui/checkbox";
import { RadioGroup, RadioGroupItem } from "./ui/radio-group";
import { DirectorySelector } from "./DirectorySelector";
import { useMobile } from "./hooks/use_mobile";
import { toast } from "sonner";
import {
    ArrowLeft,
    ChevronLeft,
    ChevronRight,
    Folder,
    Check,
} from "lucide-react";
import {
    BackupPlanInfo,
    ListOrder,
    ListTaskOrderBy,
    TaskInfo,
    TaskState,
    TaskType,
} from "./utils/task_mgr";
import { taskManager } from "./utils/fake_task_mgr";

interface RestoreWizardProps {
    onBack: () => void;
    onComplete: () => void;
    data?: { planId?: string; taskId?: string };
}

const STEPS = [
    { number: 1, title: "选择备份计划", description: "选择要恢复的备份计划" },
    { number: 2, title: "选择备份任务", description: "确定要恢复的备份任务" },
    { number: 3, title: "选择备份文件", description: "勾选需要恢复的文件" },
    { number: 4, title: "选择恢复目标", description: "设置恢复目标与策略" },
    { number: 5, title: "确认与完成", description: "检查信息并创建任务" },
];

export function RestoreWizard({
    onBack,
    onComplete,
    data,
}: RestoreWizardProps) {
    const isMobile = useMobile();
    const [currentStep, setCurrentStep] = useState(
        (data && (data.taskId ? 3 : data.planId ? 2 : 1)) || 1
    );
    const [planList, setPlanList] = useState<BackupPlanInfo[] | null>(null);
    const [selectedPlanId, setSelectedPlanId] = useState(
        () => data?.planId ?? ""
    );
    const [taskList, setTaskList] = useState<TaskInfo[] | null>(null);
    const [selectedTaskId, setSelectedTaskId] = useState(
        () => data?.taskId ?? ""
    );
    const [viewDirectory, setViewDirectory] = useState<string | null>(null);
    const [fileList, setFileList] = useState<string[] | null>(null);
    const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
    const [restoreType, setRestoreType] = useState<"original" | "custom">(
        "original"
    );
    const [customPath, setCustomPath] = useState("");
    const [overwriteMode, setOverwriteMode] = useState<
        "skip" | "overwrite" | "rename"
    >("skip");

    const selectedPlan =
        planList && planList.find((plan) => plan.plan_id === selectedPlanId);
    const selectedTask =
        taskList && taskList.find((task) => task.taskid === selectedTaskId);

    const loadPlanList = async () => {
        if (data?.planId) {
            setSelectedPlanId(data.planId);
            const plan = await taskManager.getBackupPlan(data.planId);
            if (plan) {
                setPlanList([plan]);
            }
        } else {
            const planIds = await taskManager.listBackupPlans();
            const plans = await Promise.all(
                planIds.map((id) => taskManager.getBackupPlan(id))
            );
            setPlanList(plans.filter((p): p is BackupPlanInfo => p !== null));
            if (data?.taskId) {
                const plan = plans.find((p) => p.plan_id === data.taskId);
                if (plan) {
                    setSelectedPlanId(plan.plan_id);
                    setPlanList([plan]);
                }
            }
        }
    };

    const loadTaskList = async () => {
        if (data?.taskId) {
            setSelectedTaskId(data.taskId);
            const task = await taskManager.getTaskInfo(data.taskId);
            if (task) {
                setTaskList([task]);
                if (planList) {
                    const plan = planList.find(
                        (p) => p.plan_id === task.owner_plan_id
                    );
                    if (plan) {
                        setSelectedPlanId(plan.plan_id);
                        setPlanList([plan]);
                    }
                }
            }
        } else if (selectedPlanId) {
            const taskIds = await taskManager.listBackupTasks(
                {
                    type: [TaskType.BACKUP],
                    owner_plan_id: [selectedPlanId],
                    state: [TaskState.DONE],
                },
                0,
                undefined,
                [[ListTaskOrderBy.CREATE_TIME, ListOrder.DESC]]
            );
            const tasks = await Promise.all(
                taskIds.task_ids.map((id) => taskManager.getTaskInfo(id))
            );
            setTaskList(tasks.filter((t): t is TaskInfo => t !== null));
        }
    };

    const loadFiles = async () => {
        const files = await taskManager.listFilesInTask(
            selectedTaskId,
            viewDirectory
        );
        setFileList(files);
    };

    const init = async () => {
        Promise.all([loadPlanList(), loadTaskList()]).catch((err) => {
            console.error("Failed to initialize RestoreWizard:", err);
        });
    };

    useEffect(() => {
        init();
    }, []);

    useEffect(() => {
        if (!data?.taskId) {
            setSelectedTaskId("");
            setTaskList(null);
            setSelectedFiles([]);
            setViewDirectory(null);
            loadTaskList();
        }
    }, [selectedPlanId]);

    useEffect(() => {
        setSelectedFiles([]);
        setViewDirectory(null);
    }, [selectedTaskId]);

    useEffect(() => {
        loadFiles();
    }, [viewDirectory, selectedTaskId]);

    const canProceed = () => {
        switch (currentStep) {
            case 1:
                return selectedPlanId !== "";
            case 2:
                return selectedTaskId !== "";
            case 3:
                return selectedFiles.length > 0;
            case 4:
                return (
                    restoreType === "original" || customPath.trim().length > 0
                );
            default:
                return true;
        }
    };

    const statusLabel = (status: string) => {
        switch (status) {
            case "completed":
                return "已完成";
            case "running":
                return "进行中";
            case "failed":
                return "失败";
            default:
                return status;
        }
    };

    const overwriteModeLabel =
        overwriteMode === "skip"
            ? "跳过已存在文件"
            : overwriteMode === "overwrite"
            ? "覆盖已存在文件"
            : "重命名新文件";

    const renderStep = () => {
        switch (currentStep) {
            case 1:
                return (
                    <div className="space-y-4">
                        <div>
                            <Label className="text-base">选择备份计划</Label>
                            <p className="text-sm text-muted-foreground">
                                请选择需要恢复的备份计划。
                            </p>
                        </div>
                        <div className="space-y-3">
                            {planList.map((plan) => (
                                <Card
                                    key={plan.id}
                                    className={`cursor-pointer transition-all ${
                                        plan.id === selectedPlanId
                                            ? "ring-2 ring-primary"
                                            : ""
                                    }`}
                                    onClick={() => setSelectedPlanId(plan.id)}
                                >
                                    <CardContent className="p-4 flex items-center justify-between gap-4">
                                        <div>
                                            <p className="font-medium">
                                                {plan.name}
                                            </p>
                                            <p className="text-xs text-muted-foreground">
                                                {plan.description}
                                            </p>
                                        </div>
                                        <div className="text-xs text-muted-foreground text-right">
                                            <div>
                                                {plan.tasks.length} 个任务
                                            </div>
                                            <div>
                                                最近：
                                                {plan.tasks[0]?.executedAt ??
                                                    "-"}
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
                            <Label className="text-base">选择备份任务</Label>
                            <p className="text-sm text-muted-foreground">
                                请选择要恢复的具体备份任务。
                            </p>
                        </div>
                        {!selectedPlan ? (
                            <div className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
                                请先选择备份计划。
                            </div>
                        ) : (
                            <div className="space-y-3">
                                {selectedPlan.tasks.map((task) => (
                                    <Card
                                        key={task.id}
                                        className={`cursor-pointer transition-all ${
                                            task.id === selectedTaskId
                                                ? "ring-2 ring-primary"
                                                : ""
                                        }`}
                                        onClick={() =>
                                            setSelectedTaskId(task.id)
                                        }
                                    >
                                        <CardContent className="p-4 flex items-center justify-between gap-4">
                                            <div>
                                                <p className="font-medium">
                                                    {task.executedAt}
                                                </p>
                                                <p className="text-xs text-muted-foreground">
                                                    {task.files.length} 项内容 ·{" "}
                                                    {task.size}
                                                </p>
                                            </div>
                                            <div className="text-xs text-muted-foreground text-right">
                                                {statusLabel(task.status)}
                                            </div>
                                        </CardContent>
                                    </Card>
                                ))}
                            </div>
                        )}
                    </div>
                );

            case 3:
                return (
                    <div className="space-y-4">
                        <div>
                            <Label className="text-base">选择备份文件</Label>
                            <p className="text-sm text-muted-foreground">
                                勾选需要恢复的文件或目录。
                            </p>
                        </div>
                        {!selectedTask ? (
                            <div className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
                                请先选择备份任务。
                            </div>
                        ) : (
                            <div className="space-y-2">
                                <div className="flex items-center space-x-2">
                                    <Checkbox
                                        id="select-all"
                                        checked={
                                            selectedTask.files.length > 0 &&
                                            selectedFiles.length ===
                                                selectedTask.files.length
                                        }
                                        onCheckedChange={(checked) => {
                                            if (checked === true) {
                                                setSelectedFiles(
                                                    selectedTask.files
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
                                {selectedTask.files.map((file, index) => (
                                    <div
                                        key={file}
                                        className="flex items-center gap-2 rounded-lg border p-3"
                                    >
                                        <Checkbox
                                            id={`file-${index}`}
                                            checked={selectedFiles.includes(
                                                file
                                            )}
                                            onCheckedChange={(checked) => {
                                                if (checked === true) {
                                                    setSelectedFiles((prev) =>
                                                        prev.includes(file)
                                                            ? prev
                                                            : [...prev, file]
                                                    );
                                                } else {
                                                    setSelectedFiles((prev) =>
                                                        prev.filter(
                                                            (item) =>
                                                                item !== file
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

            case 4:
                return (
                    <div className="space-y-4">
                        <div>
                            <Label className="text-base">选择恢复目标</Label>
                            <p className="text-sm text-muted-foreground">
                                设置恢复路径与文件冲突处理策略。
                            </p>
                        </div>
                        <div className="space-y-3">
                            <Label className="text-sm font-medium">
                                恢复位置
                            </Label>
                            <RadioGroup
                                value={restoreType}
                                onValueChange={(value) =>
                                    setRestoreType(
                                        value as "original" | "custom"
                                    )
                                }
                            >
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem
                                        value="original"
                                        id="restore-original"
                                    />
                                    <Label htmlFor="restore-original">
                                        恢复到原始位置
                                    </Label>
                                </div>
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem
                                        value="custom"
                                        id="restore-custom"
                                    />
                                    <Label htmlFor="restore-custom">
                                        恢复到自定义位置
                                    </Label>
                                </div>
                            </RadioGroup>
                        </div>
                        {restoreType === "custom" && (
                            <div className="space-y-2">
                                <Label className="text-sm font-medium">
                                    选择目标路径
                                </Label>
                                <DirectorySelector
                                    value={customPath}
                                    onChange={setCustomPath}
                                    placeholder="请选择恢复目标路径"
                                />
                            </div>
                        )}
                        <div className="space-y-3">
                            <Label className="text-sm font-medium">
                                文件冲突处理
                            </Label>
                            <RadioGroup
                                value={overwriteMode}
                                onValueChange={(value) =>
                                    setOverwriteMode(
                                        value as "skip" | "overwrite" | "rename"
                                    )
                                }
                            >
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem value="skip" id="skip" />
                                    <Label htmlFor="skip">跳过已存在文件</Label>
                                </div>
                                <div className="flex items-center space-x-2">
                                    <RadioGroupItem
                                        value="overwrite"
                                        id="overwrite"
                                    />
                                    <Label htmlFor="overwrite">
                                        覆盖已存在文件
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

            case 5:
                return (
                    <div className="space-y-4">
                        <div>
                            <Label className="text-base">确认与完成</Label>
                            <p className="text-sm text-muted-foreground">
                                检查信息后创建恢复任务。
                            </p>
                        </div>
                        <Card>
                            <CardContent className="p-4 space-y-2 text-sm">
                                <div className="flex justify-between">
                                    <span className="text-muted-foreground">
                                        备份计划
                                    </span>
                                    <span>
                                        {selectedPlan?.name ?? "未选择"}
                                    </span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-muted-foreground">
                                        备份任务
                                    </span>
                                    <span>
                                        {selectedTask?.executedAt ?? "未选择"}
                                    </span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-muted-foreground">
                                        文件数量
                                    </span>
                                    <span>{selectedFiles.length} 项</span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-muted-foreground">
                                        恢复路径
                                    </span>
                                    <span>
                                        {restoreType === "original"
                                            ? "原始位置"
                                            : customPath || "未设置"}
                                    </span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-muted-foreground">
                                        冲突策略
                                    </span>
                                    <span>{overwriteModeLabel}</span>
                                </div>
                            </CardContent>
                        </Card>
                        {selectedFiles.length > 0 && (
                            <Card>
                                <CardContent className="p-4 space-y-2 text-sm text-muted-foreground">
                                    {selectedFiles.map((file) => (
                                        <div
                                            key={file}
                                            className="flex items-center gap-2"
                                        >
                                            <Folder className="w-4 h-4 text-muted-foreground" />
                                            <span className="truncate">
                                                {file}
                                            </span>
                                        </div>
                                    ))}
                                </CardContent>
                            </Card>
                        )}
                        <div className="rounded-lg bg-blue-50 p-4 text-sm text-blue-800 dark:bg-blue-950 dark:text-blue-200">
                            恢复任务将在后台执行，可在任务列表查看进度。
                        </div>
                    </div>
                );

            default:
                return null;
        }
    };

    const handleNext = () => {
        if (currentStep < STEPS.length) {
            setCurrentStep((prev) => prev + 1);
        }
    };

    const handlePrevious = () => {
        if (currentStep > 1) {
            setCurrentStep((prev) => prev - 1);
        }
    };

    const handleComplete = () => {
        toast.success("恢复任务已创建");
        onComplete();
    };

    return (
        <div className={`${isMobile ? "p-4" : "p-6"} space-y-6`}>
            <div className="flex items-center gap-4">
                <Button variant="ghost" size="sm" onClick={onBack}>
                    <ArrowLeft className="w-4 h-4" />
                    {!isMobile && <span className="ml-2">返回</span>}
                </Button>
                <div>
                    <h1 className="text-xl font-semibold">创建恢复任务</h1>
                    <p className="text-sm text-muted-foreground">
                        按顺序完成恢复配置
                    </p>
                </div>
            </div>

            <div className="flex items-center justify-between">
                {STEPS.map((step, index) => (
                    <div key={step.number} className="flex items-center">
                        <div
                            className={`flex h-8 w-8 items-center justify-center rounded-full text-sm font-medium ${
                                currentStep >= step.number
                                    ? "bg-primary text-primary-foreground"
                                    : "bg-muted text-muted-foreground"
                            }`}
                        >
                            {currentStep > step.number ? (
                                <Check className="h-4 w-4" />
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
                        {index < STEPS.length - 1 && (
                            <div
                                className={`mx-4 h-px w-8 ${
                                    currentStep > step.number
                                        ? "bg-primary"
                                        : "bg-muted"
                                }`}
                            />
                        )}
                    </div>
                ))}
            </div>

            <Card>
                <CardContent className="p-6">{renderStep()}</CardContent>
            </Card>

            <div className="flex items-center justify-between">
                <Button
                    variant="outline"
                    onClick={handlePrevious}
                    disabled={currentStep === 1}
                >
                    <ChevronLeft className="mr-2 h-4 w-4" />
                    上一步
                </Button>
                <div className="flex gap-3">
                    <Button variant="outline" onClick={onBack}>
                        取消
                    </Button>
                    {currentStep < STEPS.length ? (
                        <Button onClick={handleNext} disabled={!canProceed()}>
                            下一步
                            <ChevronRight className="ml-2 h-4 w-4" />
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
