import React from 'react';
import { Home, Calendar, Server, List, Settings, Moon, Sun, Menu, X } from 'lucide-react';
import { Button } from './ui/button';
import { Sheet, SheetContent, SheetTrigger, SheetHeader, SheetTitle, SheetDescription } from './ui/sheet';
import { useTheme } from './ThemeProvider';
import { useLanguage } from './i18n/LanguageProvider';
import { useMobile } from './hooks/use-mobile';
import { cn } from './ui/utils';

type NavItem = {
  id: string;
  labelKey: keyof typeof import('./i18n/index').translations['zh-cn']['nav'];
  icon: React.ComponentType<{ className?: string }>;
};

const navItems: NavItem[] = [
  { id: 'dashboard', labelKey: 'dashboard', icon: Home },
  { id: 'plans', labelKey: 'plans', icon: Calendar },
  { id: 'services', labelKey: 'services', icon: Server },
  { id: 'tasks', labelKey: 'tasks', icon: List },
  { id: 'settings', labelKey: 'settings', icon: Settings },
];

interface SidebarProps {
  currentPage: string;
  onPageChange: (pageId: string) => void;
}

interface SidebarContentProps extends SidebarProps {
  onNavigate?: () => void;
}

function SidebarContent({ currentPage, onPageChange, onNavigate }: SidebarContentProps) {
  const { theme, toggleTheme } = useTheme();
  const { t } = useLanguage();

  const handleNavigation = (pageId: string) => {
    onPageChange(pageId);
    onNavigate?.();
  };

  return (
    <div className="h-full bg-sidebar border-r border-sidebar-border flex flex-col">
      {/* Header */}
      <div className="p-6 border-b border-sidebar-border">
        <div className="flex items-center justify-between">
          <h1 className="text-sidebar-foreground">Bucky Backup Suite</h1>
          <Button
            variant="ghost"
            size="icon"
            onClick={toggleTheme}
            aria-label="Toggle theme"
            className="text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
          >
            {theme === 'light' ? <Moon className="w-4 h-4" /> : <Sun className="w-4 h-4" />}
          </Button>
        </div>
        <p className="text-sm text-sidebar-accent-foreground mt-1">专业备份解决方案</p>
      </div>

      {/* Navigation */}
      <nav className="flex-1 p-4">
        <div className="space-y-2">
          {navItems.map((item) => (
            <Button
              key={item.id}
              variant={currentPage === item.id ? "default" : "ghost"}
              className={cn(
                "w-full justify-start gap-3 h-11 px-3",
                currentPage === item.id 
                  ? "bg-sidebar-primary text-sidebar-primary-foreground" 
                  : "text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
              )}
              onClick={() => handleNavigation(item.id)}
            >
              <item.icon className="w-5 h-5" />
              {t.nav[item.labelKey]}
            </Button>
          ))}
        </div>
      </nav>

      {/* Theme toggle moved to header */}
    </div>
  );
}

export function Sidebar({ currentPage, onPageChange }: SidebarProps) {
  const isMobile = useMobile();
  const { t } = useLanguage();

  if (isMobile) {
    return (
      <>
        {/* Mobile header with menu and page title */}
        <div className="fixed top-0 left-0 right-0 z-50 bg-background border-b h-16 flex items-center px-4 md:hidden">
          <Sheet>
            <SheetTrigger asChild>
              <Button variant="ghost" size="icon" className="mr-3">
                <Menu className="h-5 w-5" />
              </Button>
            </SheetTrigger>
            <SheetContent side="left" className="p-0 w-64">
              <SheetHeader className="sr-only">
                <SheetTitle>Navigation Menu</SheetTitle>
                <SheetDescription>Navigate between different sections of the application</SheetDescription>
              </SheetHeader>
              <SidebarContent 
                currentPage={currentPage} 
                onPageChange={onPageChange}
                onNavigate={() => {
                  // Close sheet on navigation
                  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
                }}
              />
            </SheetContent>
          </Sheet>
          <h1 className="font-medium">{t.nav[navItems.find(item => item.id === currentPage)?.labelKey || 'dashboard']}</h1>
        </div>
      </>
    );
  }

  return (
    <div className="w-64">
      <SidebarContent currentPage={currentPage} onPageChange={onPageChange} />
    </div>
  );
}
