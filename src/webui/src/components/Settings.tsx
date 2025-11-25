import React, { useCallback, useEffect, useState } from "react";
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
import { Language } from "./i18n";
import { useLanguage } from "./i18n/LanguageProvider";
import { useMobile } from "./hooks/use_mobile";
import { LoadingPage } from "./LoadingPage";
import { Settings as SettingsIcon, Globe } from "lucide-react";
import { taskManager } from "./utils/task_mgr_helper";

const DEFAULT_TASK_CONCURRENCY = 5;
const SUPPORTED_LANGUAGES: Language[] = ["zh-cn", "en"];

export function Settings() {
    const { t, language, setLanguage } = useLanguage();
    const isMobile = useMobile();
    const [loading, setLoading] = useState(true);
    const [concurrentTasks, setConcurrentTasks] = useState(
        DEFAULT_TASK_CONCURRENCY.toString()
    );

    useEffect(() => {
        let isMounted = true;
        const loadSettings = async () => {
            try {
                const settings = await taskManager.getUserSettings();
                if (!isMounted) {
                    return;
                }
                const requestedLanguage = settings.language as Language;
                const nextLanguage = SUPPORTED_LANGUAGES.includes(
                    requestedLanguage
                )
                    ? requestedLanguage
                    : "zh-cn";
                const nextConcurrency = (
                    settings.task_concurrency ?? DEFAULT_TASK_CONCURRENCY
                ).toString();
                setLanguage(nextLanguage);
                setConcurrentTasks(nextConcurrency);
            } catch (error) {
                console.error("Failed to load user settings", error);
            } finally {
                if (isMounted) {
                    setLoading(false);
                }
            }
        };
        loadSettings();
        return () => {
            isMounted = false;
        };
    }, [setLanguage]);

    const normalizeConcurrency = useCallback((value: string): number => {
        const parsed = parseInt(value, 10);
        if (Number.isFinite(parsed) && parsed > 0) {
            return parsed;
        }
        return DEFAULT_TASK_CONCURRENCY;
    }, []);

    const saveSettings = useCallback(
        async (nextLanguage: Language, nextConcurrency: number) => {
            try {
                await taskManager.saveUserSettings({
                    language: nextLanguage,
                    task_concurrency: nextConcurrency,
                });
            } catch (error) {
                console.error("Failed to save user settings", error);
            }
        },
        []
    );

    const handleLanguageChange = useCallback(
        (value: Language) => {
            setLanguage(value);
            saveSettings(value, normalizeConcurrency(concurrentTasks));
        },
        [concurrentTasks, normalizeConcurrency, saveSettings, setLanguage]
    );

    const handleConcurrencyChange = useCallback(
        (value: string) => {
            setConcurrentTasks(value);
            saveSettings(language, normalizeConcurrency(value));
        },
        [language, normalizeConcurrency, saveSettings]
    );

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
                        <Select
                            value={language}
                            onValueChange={(value) =>
                                handleLanguageChange(value as Language)
                            }
                        >
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
                            onValueChange={handleConcurrencyChange}
                        >
                            <SelectTrigger>
                                <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                                <SelectItem value="1">1</SelectItem>
                                <SelectItem value="2">2</SelectItem>
                                <SelectItem value="4">4</SelectItem>
                                <SelectItem value="5">5</SelectItem>
                                <SelectItem value="8">8</SelectItem>
                            </SelectContent>
                        </Select>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
