import React, { useState } from "react";
import { ThemeProvider } from "./components/ThemeProvider";
import { LanguageProvider } from "./components/i18n/LanguageProvider";
import { Sidebar } from "./components/Sidebar";
import { Dashboard } from "./components/Dashboard";
import { BackupPlans } from "./components/BackupPlans";
import { ServiceManagement } from "./components/ServiceManagement";
import { TaskList } from "./components/TaskList";
import { Settings } from "./components/Settings";
import { CreatePlanWizard } from "./components/CreatePlanWizard";
import { AddServiceWizard } from "./components/AddServiceWizard";
import { RestoreWizard } from "./components/RestoreWizard";
import { EditPlanWizard } from "./components/EditPlanWizard";
import { PlanDetails } from "./components/PlanDetails";
import { useMobile } from "./components/hooks/use_mobile";

interface NavigationState {
    page: string;
    data?: any;
}

export default function App() {
    const [navigationStack, setNavigationStack] = useState<NavigationState[]>([
        { page: "dashboard" },
    ]);
    const isMobile = useMobile();

    const currentPage = navigationStack[navigationStack.length - 1];

    const navigateTo = (page: string, data?: any) => {
        setNavigationStack((prev) => [...prev, { page, data }]);
    };

    const navigateBack = () => {
        if (navigationStack.length > 1) {
            setNavigationStack((prev) => prev.slice(0, -1));
        }
    };

    const navigateToRoot = (page: string) => {
        setNavigationStack([{ page }]);
    };

    const renderCurrentPage = () => {
        switch (currentPage.page) {
            case "dashboard":
                return <Dashboard onNavigate={navigateTo} />;
            case "plans":
                return <BackupPlans onNavigate={navigateTo} />;
            case "services":
                return <ServiceManagement onNavigate={navigateTo} />;
            case "tasks":
                return <TaskList onNavigate={navigateTo} />;
            case "settings":
                return <Settings />;
            case "create-plan":
                return (
                    <CreatePlanWizard
                        onBack={navigateBack}
                        onComplete={navigateBack}
                    />
                );
            case "add-service":
                return (
                    <AddServiceWizard
                        onBack={navigateBack}
                        onComplete={navigateBack}
                    />
                );
            case "restore":
                return (
                    <RestoreWizard
                        onBack={navigateBack}
                        onComplete={navigateBack}
                        data={currentPage.data}
                    />
                );
            case "edit-plan":
                return (
                    <EditPlanWizard
                        onBack={navigateBack}
                        onComplete={navigateBack}
                        data={currentPage.data}
                    />
                );
            case "plan-details":
                return (
                    <PlanDetails
                        onBack={navigateBack}
                        onNavigate={navigateTo}
                        data={currentPage.data}
                    />
                );
            default:
                return <Dashboard onNavigate={navigateTo} />;
        }
    };

    const showSidebar = ![
        "create-plan",
        "add-service",
        "restore",
        "edit-plan",
        "plan-details",
    ].includes(currentPage.page);

    return (
        <ThemeProvider>
            <LanguageProvider>
                <div className="size-full flex bg-background">
                    {showSidebar && (
                        <Sidebar
                            currentPage={currentPage.page}
                            onPageChange={navigateToRoot}
                        />
                    )}
                    <main
                        className={`flex-1 overflow-auto ${
                            !showSidebar ? "w-full" : ""
                        }`}
                    >
                        {renderCurrentPage()}
                    </main>
                </div>
            </LanguageProvider>
        </ThemeProvider>
    );
}
