import React, { useEffect, useMemo, useState } from "react";
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { ScrollArea } from "./ui/scroll-area";
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
import {
    ChevronLeft,
    Clock,
    Play,
    Pause,
    Square,
    Download,
    CheckCircle,
    AlertTriangle,
    XCircle,
    File,
    Hash,
    Folder,
} from "lucide-react";
import {
    BackupLog,
    BackupPlanInfo,
    ListOrder,
    TaskInfo,
    TaskState,
    TaskType,
} from "./utils/task_mgr";
import { TaskMgrHelper, taskManager } from "./utils/task_mgr_helper";
import { Translations } from "./i18n";

interface TaskDetailProps {
    task: TaskInfo;
    plan?: BackupPlanInfo;
    onBack: () => void;
}

interface FileEntry {
    name: string;
    size: number;
    createTime: number;
    updateTime: number;
    isDirectory: boolean;
    fullPath: string;
}

interface ChunkEntry {
    chunkId: string;
    sequence: string;
    size: number;
    status: string;
}

interface BreadcrumbEntry {
    label: string;
    requestPath: string | null;
    fullPath: string;
    isFile: boolean;
    filePath?: string;
}

const joinPath = (base: string, segment: string): string => {
    const normalizedBase = base.replace(/\\/g, "/").replace(/\/+$/, "");
    const normalizedSegment = segment.replace(/\\/g, "/").replace(/^\/+/, "");
    if (!normalizedBase) {
        return normalizedSegment;
    }
    if (!normalizedSegment) {
        return normalizedBase;
    }
    return `${normalizedBase}/${normalizedSegment}`;
};

