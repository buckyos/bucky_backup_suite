import React, { useState } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from './ui/card';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs';
import { ScrollArea } from './ui/scroll-area';
import { Progress } from './ui/progress';
import { useLanguage } from './i18n/LanguageProvider';
import { useMobile } from './hooks/use-mobile';
import { 
  ArrowLeft, 
  Calendar, 
  Clock, 
  Folder, 
  Server, 
  Play,
  Pause,
  CheckCircle,
  XCircle,
  AlertTriangle,
  FileText,
  Activity,
  Edit,
  Undo2
} from 'lucide-react';

interface PlanDetailsProps {
  onBack: () => void;
  onNavigate: (page: string, data?: any) => void;
  data?: any;
}

export function PlanDetails({ onBack, onNavigate, data }: PlanDetailsProps) {
  const { t } = useLanguage();
  const isMobile = useMobile();

  // 模拟数据
  const planData = {
    id: 1,
    name: "系统文件备份",
    description: "每日自动备份系统关键文件",
    enabled: true,
    source: "C:\\Windows\\System32",
    destination: "本地D盘",
    nextRun: "今天 23:00",
    schedule: "每天 23:00",
    lastRun: "昨天 23:00",
    status: "healthy",
    created: "2024-01-01 10:00:00",
    modified: "2024-01-10 15:30:00",
    totalBackups: 45,
    totalSize: "12.5 GB"
  };

  const taskHistory = [
    {
      id: 1,
      name: "系统文件夜间备份",
      status: "completed",
      startTime: "2024-01-15 23:00:00",
      endTime: "2024-01-15 23:28:00",
      duration: "28分钟",
      totalSize: "2.1 GB",
      processedFiles: 1542,
      transferRate: "1.2 MB/s"
    },
    {
      id: 2,
      name: "系统文件夜间备份",
      status: "completed",
      startTime: "2024-01-14 23:00:00",
      endTime: "2024-01-14 23:25:00",
      duration: "25分钟",
      totalSize: "2.0 GB",
      processedFiles: 1538,
      transferRate: "1.3 MB/s"
    },
    {
      id: 3,
      name: "系统文件夜间备份",
      status: "failed",
      startTime: "2024-01-13 23:00:00",
      endTime: "2024-01-13 23:15:00",
      duration: "15分钟",
      totalSize: "0.8 GB",
      processedFiles: 612,
      transferRate: "0.9 MB/s",
      error: "目标磁盘空间不足"
    }
  ];

  const operationLogs = [
    {
      id: 1,
      timestamp: "2024-01-15 23:00:00",
      type: "task_start",
      message: "备份任务开始执行",
      details: "自动触发的定时任务"
    },
    {
      id: 2,
      timestamp: "2024-01-15 23:28:00",
      type: "task_complete",
      message: "备份任务成功完成",
      details: "处理了 1542 个文件，总大小 2.1 GB"
    },
    {
      id: 3,
      timestamp: "2024-01-10 15:30:00",
      type: "plan_update",
      message: "计划配置已更新",
      details: "修改了执行时间为 23:00"
    },
    {
      id: 4,
      timestamp: "2024-01-10 15:25:00",
      type: "plan_update",
      message: "计划描述已更新",
      details: "更新了计划描述信息"
    }
  ];

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'healthy':
        return <Badge className="bg-green-100 text-green-800">正常</Badge>;
      case 'warning':
        return <Badge className="bg-yellow-100 text-yellow-800">警告</Badge>;
      case 'disabled':
        return <Badge variant="secondary">已禁用</Badge>;
      default:
        return <Badge variant="outline">未知</Badge>;
    }
  };

  const getTaskStatusBadge = (status: string) => {
    switch (status) {
      case 'completed':
        return <Badge className="bg-green-100 text-green-800 text-xs">已完成</Badge>;
      case 'failed':
        return <Badge className="bg-red-100 text-red-800 text-xs">失败</Badge>;
      case 'running':
        return <Badge className="bg-blue-100 text-blue-800 text-xs">执行中</Badge>;
      default:
        return <Badge variant="outline" className="text-xs">未知</Badge>;
    }
  };

  const getTaskStatusIcon = (status: string) => {
    switch (status) {
      case 'completed':
        return <CheckCircle className="w-4 h-4 text-green-500" />;
      case 'failed':
        return <XCircle className="w-4 h-4 text-red-500" />;
      case 'running':
        return <Activity className="w-4 h-4 text-blue-500" />;
      default:
        return <AlertTriangle className="w-4 h-4 text-yellow-500" />;
    }
  };

  const getLogTypeIcon = (type: string) => {
    switch (type) {
      case 'task_start':
        return <Play className="w-4 h-4 text-blue-500" />;
      case 'task_complete':
        return <CheckCircle className="w-4 h-4 text-green-500" />;
      case 'task_fail':
        return <XCircle className="w-4 h-4 text-red-500" />;
      case 'plan_update':
        return <Edit className="w-4 h-4 text-orange-500" />;
      default:
        return <FileText className="w-4 h-4 text-muted-foreground" />;
    }
  };

  return (
    <div className={`${isMobile ? 'p-4' : 'p-6'} space-y-6`}>
      {/* 头部导航 */}
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ArrowLeft className="w-4 h-4" />
          {!isMobile && <span className="ml-2">返回</span>}
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-xl font-semibold">{planData.name}</h1>
            {getStatusBadge(planData.status)}
          </div>
          <p className="text-sm text-muted-foreground">{planData.description}</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => onNavigate('edit-plan', planData)}
            className="gap-2"
          >
            <Edit className="w-4 h-4" />
            {!isMobile && '编辑'}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onNavigate('restore', { planId: planData.id })}
            className="gap-2"
          >
            <Undo2 className="w-4 h-4" />
            {!isMobile && '恢复'}
          </Button>
        </div>
      </div>

      {/* 内容标签页 */}
      <Tabs defaultValue="overview" className="space-y-6">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="overview">概览</TabsTrigger>
          <TabsTrigger value="tasks">任务历史</TabsTrigger>
          <TabsTrigger value="logs">操作日志</TabsTrigger>
        </TabsList>

        {/* 概览 */}
        <TabsContent value="overview" className="space-y-6">
          <div className="grid gap-6 md:grid-cols-2">
            {/* 基本信息 */}
            <Card>
              <CardHeader>
                <CardTitle className="text-base">基本信息</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-3">
                  <div className="flex justify-between">
                    <span className="text-sm text-muted-foreground">计划名称:</span>
                    <span className="text-sm font-medium">{planData.name}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-sm text-muted-foreground">状态:</span>
                    <span className="text-sm">{getStatusBadge(planData.status)}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-sm text-muted-foreground">启用状态:</span>
                    <span className="text-sm font-medium">{planData.enabled ? '已启用' : '已禁用'}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-sm text-muted-foreground">创建时间:</span>
                    <span className="text-sm font-medium">{planData.created}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-sm text-muted-foreground">最后修改:</span>
                    <span className="text-sm font-medium">{planData.modified}</span>
                  </div>
                </div>
              </CardContent>
            </Card>

            {/* 配置信息 */}
            <Card>
              <CardHeader>
                <CardTitle className="text-base">配置信息</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-3">
                  <div className="flex items-start gap-2">
                    <Folder className="w-4 h-4 mt-0.5 text-muted-foreground" />
                    <div className="flex-1">
                      <p className="text-sm text-muted-foreground">备份源</p>
                      <p className="text-sm font-medium">{planData.source}</p>
                    </div>
                  </div>
                  <div className="flex items-start gap-2">
                    <Server className="w-4 h-4 mt-0.5 text-muted-foreground" />
                    <div className="flex-1">
                      <p className="text-sm text-muted-foreground">备份目标</p>
                      <p className="text-sm font-medium">{planData.destination}</p>
                    </div>
                  </div>
                  <div className="flex items-start gap-2">
                    <Calendar className="w-4 h-4 mt-0.5 text-muted-foreground" />
                    <div className="flex-1">
                      <p className="text-sm text-muted-foreground">执行计划</p>
                      <p className="text-sm font-medium">{planData.schedule}</p>
                    </div>
                  </div>
                  <div className="flex items-start gap-2">
                    <Clock className="w-4 h-4 mt-0.5 text-muted-foreground" />
                    <div className="flex-1">
                      <p className="text-sm text-muted-foreground">下次执行</p>
                      <p className="text-sm font-medium">{planData.nextRun}</p>
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          {/* 统计信息 */}
          <div className="grid gap-4 md:grid-cols-4">
            <Card>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-blue-100 rounded-full">
                    <Activity className="w-4 h-4 text-blue-600" />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">总备份次数</p>
                    <p className="text-lg font-semibold">{planData.totalBackups}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-green-100 rounded-full">
                    <CheckCircle className="w-4 h-4 text-green-600" />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">成功次数</p>
                    <p className="text-lg font-semibold">{planData.totalBackups - 1}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-red-100 rounded-full">
                    <XCircle className="w-4 h-4 text-red-600" />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">失败次数</p>
                    <p className="text-lg font-semibold">1</p>
                  </div>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-purple-100 rounded-full">
                    <Server className="w-4 h-4 text-purple-600" />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">总数据量</p>
                    <p className="text-lg font-semibold">{planData.totalSize}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        {/* 任务历史 */}
        <TabsContent value="tasks" className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-lg font-medium">任务执行历史</h3>
              <p className="text-sm text-muted-foreground">按时间降序排列的备份任务</p>
            </div>
          </div>

          <div className="space-y-3">
            {taskHistory.map((task) => (
              <Card key={task.id}>
                <CardContent className="p-4">
                  <div className="flex items-start gap-4">
                    {getTaskStatusIcon(task.status)}
                    <div className="flex-1 space-y-2">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <h4 className="font-medium">{task.name}</h4>
                          {getTaskStatusBadge(task.status)}
                        </div>
                        <div className="text-right">
                          <p className="text-sm font-medium">{task.totalSize}</p>
                          <p className="text-xs text-muted-foreground">{task.duration}</p>
                        </div>
                      </div>
                      
                      <div className={`grid ${isMobile ? 'grid-cols-1' : 'grid-cols-3'} gap-4 text-sm`}>
                        <div>
                          <p className="text-muted-foreground">开始时间</p>
                          <p className="font-medium">{task.startTime}</p>
                        </div>
                        <div>
                          <p className="text-muted-foreground">处理文件</p>
                          <p className="font-medium">{task.processedFiles} 个</p>
                        </div>
                        <div>
                          <p className="text-muted-foreground">传输速度</p>
                          <p className="font-medium">{task.transferRate}</p>
                        </div>
                      </div>

                      {task.error && (
                        <div className="p-3 bg-red-50 text-red-700 rounded-md text-sm">
                          错误: {task.error}
                        </div>
                      )}

                      {task.status === 'completed' && (
                        <div className="flex justify-end">
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => onNavigate('restore', { taskId: task.id })}
                            className="gap-2"
                          >
                            <Undo2 className="w-3 h-3" />
                            恢复
                          </Button>
                        </div>
                      )}
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </TabsContent>

        {/* 操作日志 */}
        <TabsContent value="logs" className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-lg font-medium">操作日志</h3>
              <p className="text-sm text-muted-foreground">计划相关的所有操作记录</p>
            </div>
          </div>

          <Card>
            <CardContent className="p-0">
              <div className="space-y-0">
                {operationLogs.map((log, index) => (
                  <div 
                    key={log.id} 
                    className={`p-4 ${index < operationLogs.length - 1 ? 'border-b' : ''}`}
                  >
                    <div className="flex items-start gap-3">
                      {getLogTypeIcon(log.type)}
                      <div className="flex-1">
                        <div className="flex items-center justify-between mb-1">
                          <p className="font-medium text-sm">{log.message}</p>
                          <p className="text-xs text-muted-foreground">{log.timestamp}</p>
                        </div>
                        <p className="text-xs text-muted-foreground">{log.details}</p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}