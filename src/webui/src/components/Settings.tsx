import React, { useEffect, useState } from "react";
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from "./ui/card";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "./ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { Separator } from "./ui/separator";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { LoadingPage } from "./LoadingPage";
import {
    Settings as SettingsIcon,
    Bell,
    Shield,
    HardDrive,
    Globe,
    User,
    Download,
    Upload,
} from "lucide-react";

export function Settings() {
    const { t, language, setLanguage } = useLanguage();
    const isMobile = useMobile();
    const [loading, setLoading] = useState(true);
    useEffect(() => {
        const id = window.setTimeout(() => setLoading(false), 500);
        return () => window.clearTimeout(id);
    }, []);

    if (loading) {
        return (
            <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-6`}>
                <div>
                    <h1 className="mb-2">{t.settings.title}</h1>
                    <p className="text-muted-foreground">{t.settings.subtitle}</p>
                </div>
                <LoadingPage status={`${t.common.loading} ${t.nav.settings}...`} />
            </div>
        );
    }

    return (
        <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-6`}>
            <div>
                <h1 className="mb-2">{t.settings.title}</h1>
                <p className="text-muted-foreground">{t.settings.subtitle}</p>
            </div>

            <Tabs defaultValue="general" className="space-y-6">
                <TabsList
                    className={`grid w-full ${
                        isMobile ? "grid-cols-3" : "grid-cols-5"
                    }`}
                >
                    <TabsTrigger value="general">
                        {t.settings.general}
                    </TabsTrigger>
                    <TabsTrigger value="notifications">
                        {t.settings.notifications}
                    </TabsTrigger>
                    {!isMobile && (
                        <TabsTrigger value="security">
                            {t.settings.security}
                        </TabsTrigger>
                    )}
                    {!isMobile && (
                        <TabsTrigger value="performance">
                            {t.settings.performance}
                        </TabsTrigger>
                    )}
                    <TabsTrigger value="advanced">
                        {t.settings.advanced}
                    </TabsTrigger>
                </TabsList>

                {/* 常规设置 */}
                <TabsContent value="general" className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <User className="w-5 h-5" />
                                用户首选项
                            </CardTitle>
                            <CardDescription>
                                配置基本的用户界面和行为设置
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <div className="space-y-2">
                                    <Label>{t.settings.language}</Label>
                                    <Select
                                        value={language}
                                        onValueChange={setLanguage}
                                    >
                                        <SelectTrigger>
                                            <SelectValue />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="zh-cn">
                                                简体中文
                                            </SelectItem>
                                            <SelectItem value="en">
                                                English
                                            </SelectItem>
                                        </SelectContent>
                                    </Select>
                                </div>
                                <div className="space-y-2">
                                    <Label>{t.settings.timezone}</Label>
                                    <Select defaultValue="asia-shanghai">
                                        <SelectTrigger>
                                            <SelectValue />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="asia-shanghai">
                                                Asia/Shanghai (UTC+8)
                                            </SelectItem>
                                            <SelectItem value="utc">
                                                UTC (UTC+0)
                                            </SelectItem>
                                            <SelectItem value="america-new-york">
                                                America/New_York (UTC-5)
                                            </SelectItem>
                                        </SelectContent>
                                    </Select>
                                </div>
                            </div>

                            <Separator />

                            <div className="space-y-4">
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>{t.settings.autoStart}</Label>
                                        <p className="text-sm text-muted-foreground">
                                            系统启动时自动启动备份服务
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>
                                            {t.settings.minimizeToTray}
                                        </Label>
                                        <p className="text-sm text-muted-foreground">
                                            关闭窗口时最小化到系统托盘而非退出
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>{t.settings.autoUpdate}</Label>
                                        <p className="text-sm text-muted-foreground">
                                            定期检查软件更新
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                            </div>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <HardDrive className="w-5 h-5" />
                                默认路径
                            </CardTitle>
                            <CardDescription>
                                设置备份和恢复的默认路径
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="space-y-2">
                                <Label>默认备份路径</Label>
                                <div className="flex gap-2">
                                    <Input
                                        defaultValue="D:\Backups"
                                        className="flex-1"
                                    />
                                    <Button variant="outline">浏览</Button>
                                </div>
                            </div>
                            <div className="space-y-2">
                                <Label>临时文件路径</Label>
                                <div className="flex gap-2">
                                    <Input
                                        defaultValue="C:\Users\{username}\AppData\Local\Temp\BuckyBackup"
                                        className="flex-1"
                                    />
                                    <Button variant="outline">浏览</Button>
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* 通知设置 */}
                <TabsContent value="notifications" className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Bell className="w-5 h-5" />
                                通知设置
                            </CardTitle>
                            <CardDescription>
                                控制何时和如何接收通知
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="space-y-4">
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>桌面通知</Label>
                                        <p className="text-sm text-muted-foreground">
                                            显示系统桌面通知
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>任务完成通知</Label>
                                        <p className="text-sm text-muted-foreground">
                                            备份或恢复任务完成时通知
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>错误警告通知</Label>
                                        <p className="text-sm text-muted-foreground">
                                            发生错误时立即通知
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>存储空间警告</Label>
                                        <p className="text-sm text-muted-foreground">
                                            备份目标空间不足时警告
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                            </div>

                            <Separator />

                            <div className="space-y-4">
                                <h4>邮件通知</h4>
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div className="space-y-2">
                                        <Label>SMTP服务器</Label>
                                        <Input placeholder="smtp.example.com" />
                                    </div>
                                    <div className="space-y-2">
                                        <Label>端口</Label>
                                        <Input placeholder="587" />
                                    </div>
                                    <div className="space-y-2">
                                        <Label>发件人邮箱</Label>
                                        <Input placeholder="backup@example.com" />
                                    </div>
                                    <div className="space-y-2">
                                        <Label>收件人邮箱</Label>
                                        <Input placeholder="admin@example.com" />
                                    </div>
                                </div>
                                <div className="flex items-center justify-between">
                                    <Label>启用邮件通知</Label>
                                    <Switch />
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* 安全设置 */}
                <TabsContent value="security" className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Shield className="w-5 h-5" />
                                安全和加密
                            </CardTitle>
                            <CardDescription>
                                配置数据加密和安全选项
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="space-y-4">
                                <div className="space-y-2">
                                    <Label>默认加密算法</Label>
                                    <Select defaultValue="aes256">
                                        <SelectTrigger>
                                            <SelectValue />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="aes256">
                                                AES-256-CBC
                                            </SelectItem>
                                            <SelectItem value="aes192">
                                                AES-192-CBC
                                            </SelectItem>
                                            <SelectItem value="aes128">
                                                AES-128-CBC
                                            </SelectItem>
                                        </SelectContent>
                                    </Select>
                                </div>

                                <div className="space-y-2">
                                    <Label>主加密密钥</Label>
                                    <div className="flex gap-2">
                                        <Input
                                            type="password"
                                            placeholder="输入主密钥"
                                            className="flex-1"
                                        />
                                        <Button variant="outline">生成</Button>
                                    </div>
                                    <p className="text-sm text-muted-foreground">
                                        用于加密所有备份数据的主密钥，请妥善保管
                                    </p>
                                </div>

                                <Separator />

                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>自动锁定</Label>
                                        <p className="text-sm text-muted-foreground">
                                            无操作一段时间后自动锁定应用
                                        </p>
                                    </div>
                                    <Switch />
                                </div>

                                <div className="space-y-2">
                                    <Label>锁定超时 (分钟)</Label>
                                    <Input type="number" defaultValue="30" />
                                </div>

                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>文件完整性检查</Label>
                                        <p className="text-sm text-muted-foreground">
                                            备份后验证文件完整性
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* 性能设置 */}
                <TabsContent value="performance" className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <Download className="w-5 h-5" />
                                传输和性能
                            </CardTitle>
                            <CardDescription>
                                优化备份性能和资源使用
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <div className="space-y-2">
                                    <Label>并发任务数量</Label>
                                    <Select defaultValue="2">
                                        <SelectTrigger>
                                            <SelectValue />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="1">1</SelectItem>
                                            <SelectItem value="2">2</SelectItem>
                                            <SelectItem value="4">4</SelectItem>
                                            <SelectItem value="8">8</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </div>
                                <div className="space-y-2">
                                    <Label>网络传输线程</Label>
                                    <Select defaultValue="4">
                                        <SelectTrigger>
                                            <SelectValue />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="1">1</SelectItem>
                                            <SelectItem value="2">2</SelectItem>
                                            <SelectItem value="4">4</SelectItem>
                                            <SelectItem value="8">8</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </div>
                            </div>

                            <div className="space-y-2">
                                <Label>传输速率限制 (MB/s)</Label>
                                <Input type="number" placeholder="0 = 无限制" />
                                <p className="text-sm text-muted-foreground">
                                    0 表示无限制
                                </p>
                            </div>

                            <Separator />

                            <div className="space-y-4">
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>智能调度</Label>
                                        <p className="text-sm text-muted-foreground">
                                            根据系统负载自动调整备份速度
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>

                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>低优先级模式</Label>
                                        <p className="text-sm text-muted-foreground">
                                            降低备份任务的CPU优先级
                                        </p>
                                    </div>
                                    <Switch />
                                </div>

                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>压缩优化</Label>
                                        <p className="text-sm text-muted-foreground">
                                            自动选择最优压缩算法
                                        </p>
                                    </div>
                                    <Switch defaultChecked />
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* 高级设置 */}
                <TabsContent value="advanced" className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle className="flex items-center gap-2">
                                <SettingsIcon className="w-5 h-5" />
                                高级选项
                            </CardTitle>
                            <CardDescription>高级用户配置选项</CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="space-y-2">
                                <Label>日志级别</Label>
                                <Select defaultValue="info">
                                    <SelectTrigger>
                                        <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="error">
                                            错误
                                        </SelectItem>
                                        <SelectItem value="warn">
                                            警告
                                        </SelectItem>
                                        <SelectItem value="info">
                                            信息
                                        </SelectItem>
                                        <SelectItem value="debug">
                                            调试
                                        </SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>

                            <div className="space-y-2">
                                <Label>日志保留天数</Label>
                                <Input type="number" defaultValue="30" />
                            </div>

                            <Separator />

                            <div className="space-y-4">
                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>开发者模式</Label>
                                        <p className="text-sm text-muted-foreground">
                                            启用高级调试功能
                                        </p>
                                    </div>
                                    <Switch />
                                </div>

                                <div className="flex items-center justify-between">
                                    <div className="space-y-1">
                                        <Label>实验性功能</Label>
                                        <p className="text-sm text-muted-foreground">
                                            启用实验性功能（可能不稳定）
                                        </p>
                                    </div>
                                    <Switch />
                                </div>
                            </div>

                            <Separator />

                            <div className="space-y-4">
                                <h4>维护操作</h4>
                                <div className="flex gap-2">
                                    <Button variant="outline">
                                        清理临时文件
                                    </Button>
                                    <Button variant="outline">
                                        重置所有设置
                                    </Button>
                                    <Button variant="outline">导出配置</Button>
                                    <Button variant="outline">导入配置</Button>
                                </div>
                            </div>
                        </CardContent>
                    </Card>
                </TabsContent>
            </Tabs>

            {/* 保存设置 */}
            <Card>
                <CardContent className="py-4">
                    <div className="flex items-center justify-between">
                        <p className="text-sm text-muted-foreground">
                            设置更改将自动保存
                        </p>
                        <div className="flex gap-2">
                            <Button variant="outline">重置为默认</Button>
                            <Button>保存所有设置</Button>
                        </div>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