export function TaskDetail({ task, plan, onBack }: TaskDetailProps) {
    const { t } = useLanguage();
    const isMobile = useMobile();
    const [taskPlan, setTaskPlan] = useState<BackupPlanInfo | undefined>(plan);
    const taskRoot = useMemo(() => {
        const root = (task as TaskInfo & { root?: string }).root ?? "";
        return root.replace(/\\/g, "/");
    }, [task]);
    const rootLabel = taskRoot || "根目录";
    const [viewedPath, setViewedPath] = useState<string>(rootLabel);

    useEffect(() => {
        if (!plan) {
            taskManager.getBackupPlan(task.owner_plan_id).then((p) => {
                setTaskPlan(p);
            });
        }
    }, []);

    const formatLogMessage = (log: BackupLog) => {
        const { params } = log;
        if (params === null || params === undefined) {
            return "";
        }
        if (typeof params === "string") {
            return params;
        }
        try {
            return JSON.stringify(params);
        } catch {
            return String(params);
        }
    };

    const getStatusIcon = (status: string) => {
        switch (status) {
            case "completed":
                return <CheckCircle className="w-4 h-4 text-green-500" />;
            case "failed":
                return <XCircle className="w-4 h-4 text-red-500" />;
            case "running":
                return <Download className="w-4 h-4 text-blue-500" />;
            default:
                return <AlertTriangle className="w-4 h-4 text-yellow-500" />;
        }
    };

    return (
        <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-4`}>
            {/* 头部 */}
            <div className="flex items-center gap-4">
                <Button
                    variant="ghost"
                    size="sm"
                    onClick={onBack}
                    className="gap-2"
                >
                    <ChevronLeft className="w-4 h-4" />
                    {t.common.back}
                </Button>
                <div className="flex-1">
                    <h1 className="mb-2">{task.name}</h1>
                    <p className="text-muted-foreground text-sm">
                        {taskPlan?.title || "-"}
                    </p>
                </div>
            </div>
            {/* 任务概览 */}
            <TaskSummaryCard
                task={task}
                t={t}
                isMobile={isMobile}
                plan={taskPlan}
            />
            {/* 详细信息标签页 */}
            <Tabs defaultValue="files" className="space-y-4">
                <TabsList className="grid w-full grid-cols-2">
                    <TabsTrigger value="files">文件列表</TabsTrigger>
                    <TabsTrigger value="logs">日志</TabsTrigger>
                </TabsList>

                <TabsContent value="files" className="space-y-4">
                    <Card>
                        <CardHeader>
                            <CardTitle className="text-base">
                                文件列表
                            </CardTitle>
                            <CardDescription>{viewedPath}</CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <FileListBreadcrumbs
                                task={task}
                                taskRoot={taskRoot}
                                rootLabel={rootLabel}
                                onBreadcrumbChange={(currentBreadcrumb) => {
                                    setViewedPath(
                                        currentBreadcrumb?.fullPath || rootLabel
                                    );
                                }}
                            />
                        </CardContent>
                    </Card>
                </TabsContent>

                <TabsContent value="logs" className="space-y-4">
                    <Card>
                        <CardHeader>
                            <CardTitle className="text-base">
                                执行日志
                            </CardTitle>
                            <CardDescription>
                                任务执行的详细日志记录
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <LogsTabContent task={task} />
                        </CardContent>
                    </Card>
                </TabsContent>
            </Tabs>
        </div>
    );
}

function TaskSummaryCard({
    task,
    t,
    isMobile,
    plan,
}: {
    task: TaskInfo;
    t: Translations;
    isMobile: boolean;
    plan?: BackupPlanInfo;
}) {
    const taskProgress = TaskMgrHelper.taskProgress(task);
    const getStatusBadge = (status: TaskState) => {
        switch (status) {
            case TaskState.RUNNING:
                return (
                    <Badge className="bg-blue-100 text-blue-800 text-xs">
                        {t.tasks.running}
                    </Badge>
                );
            case TaskState.DONE:
                return (
                    <Badge className="bg-green-100 text-green-800 text-xs">
                        {t.tasks.completed}
                    </Badge>
                );
            case TaskState.PAUSED:
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                        {t.tasks.paused}
                    </Badge>
                );
            case TaskState.PAUSED:
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                        {t.tasks.paused}
                    </Badge>
                );
            case TaskState.FAILED:
                return (
                    <Badge className="bg-red-100 text-red-800 text-xs">
                        {t.tasks.failed}
                    </Badge>
                );
            case TaskState.PAUSING:
                return (
                    <Badge className="bg-yellow-100 text-yellow-800 text-xs">
                        暂停中...
                    </Badge>
                );
            case TaskState.PENDING:
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

    return (
        <Card>
            <CardHeader>
                <div className="flex items-center justify-between">
                    <div className="flex items-center gap-3">
                        <CardTitle className="text-lg">任务概览</CardTitle>
                        {getStatusBadge(task.state)}
                    </div>
                    <div className="flex gap-2">
                        {task.state === TaskState.RUNNING && (
                            <Button
                                variant="outline"
                                size="sm"
                                className="gap-1"
                            >
                                <Pause className="w-3 h-3" />
                                {t.tasks.pause}
                            </Button>
                        )}
                        {task.state === TaskState.PAUSED && (
                            <Button
                                variant="outline"
                                size="sm"
                                className="gap-1"
                            >
                                <Play className="w-3 h-3" />
                                {t.tasks.resume}
                            </Button>
                        )}
                    </div>
                </div>
            </CardHeader>
            <CardContent>
                {task.state === TaskState.RUNNING && (
                    <div className="space-y-3 mb-4">
                        <div className="flex justify-between text-sm">
                            <span>进度: {taskProgress}%</span>
                            <span>{TaskMgrHelper.taskSpeedStr(task)}</span>
                        </div>
                        <Progress value={taskProgress} className="h-3" />
                        <div className="flex justify-between text-sm text-muted-foreground">
                            <span>
                                {TaskMgrHelper.formatSize(task.completed_size)}{" "}
                                / {TaskMgrHelper.formatSize(task.total_size)}
                            </span>
                            <span>{TaskMgrHelper.taskRemainingStr(task)}</span>
                        </div>
                    </div>
                )}

                <div
                    className={`grid ${
                        isMobile ? "grid-cols-1" : "grid-cols-2"
                    } gap-4 text-sm`}
                >
                    <div>
                        <p className="text-muted-foreground mb-1">创建时间</p>
                        <p className="flex items-center gap-2">
                            <Clock className="w-4 h-4" />
                            {task.create_time
                                ? new Date(task.create_time).toLocaleString()
                                : "-"}
                        </p>
                    </div>
                    <div>
                        <p className="text-muted-foreground mb-1">数据大小</p>
                        <p>
                            {TaskMgrHelper.formatSize(task.completed_size)} /{" "}
                            {TaskMgrHelper.formatSize(task.total_size)}
                        </p>
                    </div>
                    <div>
                        <p className="text-muted-foreground mb-1">任务类型</p>
                        <p>
                            {task.task_type === TaskType.BACKUP
                                ? t.tasks.backup
                                : t.tasks.restore}
                        </p>
                    </div>
                    <div>
                        <p className="text-muted-foreground mb-1">所属计划</p>
                        <p>{plan?.title || "-"}</p>
                    </div>
                </div>

                {task.error && (
                    <div className="mt-4 p-3 bg-red-50 text-red-700 rounded-md text-sm">
                        <p className="font-medium mb-1">错误信息</p>
                        <p>{task.error}</p>
                    </div>
                )}
            </CardContent>
        </Card>
    );
}

function FileListBreadcrumbs({
    task,
    taskRoot,
    rootLabel,
    onBreadcrumbChange,
}: {
    task: TaskInfo;
    taskRoot: string;
    rootLabel: string;
    onBreadcrumbChange?: (current: BreadcrumbEntry | null) => void;
}) {
    const [breadcrumbs, setBreadcrumbs] = useState<BreadcrumbEntry[]>([]);
    const [fileEntries, setFileEntries] = useState<FileEntry[]>([]);
    const [chunkEntries, setChunkEntries] = useState<ChunkEntry[]>([]);
    const [contentLoading, setContentLoading] = useState(false);
    const [contentError, setContentError] = useState<string | null>(null);
    const currentBreadcrumb =
        breadcrumbs.length > 0 ? breadcrumbs[breadcrumbs.length - 1] : null;
    const rootBreadcrumb = {
        label: rootLabel,
        requestPath: null,
        fullPath: taskRoot,
        isFile: false,
    } as BreadcrumbEntry;
    const breadcrumbsForRender =
        breadcrumbs.length > 0 ? breadcrumbs : [rootBreadcrumb];

    useEffect(() => {
        setBreadcrumbs([rootBreadcrumb]);
        onBreadcrumbChange?.(rootBreadcrumb);
    }, [rootLabel, taskRoot, task.taskid]);

    useEffect(() => {
        if (!task.taskid || breadcrumbs.length === 0) {
            return;
        }

        const activeCrumb = breadcrumbs[breadcrumbs.length - 1];
        let cancelled = false;

        const loadContent = async () => {
            setContentLoading(true);
            setContentError(null);
            try {
                if (activeCrumb.isFile && activeCrumb.filePath) {
                    const chunks = await taskManager.listChunksInFile(
                        task.taskid,
                        activeCrumb.filePath
                    );
                    if (cancelled) {
                        return;
                    }
                    setChunkEntries(
                        (chunks ?? []).map((chunk) => ({
                            chunkId: chunk.chunkid,
                            sequence: chunk.seq,
                            size: chunk.size,
                            status: chunk.status,
                        }))
                    );
                    setFileEntries([]);
                } else {
                    const directoryPath = activeCrumb.requestPath ?? null;
                    const files = await taskManager.listFilesInTask(
                        task.taskid,
                        directoryPath
                    );
                    if (cancelled) {
                        return;
                    }
                    const basePath = activeCrumb.fullPath;
                    setFileEntries(
                        (files ?? []).map((file) => ({
                            name: file.name,
                            size: file.len,
                            createTime: file.create_time,
                            updateTime: file.update_time,
                            isDirectory: file.is_dir,
                            fullPath: joinPath(basePath, file.name),
                        }))
                    );
                    setChunkEntries([]);
                }
            } catch (error) {
                if (cancelled) {
                    return;
                }
                setContentError(
                    error instanceof Error ? error.message : String(error)
                );
                setFileEntries([]);
                setChunkEntries([]);
            } finally {
                if (!cancelled) {
                    setContentLoading(false);
                }
            }
        };

        loadContent().catch((err) => {
            console.error("Failed to load task detail content:", err);
        });

        return () => {
            cancelled = true;
        };
    }, [breadcrumbs, task.taskid]);

    const handleEntryClick = (entry: FileEntry) => {
        if (!currentBreadcrumb) {
            return;
        }

        const baseRequest =
            currentBreadcrumb.requestPath ?? currentBreadcrumb.fullPath;
        const nextRequestPath = joinPath(baseRequest, entry.name);
        const newBreadcrumb = {
            label: entry.name,
            requestPath: nextRequestPath,
            fullPath: entry.fullPath,
            isFile: !entry.isDirectory,
        } as BreadcrumbEntry;
        console.log("new breadcrumb:", newBreadcrumb);
        setBreadcrumbs((prev) => [...prev, newBreadcrumb]);
        onBreadcrumbChange?.(newBreadcrumb);
    };

    const handleBreadcrumbClick = (index: number) => {
        setBreadcrumbs((prev) => prev.slice(0, index + 1));
        onBreadcrumbChange?.(breadcrumbs[index] || null);
    };

    const isViewingChunks = Boolean(
        currentBreadcrumb &&
            currentBreadcrumb.isFile &&
            currentBreadcrumb.fullPath
    );
    console.log(
        "current breadcrumb:",
        currentBreadcrumb,
        "isViewingChunks:",
        isViewingChunks
    );

    return (
        <>
            <Breadcrumb>
                <BreadcrumbList>
                    {breadcrumbsForRender.map((crumb, index) => (
                        <React.Fragment key={`${crumb.fullPath}-${index}`}>
                            <BreadcrumbItem>
                                {index === breadcrumbsForRender.length - 1 ? (
                                    <BreadcrumbPage>
                                        {crumb.label}
                                    </BreadcrumbPage>
                                ) : (
                                    <BreadcrumbLink asChild>
                                        <button
                                            type="button"
                                            className="text-sm hover:underline"
                                            onClick={() =>
                                                handleBreadcrumbClick(index)
                                            }
                                        >
                                            {crumb.label}
                                        </button>
                                    </BreadcrumbLink>
                                )}
                            </BreadcrumbItem>
                            {index < breadcrumbsForRender.length - 1 && (
                                <BreadcrumbSeparator />
                            )}
                        </React.Fragment>
                    ))}
                </BreadcrumbList>
            </Breadcrumb>

            {contentError && (
                <div className="text-sm text-red-600">{contentError}</div>
            )}

            {contentLoading ? (
                <div className="text-sm text-muted-foreground">加载中...</div>
            ) : isViewingChunks ? (
                chunkEntries.length === 0 ? (
                    <div className="text-sm text-muted-foreground">
                        暂无数据块
                    </div>
                ) : (
                    <ScrollArea className="h-[400px]">
                        <div className="space-y-3">
                            {chunkEntries.map((chunk) => (
                                <div
                                    key={chunk.chunkId}
                                    className="flex items-center gap-3 p-3 border rounded-md"
                                >
                                    <Hash className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                                    <div className="flex-1 min-w-0">
                                        <p className="text-sm font-medium">
                                            {chunk.chunkId}
                                        </p>
                                        <div className="flex flex-wrap items-center gap-3 mt-1 text-xs text-muted-foreground">
                                            <span>序号: {chunk.sequence}</span>
                                            <span>
                                                大小:{" "}
                                                {TaskMgrHelper.formatSize(
                                                    chunk.size
                                                )}
                                            </span>
                                            <span>状态: {chunk.status}</span>
                                        </div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </ScrollArea>
                )
            ) : fileEntries.length === 0 ? (
                <div className="text-sm text-muted-foreground">暂无文件</div>
            ) : (
                <ScrollArea className="h-[400px]">
                    <div className="space-y-3">
                        {fileEntries.map((entry) => (
                            <button
                                key={entry.fullPath || entry.name}
                                type="button"
                                onClick={() => handleEntryClick(entry)}
                                className="w-full text-left"
                            >
                                <div className="flex items-center gap-3 p-3 border rounded-md hover:bg-muted">
                                    <div className="flex items-center gap-3 flex-1 min-w-0">
                                        {entry.isDirectory ? (
                                            <Folder className="w-4 h-4 text-blue-500 flex-shrink-0" />
                                        ) : (
                                            <File className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                                        )}
                                        <div className="flex-1 min-w-0">
                                            <p className="text-sm font-medium truncate">
                                                {entry.name}
                                            </p>
                                            <div className="flex flex-wrap items-center gap-3 mt-1 text-xs text-muted-foreground">
                                                {!entry.isDirectory && (
                                                    <span>
                                                        大小:{" "}
                                                        {TaskMgrHelper.formatSize(
                                                            entry.size
                                                        )}
                                                    </span>
                                                )}
                                                <span>
                                                    更新时间:{" "}
                                                    {TaskMgrHelper.formatTime(
                                                        entry.updateTime,
                                                        "--"
                                                    )}
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </button>
                        ))}
                    </div>
                </ScrollArea>
            )}
        </>
    );
}

function LogsTabContent({ task }: { task: TaskInfo }) {
    const [logs, setLogs] = useState<BackupLog[]>([]);
    const [logsLoading, setLogsLoading] = useState(false);
    const [logsError, setLogsError] = useState<string | null>(null);

    useEffect(() => {
        if (!task.taskid) {
            return;
        }

        let cancelled = false;

        const loadLogs = async () => {
            setLogsLoading(true);
            setLogsError(null);
            try {
                const result = await taskManager.listLogs(
                    0,
                    200,
                    ListOrder.DESC,
                    { task_id: task.taskid }
                );
                if (cancelled) {
                    return;
                }
                setLogs(result.logs ?? []);
            } catch (error) {
                if (cancelled) {
                    return;
                }
                setLogsError(
                    error instanceof Error ? error.message : String(error)
                );
                setLogs([]);
            } finally {
                if (!cancelled) {
                    setLogsLoading(false);
                }
            }
        };

        loadLogs().catch((err) => {
            console.error("Failed to load task logs:", err);
        });

        return () => {
            cancelled = true;
        };
    }, [task.taskid]);

    return (
        <>
            {logsError && (
                <div className="text-sm text-red-600">{logsError}</div>
            )}
            {logsLoading ? (
                <div className="text-sm text-muted-foreground">
                    日志加载中...
                </div>
            ) : logs.length === 0 ? (
                <div className="text-sm text-muted-foreground">暂无日志</div>
            ) : (
                <ScrollArea className="h-[400px]">
                    <div className="space-y-2 font-mono text-sm">
                        {logs.map((log) => (
                            <div key={log.seq} className="flex gap-3 text-xs">
                                <span className="text-muted-foreground flex-shrink-0">
                                    {TaskMgrHelper.formatTime(log.timestamp)}
                                </span>
                                <span className="flex-shrink-0 uppercase text-blue-600">
                                    {log.type}
                                </span>
                                <span className="flex-1 break-words">
                                    {TaskMgrHelper.formatLog(log)}
                                </span>
                            </div>
                        ))}
                    </div>
                </ScrollArea>
            )}
        </>
    );
}
