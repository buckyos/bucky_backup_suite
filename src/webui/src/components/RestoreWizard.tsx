import React, { useEffect, useMemo, useState } from "react";
import { Card, CardContent } from "./ui/card";
import { Button } from "./ui/button";
import { Label } from "./ui/label";
import { Checkbox } from "./ui/checkbox";
import { RadioGroup, RadioGroupItem } from "./ui/radio-group";
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
    Loader2,
} from "lucide-react";
import {
    Breadcrumb,
    BreadcrumbList,
    BreadcrumbItem,
    BreadcrumbLink,
    BreadcrumbPage,
    BreadcrumbSeparator,
} from "./ui/breadcrumb";
import {
    BackupPlanInfo,
    TaskInfo,
    TaskType,
    TaskState,
    ListTaskOrderBy,
    ListOrder,
    DirectoryPurpose,
} from "./utils/task_mgr";
import { TaskMgrHelper, taskManager } from "./utils/task_mgr_helper";

interface RestoreWizardProps {
    onBack: () => void;
    onComplete: () => void;
    data?: { planId?: string; taskId?: string };
}

interface BreadcrumbNode {
    label: string;
    path: string | null;
}

interface TaskFileEntry {
    id: string;
    label: string;
    fullPath: string;
    requestPath: string;
    isDirectory: boolean;
}

interface TargetDirectoryEntry {
    id: string;
    label: string;
    path: string;
}

const TARGET_ROOT_BREADCRUMB: BreadcrumbNode = {
    label: "目标目录",
    path: null,
};

type RawTaskFileEntry = string | Record<string, any>;

const ROOT_BREADCRUMB: BreadcrumbNode = { label: "备份内容", path: null };

