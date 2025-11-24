import React, { useEffect, useState } from "react";
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from "./ui/card";
import { Label } from "./ui/label";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "./ui/select";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { LoadingPage } from "./LoadingPage";
import { Settings as SettingsIcon, Globe } from "lucide-react";

export function Settings() {
    const { t, language, setLanguage } = useLanguage();
    const isMobile = useMobile();
    const [loading, setLoading] = useState(true);
    const [concurrentTasks, setConcurrentTasks] = useState("2");

    useEffect(() => {
        const id = window.setTimeout(() => setLoading(false), 500);
        return () => window.clearTimeout(id);
    }, []);

    if (loading) {
        return (
            <div className={`${isMobile ? "p-4 pt-16" : "p-6"} space-y-6`}>
                <div>
                    <h1 className="mb-2">{t.settings.title}</h1>
                    <p className="text-muted-foreground">
                        {t.settings.subtitle}
                    </p>
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

            <Card>
                <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                        <SettingsIcon className="w-5 h-5" />
                        {t.settings.general}
                    </CardTitle>
                    <CardDescription>{t.settings.subtitle}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-6">
                    <div className="space-y-2">
                        <Label className="flex items-center gap-2">
                            <Globe className="w-4 h-4" />
                            {t.settings.language}
                        </Label>
                        <Select value={language} onValueChange={setLanguage}>
                            <SelectTrigger>
                                <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                                <SelectItem value="zh-cn">简体中文</SelectItem>
                                <SelectItem value="en">English</SelectItem>
                            </SelectContent>
                        </Select>
                    </div>

                    <div className="space-y-2">
                        <Label className="flex items-center gap-2">
                            <SettingsIcon className="w-4 h-4" />
                            {t.settings.taskConcurrency}
                        </Label>
                        <Select
                            value={concurrentTasks}
                            onValueChange={setConcurrentTasks}
                        >
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
                </CardContent>
            </Card>
        </div>
    );
}
