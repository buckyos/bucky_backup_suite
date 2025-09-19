import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from './ui/card';
import { Button } from './ui/button';
import { Progress } from './ui/progress';
import { Badge } from './ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs';
import { ScrollArea } from './ui/scroll-area';
import { useLanguage } from './i18n/LanguageProvider';
import { useMobile } from './hooks/use-mobile';
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
  Hash
} from 'lucide-react';

interface TaskDetailProps {
  task: any;
  onBack: () => void;
}

export function TaskDetail({ task, onBack }: TaskDetailProps) {
  const { t } = useLanguage();
  const isMobile = useMobile();

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'completed':
        return <CheckCircle className="w-4 h-4 text-green-500" />;
      case 'failed':
        return <XCircle className="w-4 h-4 text-red-500" />;
      case 'running':
        return <Download className="w-4 h-4 text-blue-500" />;
      default:
        return <AlertTriangle className="w-4 h-4 text-yellow-500" />;
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'running':
        return <Badge className="bg-blue-100 text-blue-800 text-xs">{t.tasks.running}</Badge>;
      case 'completed':
        return <Badge className="bg-green-100 text-green-800 text-xs">{t.tasks.completed}</Badge>;
      case 'paused':
        return <Badge className="bg-yellow-100 text-yellow-800 text-xs">{t.tasks.paused}</Badge>;
      case 'failed':
        return <Badge className="bg-red-100 text-red-800 text-xs">{t.tasks.failed}</Badge>;
      case 'queued':
        return <Badge className="bg-gray-100 text-gray-800 text-xs">{t.tasks.queued}</Badge>;
      default:
        return <Badge variant="outline" className="text-xs">未知</Badge>;
    }
  };

  // 模拟文件列表数据
  const files = [
    {
      path: 'C:\\Users\\Documents\\report.pdf',
      status: 'completed',
      size: '2.5 MB',
      hash: 'sha256:a1b2c3d4...',
      speed: '',
      progress: 100
    },
    {
      path: 'C:\\Users\\Documents\\presentation.pptx',
      status: 'running',
      size: '15.2 MB',
      hash: 'sha256:e5f6g7h8...',
      speed: '3.2 MB/s',
      progress: 65
    },
    {
      path: 'C:\\Users\\Documents\\data.xlsx',
      status: 'pending',
      size: '8.7 MB',
      hash: '',
      speed: '',
      progress: 0
    },
    {
      path: 'C:\\Users\\Documents\\image.jpg',
      status: 'failed',
      size: '4.1 MB',
      hash: '',
      speed: '',
      progress: 23
    }
  ];

  // 模拟Chunk列表数据
  const chunks = [
    {
      id: 'chunk_001',
      hash: 'sha256:1a2b3c4d5e6f...',
      size: '64 KB',
      status: 'completed',
      speed: '',
      file: 'report.pdf'
    },
    {
      id: 'chunk_002',
      hash: 'sha256:2b3c4d5e6f7g...',
      size: '64 KB',
      status: 'running',
      speed: '2.1 MB/s',
      file: 'presentation.pptx'
    },
    {
      id: 'chunk_003',
      hash: 'sha256:3c4d5e6f7g8h...',
      size: '64 KB',
      status: 'pending',
      speed: '',
      file: 'presentation.pptx'
    }
  ];

  // 模拟日志数据
  const logs = [
    { time: '2024-01-15 23:00:00', level: 'info', message: '开始备份任务: 系统文件夜间备份' },
    { time: '2024-01-15 23:00:05', level: 'info', message: '扫描源目录: C:\\Windows\\System32' },
    { time: '2024-01-15 23:00:10', level: 'info', message: '找到 1,247 个文件需要备份' },
    { time: '2024-01-15 23:02:15', level: 'info', message: '开始传输文件: system.dll' },
    { time: '2024-01-15 23:02:45', level: 'warning', message: '文件跳过: access_denied.sys (权限不足)' },
    { time: '2024-01-15 23:05:30', level: 'info', message: '已完成 456/1247 个文件' },
    { time: '2024-01-15 23:08:12', level: 'info', message: '当前速度: 12.5 MB/s' },
  ];

  return (
    <div className={`${isMobile ? 'p-4 pt-16' : 'p-6'} space-y-4`}>
      {/* 头部 */}
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="sm" onClick={onBack} className="gap-2">
          <ChevronLeft className="w-4 h-4" />
          {t.common.back}
        </Button>
        <div className="flex-1">
          <h1 className="mb-2">{task.name}</h1>
          <p className="text-muted-foreground text-sm">{task.plan}</p>
        </div>
      </div>

      {/* 任务概览 */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <CardTitle className="text-lg">任务概览</CardTitle>
              {getStatusBadge(task.status)}
            </div>
            <div className="flex gap-2">
              {task.status === 'running' && (
                <Button variant="outline" size="sm" className="gap-1">
                  <Pause className="w-3 h-3" />
                  {t.tasks.pause}
                </Button>
              )}
              {task.status === 'paused' && (
                <Button variant="outline" size="sm" className="gap-1">
                  <Play className="w-3 h-3" />
                  {t.tasks.resume}
                </Button>
              )}
              {(task.status === 'running' || task.status === 'paused') && (
                <Button variant="outline" size="sm" className="gap-1 text-destructive">
                  <Square className="w-3 h-3" />
                  {t.tasks.stop}
                </Button>
              )}
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {task.status === 'running' && (
            <div className="space-y-3 mb-4">
              <div className="flex justify-between text-sm">
                <span>进度: {task.progress}%</span>
                <span>{task.speed}</span>
              </div>
              <Progress value={task.progress} className="h-3" />
              <div className="flex justify-between text-sm text-muted-foreground">
                <span>{task.processedSize} / {task.totalSize}</span>
                <span>{task.remaining}</span>
              </div>
            </div>
          )}
          
          <div className={`grid ${isMobile ? 'grid-cols-1' : 'grid-cols-2'} gap-4 text-sm`}>
            <div>
              <p className="text-muted-foreground mb-1">开始时间</p>
              <p className="flex items-center gap-2">
                <Clock className="w-4 h-4" />
                {task.startTime ? new Date(task.startTime).toLocaleString() : '-'}
              </p>
            </div>
            <div>
              <p className="text-muted-foreground mb-1">数据大小</p>
              <p>{task.processedSize} / {task.totalSize}</p>
            </div>
            <div>
              <p className="text-muted-foreground mb-1">任务类型</p>
              <p>{task.type === 'backup' ? t.tasks.backup : t.tasks.restore}</p>
            </div>
            <div>
              <p className="text-muted-foreground mb-1">所属计划</p>
              <p>{task.plan}</p>
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

      {/* 详细信息标签页 */}
      <Tabs defaultValue="files" className="space-y-4">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="files">文件列表</TabsTrigger>
          <TabsTrigger value="chunks">数据块</TabsTrigger>
          <TabsTrigger value="logs">日志</TabsTrigger>
        </TabsList>

        <TabsContent value="files" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">文件列表</CardTitle>
              <CardDescription>任务中所有文件的详细状态</CardDescription>
            </CardHeader>
            <CardContent>
              <ScrollArea className="h-[400px]">
                <div className="space-y-3">
                  {files.map((file, index) => (
                    <div key={index} className="flex items-center gap-3 p-3 border rounded-md">
                      <File className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium truncate">{file.path}</p>
                        <div className="flex items-center gap-4 mt-1 text-xs text-muted-foreground">
                          <span>{file.size}</span>
                          {file.hash && (
                            <>
                              <span>•</span>
                              <span className="truncate">{file.hash}</span>
                            </>
                          )}
                          {file.speed && (
                            <>
                              <span>•</span>
                              <span>{file.speed}</span>
                            </>
                          )}
                        </div>
                        {file.status === 'running' && (
                          <Progress value={file.progress} className="h-1 mt-2" />
                        )}
                      </div>
                      <div className="flex-shrink-0">
                        {getStatusIcon(file.status)}
                      </div>
                    </div>
                  ))}
                </div>
              </ScrollArea>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="chunks" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">数据块列表</CardTitle>
              <CardDescription>文件分块的传输状态</CardDescription>
            </CardHeader>
            <CardContent>
              <ScrollArea className="h-[400px]">
                <div className="space-y-3">
                  {chunks.map((chunk) => (
                    <div key={chunk.id} className="flex items-center gap-3 p-3 border rounded-md">
                      <Hash className="w-4 h-4 text-muted-foreground flex-shrink-0" />
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium">{chunk.id}</p>
                        <div className="flex items-center gap-4 mt-1 text-xs text-muted-foreground">
                          <span>{chunk.size}</span>
                          <span>•</span>
                          <span className="truncate">{chunk.hash}</span>
                          <span>•</span>
                          <span>{chunk.file}</span>
                          {chunk.speed && (
                            <>
                              <span>•</span>
                              <span>{chunk.speed}</span>
                            </>
                          )}
                        </div>
                      </div>
                      <div className="flex-shrink-0">
                        {getStatusIcon(chunk.status)}
                      </div>
                    </div>
                  ))}
                </div>
              </ScrollArea>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="logs" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">执行日志</CardTitle>
              <CardDescription>任务执行的详细日志记录</CardDescription>
            </CardHeader>
            <CardContent>
              <ScrollArea className="h-[400px]">
                <div className="space-y-2 font-mono text-sm">
                  {logs.map((log, index) => (
                    <div key={index} className="flex gap-3 text-xs">
                      <span className="text-muted-foreground flex-shrink-0">{log.time}</span>
                      <span className={`flex-shrink-0 uppercase ${
                        log.level === 'error' ? 'text-red-600' :
                        log.level === 'warning' ? 'text-yellow-600' :
                        'text-blue-600'
                      }`}>
                        {log.level}
                      </span>
                      <span className="flex-1">{log.message}</span>
                    </div>
                  ))}
                </div>
              </ScrollArea>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}