export function RestoreWizard({
    onBack,
    onComplete,
    data,
}: RestoreWizardProps) {
    const isMobile = useMobile();
    const [cancelled, setCancelled] = useState(false);

    const [currentStep, setCurrentStep] = useState(
        (data && (data.taskId ? 3 : data.planId ? 2 : 1)) || 1
    );

    const [planList, setPlanList] = useState<BackupPlanInfo[] | null>(null);
    const [selectedPlanId, setSelectedPlanId] = useState(
        () => data?.planId ?? ""
    );
    const [plansLoading, setPlansLoading] = useState(true);
    const [plansError, setPlansError] = useState<string | null>(null);

    const [taskList, setTaskList] = useState<TaskInfo[] | null>(null);
    const [selectedTaskId, setSelectedTaskId] = useState(
        () => data?.taskId ?? ""
    );
    const [tasksLoading, setTasksLoading] = useState(false);
    const [tasksError, setTasksError] = useState<string | null>(null);

    const [fileEntries, setFileEntries] = useState<TaskFileEntry[]>([]);
    const [filesLoading, setFilesLoading] = useState(false);
    const [filesError, setFilesError] = useState<string | null>(null);

    const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
    const [breadcrumbs, setBreadcrumbs] = useState<BreadcrumbNode[]>([
        ROOT_BREADCRUMB,
    ]);
    const [targetBreadcrumbs, setTargetBreadcrumbs] = useState<
        BreadcrumbNode[]
    >([TARGET_ROOT_BREADCRUMB]);
    const [targetDirectories, setTargetDirectories] = useState<
        TargetDirectoryEntry[]
    >([]);
    const [targetLoading, setTargetLoading] = useState(false);
    const [targetError, setTargetError] = useState<string | null>(null);

    const [restoreType, setRestoreType] = useState<"original" | "custom">(
        "original"
    );
    const [customPath, setCustomPath] = useState("");
    const [overwriteMode, setOverwriteMode] = useState<
        "skip" | "overwrite" | "rename"
    >("skip");

    const steps = [
        {
            number: 1,
            title: "选择备份计划",
            description: "选择要恢复的备份计划",
        },
        {
            number: 2,
            title: "选择备份任务",
            description: "选择要使用的备份任务",
        },
        { number: 3, title: "选择备份文件", description: "勾选需要恢复的文件" },
        { number: 4, title: "选择恢复目标", description: "设置恢复目标路径" },
        { number: 5, title: "确认与完成", description: "检查设置并创建任务" },
    ];

    const currentBreadcrumb = breadcrumbs[breadcrumbs.length - 1];
    const currentPath = currentBreadcrumb?.path ?? null;
    const targetCurrentBreadcrumb =
        targetBreadcrumbs[targetBreadcrumbs.length - 1];
    const targetCurrentPath = targetCurrentBreadcrumb?.path ?? null;

    const selectedPlan = useMemo(
        () =>
            planList &&
            planList.find((plan) => plan.plan_id === selectedPlanId),
        [planList, selectedPlanId]
    );
    const selectedTask = useMemo(
        () =>
            taskList && taskList.find((task) => task.taskid === selectedTaskId),
        [taskList, selectedTaskId]
    );

    const loadPlanList = async () => {
        setPlansLoading(true);
        setPlansError(null);

        try {
            if (data?.planId) {
                setSelectedPlanId(data.planId);
                const plan = await taskManager.getBackupPlan(data.planId);
                if (plan) {
                    setPlanList([plan]);
                }
            } else {
                const planIds = await taskManager.listBackupPlans();
                if (cancelled) return;
                const plans = await Promise.all(
                    planIds.map((id) => taskManager.getBackupPlan(id))
                );
                setPlanList(
                    plans.filter((p): p is BackupPlanInfo => p !== null)
                );
                if (data?.taskId) {
                    const plan = plans.find((p) => p.plan_id === data.taskId);
                    if (plan) {
                        setSelectedPlanId(plan.plan_id);
                        setPlanList([plan]);
                    }
                }
            }
        } catch (error) {
            if (cancelled) return;

            setPlansError(formatErrorMessage(error));
        } finally {
            if (!cancelled) {
                setPlansLoading(false);
            }
        }
    };

    const loadTaskList = async () => {
        setTasksLoading(true);
        setTasksError(null);
        try {
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
        } catch (error) {
            if (cancelled) return;
            setTasksError(formatErrorMessage(error));
        } finally {
            if (!cancelled) {
                setTasksLoading(false);
            }
        }
    };

    const loadFiles = async () => {
        setFilesLoading(true);
        setFilesError(null);
        try {
            const files = await taskManager.listFilesInTask(
                selectedTaskId,
                currentPath
            );
            const entries = normalizeTaskFileEntries(
                Array.isArray(files) ? files : [],
                currentPath
            );
            setFileEntries(entries);
        } catch (error) {
            if (cancelled) return;
            setFilesError(formatErrorMessage(error));
        } finally {
            if (!cancelled) {
                setFilesLoading(false);
            }
        }
    };

    const init = async () => {
        Promise.all([loadPlanList(), loadTaskList()]).catch((err) => {
            console.error("Failed to initialize RestoreWizard:", err);
        });
    };

    useEffect(() => {
        init();
        return () => {
            setCancelled(true);
        };
    }, []);

    useEffect(() => {
        if (!data?.taskId) {
            setSelectedTaskId("");
            setTaskList(null);
            setSelectedFiles([]);
            loadTaskList();
        }
    }, [selectedPlanId]);

    useEffect(() => {
        setSelectedFiles([]);
        setBreadcrumbs([ROOT_BREADCRUMB]);
    }, [selectedTaskId]);

    useEffect(() => {
        loadFiles();
    }, [breadcrumbs, selectedTaskId]);

    useEffect(() => {
        if (restoreType !== "custom") return;
        let ignore = false;

        const loadDirectories = async () => {
            setTargetLoading(true);
            setTargetError(null);
            try {
                const dirs = await taskManager.listDirChildren(
                    targetCurrentPath ?? undefined,
                    DirectoryPurpose.RESTORE_TARGET,
                    { only_dirs: true }
                );

                if (ignore) return;

                const entries = (Array.isArray(dirs) ? dirs : []).map(
                    (dir, index) => {
                        const label = dir.name || "未命名";
                        const nextPath = buildRequestPath(
                            targetCurrentPath,
                            dir.name || label
                        );
                        const safePath = nextPath || label;
                        return {
                            id: `${safePath}-${index}`,
                            label,
                            path: safePath,
                        };
                    }
                );
                setTargetDirectories(entries);
            } catch (error) {
                if (ignore) return;
                setTargetError(formatErrorMessage(error));
                setTargetDirectories([]);
            } finally {
                if (!ignore) {
                    setTargetLoading(false);
                }
            }
        };

        loadDirectories();

        return () => {
            ignore = true;
        };
    }, [restoreType, targetCurrentPath]);

    useEffect(() => {
        if (restoreType !== "custom") return;
        const nextPath = targetCurrentPath ?? "";
        if (nextPath === customPath) return;
        setCustomPath(nextPath);
    }, [restoreType, targetCurrentPath, customPath]);

    useEffect(() => {
        if (
            !plansLoading &&
            selectedPlanId &&
            !(
                planList &&
                planList.find((plan) => plan.plan_id === selectedPlanId)
            )
        ) {
            setSelectedPlanId("");
        }
    }, [planList, plansLoading, selectedPlanId]);

    const overwriteModeLabel =
        overwriteMode === "skip"
            ? "跳过已存在文件"
            : overwriteMode === "overwrite"
            ? "覆盖已存在文件"
            : "重命名新文件";
    const canProceed = () => {
        console.log("canProceed check for step", currentStep);
        switch (currentStep) {
            case 1:
                return selectedPlanId !== "" && !plansLoading;
            case 2:
                return selectedTaskId !== "" && !tasksLoading;
            case 3:
                return selectedFiles.length > 0 && !filesLoading;
            case 4:
                return (
                    (restoreType === "original" ||
                        customPath.trim().length > 0) &&
                    !filesLoading
                );
            case 5:
                return true;
            default:
                return false;
        }
    };

    const handleNext = () => {
        if (currentStep < steps.length) {
            setCurrentStep((prev) => prev + 1);
        }
    };

    const handlePrevious = () => {
        if (currentStep > 1) {
            setCurrentStep((prev) => prev - 1);
        }
    };

    const handleComplete = async () => {
        console.log("handleComplete");
        try {
            await taskManager.createRestoreTask(
                selectedPlanId,
                taskList!.find(task => task.taskid === selectedTaskId)!.checkpoint_id,
                customPath,
                overwriteMode === "overwrite",
                selectedFiles[0]
            );
            onComplete();

            console.log("handleComplete return");
            // toast.success("恢复任务已创建");
        } catch (error) {
            console.log("handleComplete error", error);
            toast.error("创建恢复任务失败：" + formatErrorMessage(error));
            return;
        }
    };

    const handleToggleSelectAll = (checked: boolean | "indeterminate") => {
        if (checked === true) {
            setSelectedFiles((prev) => {
                const existing = new Set(prev);
                fileEntries.forEach((entry) =>
                    existing.add(entry.fullPath || entry.label)
                );
                return Array.from(existing);
            });
        } else if (checked === false) {
            setSelectedFiles((prev) =>
                prev.filter(
                    (item) =>
                        !fileEntries.some(
                            (entry) =>
                                entry.fullPath === item || entry.label === item
                        )
                )
            );
        }
    };

    const handleEnterDirectory = (entry: TaskFileEntry) => {
        if (!entry.isDirectory) return;
        const nextPath = entry.requestPath;
        setBreadcrumbs((prev) => [
            ...prev,
            { label: entry.label, path: nextPath },
        ]);
    };

    const handleBreadcrumbClick = (index: number) => {
        setBreadcrumbs((prev) => prev.slice(0, index + 1));
    };

    const handleTargetBreadcrumbClick = (index: number) => {
        setTargetBreadcrumbs((prev) => prev.slice(0, index + 1));
    };

    const handleTargetDirectoryNavigate = (
        entry: TargetDirectoryEntry
    ): void => {
        if (entry.path === targetCurrentPath) return;
        setTargetBreadcrumbs((prev) => [
            ...prev,
            { label: entry.label, path: entry.path },
        ]);
    };

    const renderPlansStep = () => (
        <div className="space-y-4">
            <div>
                <Label className="text-base">选择备份计划</Label>
                <p className="text-sm text-muted-foreground">
                    请选择需要恢复的备份计划。
                </p>
            </div>

            {plansLoading ? (
                <div className="flex items-center gap-2 rounded-lg border p-4 text-sm text-muted-foreground">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    正在加载备份计划...
                </div>
            ) : plansError ? (
                <div className="rounded-lg border border-destructive/60 bg-destructive/5 p-4 text-sm text-destructive">
                    加载备份计划失败：{plansError}
                </div>
            ) : planList!.length === 0 ? (
                <div className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
                    暂无可用的备份计划。
                </div>
            ) : (
                <div className="space-y-3">
                    {planList!.map((plan) => (
                        <Card
                            key={plan.plan_id}
                            className={`cursor-pointer transition-all ${
                                selectedPlanId === plan.plan_id
                                    ? "ring-2 ring-primary"
                                    : ""
                            }`}
                            onClick={() => setSelectedPlanId(plan.plan_id)}
                        >
                            <CardContent className="flex items-start justify-between gap-4 p-4">
                                <div className="space-y-2">
                                    <div className="flex items-center gap-2">
                                        <div
                                            className={`flex h-4 w-4 items-center justify-center rounded-full border-2 ${
                                                selectedPlanId === plan.plan_id
                                                    ? "border-primary bg-primary"
                                                    : "border-muted-foreground"
                                            }`}
                                        >
                                            {selectedPlanId ===
                                                plan.plan_id && (
                                                <Check className="h-3 w-3 text-primary-foreground" />
                                            )}
                                        </div>
                                        <p className="font-medium">
                                            {plan.title || plan.plan_id}
                                        </p>
                                    </div>
                                    {plan.description && (
                                        <p className="text-xs text-muted-foreground">
                                            {plan.description}
                                        </p>
                                    )}
                                    <div className="flex flex-wrap gap-3 text-xs text-muted-foreground">
                                        <span>来源：{plan.source}</span>
                                        <span>目标：{plan.target}</span>
                                    </div>
                                </div>
                                <div className="text-right text-xs text-muted-foreground">
                                    <div>已备份 {plan.total_backup} 次</div>
                                    <div>
                                        最近一次：
                                        {plan.last_run_time
                                            ? new Date(
                                                  plan.last_run_time
                                              ).toLocaleString()
                                            : "未执行"}
                                    </div>
                                </div>
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}
        </div>
    );
    const renderTasksStep = () => (
        <div className="space-y-4">
            <div>
                <Label className="text-base">选择备份任务</Label>
                <p className="text-sm text-muted-foreground">
                    请选择要用于恢复的备份任务。
                </p>
            </div>

            {!selectedPlan ? (
                <div className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
                    请先选择备份计划。
                </div>
            ) : tasksLoading ? (
                <div className="flex items-center gap-2 rounded-lg border p-4 text-sm text-muted-foreground">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    正在加载备份任务...
                </div>
            ) : tasksError ? (
                <div className="rounded-lg border border-destructive/60 bg-destructive/5 p-4 text-sm text-destructive">
                    加载备份任务失败：{tasksError}
                </div>
            ) : !taskList || taskList.length === 0 ? (
                <div className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
                    当前备份计划尚无可用任务。
                </div>
            ) : (
                <div className="space-y-3">
                    {taskList!.map((task) => (
                        <Card
                            key={task.taskid}
                            className={`cursor-pointer transition-all ${
                                selectedTaskId === task.taskid
                                    ? "ring-2 ring-primary"
                                    : ""
                            }`}
                            onClick={() => setSelectedTaskId(task.taskid)}
                        >
                            <CardContent className="flex items-center justify-between gap-4 p-4">
                                <div className="flex items-start gap-3">
                                    <FileText className="h-4 w-4 text-muted-foreground" />
                                    <div>
                                        <p className="font-medium">
                                            {task.name || task.taskid}
                                        </p>
                                        <p className="text-xs text-muted-foreground">
                                            创建时间：
                                            {formatDateTime(task.create_time)}
                                        </p>
                                        <p className="text-xs text-muted-foreground">
                                            大小：
                                            {TaskMgrHelper.taskTotalStr(task)} ·
                                            条目：
                                            {task.item_count}
                                        </p>
                                    </div>
                                </div>
                                <div className="text-right text-xs text-muted-foreground">
                                    <div>
                                        状态：{translateTaskState(task.state)}
                                    </div>
                                    <div>检查点：{task.checkpoint_id}</div>
                                </div>
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}
        </div>
    );

    const renderFilesStep = () => {
        const allSelected =
            fileEntries.length > 0 &&
            fileEntries.every((entry) =>
                selectedFiles.includes(entry.fullPath)
            );
        const someSelected = fileEntries.some((entry) =>
            selectedFiles.includes(entry.fullPath)
        );
        const selectAllState =
            fileEntries.length === 0
                ? false
                : allSelected
                ? true
                : someSelected
                ? "indeterminate"
                : false;

        return (
            <div className="space-y-4">
                <div>
                    <Label className="text-base">选择备份文件</Label>
                    <p className="text-sm text-muted-foreground">
                        使用面包屑逐级浏览目录，勾选需要恢复的文件或目录。
                    </p>
                </div>

                {!selectedTask ? (
                    <div className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
                        请先选择备份任务。
                    </div>
                ) : (
                    <>
                        <Breadcrumb>
                            <BreadcrumbList>
                                {breadcrumbs.map((item, index) => (
                                    <React.Fragment
                                        key={`${item.label}-${index}`}
                                    >
                                        <BreadcrumbItem>
                                            {index ===
                                            breadcrumbs.length - 1 ? (
                                                <BreadcrumbPage>
                                                    {item.label}
                                                </BreadcrumbPage>
                                            ) : (
                                                <BreadcrumbLink
                                                    className="cursor-pointer"
                                                    onClick={() =>
                                                        handleBreadcrumbClick(
                                                            index
                                                        )
                                                    }
                                                >
                                                    {item.label}
                                                </BreadcrumbLink>
                                            )}
                                        </BreadcrumbItem>
                                        {index < breadcrumbs.length - 1 && (
                                            <BreadcrumbSeparator />
                                        )}
                                    </React.Fragment>
                                ))}
                            </BreadcrumbList>
                        </Breadcrumb>

                        <div className="flex items-center space-x-2">
                            <Checkbox
                                id="select-all"
                                checked={selectAllState}
                                onCheckedChange={handleToggleSelectAll}
                                disabled={fileEntries.length === 0}
                            />
                            <Label htmlFor="select-all" className="font-medium">
                                全选当前目录
                            </Label>
                        </div>

                        {filesLoading ? (
                            <div className="flex items-center gap-2 rounded-lg border p-4 text-sm text-muted-foreground">
                                <Loader2 className="h-4 w-4 animate-spin" />
                                正在加载文件列表...
                            </div>
                        ) : filesError ? (
                            <div className="rounded-lg border border-destructive/60 bg-destructive/5 p-4 text-sm text-destructive">
                                加载文件列表失败：{filesError}
                            </div>
                        ) : fileEntries.length === 0 ? (
                            <div className="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground">
                                当前目录下没有发现文件或目录。
                            </div>
                        ) : (
                            <div className="space-y-2">
                                {fileEntries.map((entry) => (
                                    <div
                                        key={entry.id}
                                        className="flex items-center gap-2 rounded-lg border p-3 hover:bg-muted/40"
                                    >
                                        <Checkbox
                                            checked={selectedFiles.includes(
                                                entry.fullPath
                                            )}
                                            onCheckedChange={(checked) => {
                                                if (checked === true) {
                                                    setSelectedFiles((prev) =>
                                                        prev.includes(
                                                            entry.fullPath
                                                        )
                                                            ? prev
                                                            : [
                                                                  ...prev,
                                                                  entry.fullPath,
                                                              ]
                                                    );
                                                } else if (checked === false) {
                                                    setSelectedFiles((prev) =>
                                                        prev.filter(
                                                            (item) =>
                                                                item !==
                                                                entry.fullPath
                                                        )
                                                    );
                                                }
                                            }}
                                        />
                                        {entry.isDirectory ? (
                                            <Folder className="h-4 w-4 text-muted-foreground" />
                                        ) : (
                                            <FileText className="h-4 w-4 text-muted-foreground" />
                                        )}
                                        <button
                                            type="button"
                                            className="flex-1 text-left"
                                            onClick={() =>
                                                handleEnterDirectory(entry)
                                            }
                                            disabled={!entry.isDirectory}
                                        >
                                            <span
                                                className={`${
                                                    entry.isDirectory
                                                        ? "text-primary"
                                                        : ""
                                                }`}
                                            >
                                                {entry.label}
                                            </span>
                                            <span className="block text-xs text-muted-foreground">
                                                {entry.fullPath}
                                            </span>
                                        </button>
                                    </div>
                                ))}
                            </div>
                        )}
                    </>
                )}
            </div>
        );
    };
    const renderSettingsStep = () => (
        <div className="space-y-4">
            <div>
                <Label className="text-base">选择恢复目标</Label>
                <p className="text-sm text-muted-foreground">
                    设置恢复路径和文件冲突处理策略。
                </p>
            </div>

            <div className="space-y-3">
                <Label className="text-sm font-medium">恢复位置</Label>
                <RadioGroup
                    value={restoreType}
                    onValueChange={(value) =>
                        setRestoreType(value as "original" | "custom")
                    }
                >
                    <div className="flex items-center space-x-2">
                        <RadioGroupItem
                            value="original"
                            id="restore-original"
                        />
                        <Label htmlFor="restore-original">恢复到原始位置</Label>
                    </div>
                    <div className="flex items-center space-x-2">
                        <RadioGroupItem value="custom" id="restore-custom" />
                        <Label htmlFor="restore-custom">恢复到自定义位置</Label>
                    </div>
                </RadioGroup>
            </div>

            {restoreType === "custom" && (
                <div className="space-y-3">
                    <div className="flex items-center justify-between">
                        <Label className="text-sm font-medium">
                            选择目标路径
                        </Label>
                    </div>

                    <div className="rounded-md border">
                        <div className="border-b bg-muted/50 px-3 py-2">
                            <Breadcrumb>
                                <BreadcrumbList>
                                    {targetBreadcrumbs.map((item, index) => (
                                        <React.Fragment
                                            key={`${item.label}-${index}`}
                                        >
                                            <BreadcrumbItem>
                                                {index ===
                                                targetBreadcrumbs.length - 1 ? (
                                                    <BreadcrumbPage>
                                                        {item.label}
                                                    </BreadcrumbPage>
                                                ) : (
                                                    <BreadcrumbLink
                                                        className="cursor-pointer"
                                                        onClick={() =>
                                                            handleTargetBreadcrumbClick(
                                                                index
                                                            )
                                                        }
                                                    >
                                                        {item.label}
                                                    </BreadcrumbLink>
                                                )}
                                            </BreadcrumbItem>
                                            {index <
                                                targetBreadcrumbs.length -
                                                    1 && (
                                                <BreadcrumbSeparator />
                                            )}
                                        </React.Fragment>
                                    ))}
                                </BreadcrumbList>
                            </Breadcrumb>
                        </div>

                        <div className="p-3 space-y-2">
                            {targetLoading ? (
                                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                                    <Loader2 className="h-4 w-4 animate-spin" />
                                    正在加载目录...
                                </div>
                            ) : targetError ? (
                                <div className="text-sm text-destructive">
                                    {targetError}
                                </div>
                            ) : targetDirectories.length === 0 ? (
                                <div className="text-sm text-muted-foreground">
                                    该目录暂无子目录
                                </div>
                            ) : (
                                targetDirectories.map((entry) => (
                                    <button
                                        key={entry.id}
                                        type="button"
                                        onClick={() =>
                                            handleTargetDirectoryNavigate(entry)
                                        }
                                        className="flex w-full items-center justify-between rounded-md border border-transparent px-3 py-2 text-left text-sm transition-colors hover:border-accent hover:bg-accent/50"
                                    >
                                        <span className="flex items-center gap-2 overflow-hidden">
                                            <Folder className="h-4 w-4 text-muted-foreground" />
                                            <span className="truncate">
                                                {entry.label}
                                            </span>
                                        </span>
                                        <ChevronRight className="h-4 w-4 text-muted-foreground" />
                                    </button>
                                ))
                            )}
                        </div>
                    </div>

                    <div className="text-xs text-muted-foreground">
                        当前选择：{customPath || "未选择"}
                    </div>
                </div>
            )}

            <div className="space-y-3">
                <Label className="text-sm font-medium">文件冲突处理</Label>
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
                        <RadioGroupItem value="overwrite" id="overwrite" />
                        <Label htmlFor="overwrite">覆盖已存在文件</Label>
                    </div>
                    <div className="flex items-center space-x-2">
                        <RadioGroupItem value="rename" id="rename" />
                        <Label htmlFor="rename">重命名新文件</Label>
                    </div>
                </RadioGroup>
            </div>
        </div>
    );

    const renderConfirmStep = () => (
        <div className="space-y-4">
            <div>
                <Label className="text-base">确认恢复信息</Label>
                <p className="text-sm text-muted-foreground">
                    请确认以下配置信息，确保恢复任务符合预期。
                </p>
            </div>

            <Card>
                <CardContent className="space-y-2 p-4 text-sm">
                    <div className="flex justify-between">
                        <span className="text-muted-foreground">备份计划</span>
                        <span>{selectedPlan?.title ?? "未选择"}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-muted-foreground">备份任务</span>
                        <span>{selectedTask?.name ?? "未选择"}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-muted-foreground">检查点</span>
                        <span>{selectedTask?.checkpoint_id ?? "-"}</span>
                    </div>
                    <div className="flex justify之间">
                        <span className="text-muted-foreground">文件数量</span>
                        <span>{selectedFiles.length} 项</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-muted-foreground">恢复路径</span>
                        <span>
                            {restoreType === "original"
                                ? "原始位置"
                                : customPath || "未设置"}
                        </span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-muted-foreground">冲突策略</span>
                        <span>{overwriteModeLabel}</span>
                    </div>
                </CardContent>
            </Card>

            {selectedFiles.length > 0 && (
                <Card>
                    <CardContent className="space-y-2 p-4 text-sm text-muted-foreground">
                        <div className="flex items-center gap-2 text-foreground">
                            <HardDrive className="h-4 w-4" />
                            即将恢复的文件
                        </div>
                        {selectedFiles.map((file) => (
                            <div key={file} className="flex items-center gap-2">
                                <Folder className="h-4 w-4 text-muted-foreground" />
                                <span className="truncate">{file}</span>
                            </div>
                        ))}
                    </CardContent>
                </Card>
            )}

            <div className="rounded-lg bg-blue-50 p-4 text-sm text-blue-800 dark:bg-blue-950 dark:text-blue-200">
                恢复任务将在后台执行，您可以在任务列表中查看进度。
            </div>
        </div>
    );

    const renderStepContent = () => {
        switch (currentStep) {
            case 1:
                return renderPlansStep();
            case 2:
                return renderTasksStep();
            case 3:
                return renderFilesStep();
            case 4:
                return renderSettingsStep();
            case 5:
                return renderConfirmStep();
            default:
                return null;
        }
    };

    return (
        <div className={`${isMobile ? "p-4" : "p-6"} space-y-6`}>
            <div className="flex items-center gap-4">
                <Button variant="ghost" size="sm" onClick={onBack}>
                    <ArrowLeft className="h-4 w-4" />
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
                {steps.map((step, index) => (
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
                        {index < steps.length - 1 && (
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
                <CardContent className="p-6">{renderStepContent()}</CardContent>
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
                    {currentStep < steps.length ? (
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
function normalizeTaskFileEntries(
    rawEntries: Array<{
        name: string;
        len: number;
        create_time: number;
        update_time: number;
        is_dir: boolean;
    }>,
    currentPath: string | null
): TaskFileEntry[] {
    return rawEntries.map((entry, index) => {
        const raw = entry.name || "";
        const trimmed = trimTrailingSlash(raw);
        const isDirectory = Boolean(entry.is_dir);
        const label =
            getLastPathSegment(trimmed) ||
            trimmed ||
            (isDirectory ? "目录" : "文件");
        const requestPath = buildRequestPath(currentPath, trimmed || label);
        const fullPath = normalizeDisplayPath(requestPath || trimmed || label);
        return {
            id: `${fullPath || label}-${index}`,
            label,
            fullPath,
            requestPath: requestPath || fullPath || label,
            isDirectory,
        };
    });
}

function buildRequestPath(base: string | null, segment: string): string {
    const normalizedSegment = normalizeSegment(segment);
    if (!base || base.length === 0) {
        return normalizedSegment;
    }
    if (isAbsolutePath(normalizedSegment)) {
        return normalizedSegment;
    }
    const normalizedBase = normalizeSegment(base);
    if (normalizedBase === "") {
        return normalizedSegment;
    }
    if (normalizedBase === "/") {
        return `/${normalizedSegment}`;
    }
    return `${normalizedBase}${
        normalizedBase.endsWith("/") ? "" : "/"
    }${normalizedSegment}`;
}

function normalizeSegment(input: string): string {
    if (!input) return "";
    let result = input.replace(/\\/g, "/");
    if (/^[A-Za-z]:$/.test(result)) {
        return result;
    }
    if (result.includes("://")) {
        return trimTrailingSlash(result);
    }
    if (result === "/") {
        return "/";
    }
    return trimTrailingSlash(result);
}

function normalizeDisplayPath(input: string): string {
    if (!input) return "";
    let result = input.replace(/\\/g, "/");
    if (result !== "/" && result.endsWith("/")) {
        result = result.slice(0, -1);
    }
    return result;
}

function trimTrailingSlash(input: string): string {
    if (!input) return "";
    let result = input.trim();
    while (
        result.length > 1 &&
        (result.endsWith("/") || result.endsWith("\\"))
    ) {
        result = result.slice(0, -1);
    }
    return result;
}

function getLastPathSegment(path: string): string | null {
    if (!path) return null;
    const normalized = path.replace(/\\/g, "/");
    const segments = normalized.split("/").filter(Boolean);
    if (segments.length === 0) return path;
    return segments[segments.length - 1];
}

function isAbsolutePath(path: string): boolean {
    return (
        /^[A-Za-z]:/.test(path) || path.startsWith("/") || path.includes("://")
    );
}

function formatErrorMessage(error: unknown): string {
    if (error instanceof Error) {
        return error.message;
    }
    if (typeof error === "string") {
        return error;
    }
    return "未知错误";
}

function formatDateTime(timestamp?: number): string {
    if (!timestamp) return "--";
    try {
        return new Date(timestamp).toLocaleString();
    } catch {
        return String(timestamp);
    }
}

function translateTaskState(state: TaskState): string {
    switch (state) {
        case TaskState.RUNNING:
            return "运行中";
        case TaskState.PENDING:
            return "排队中";
        case TaskState.PAUSED:
            return "已暂停";
        case TaskState.DONE:
            return "已完成";
        case TaskState.FAILED:
            return "失败";
        default:
            return state;
    }
}
