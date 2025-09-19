import React, { useState, useEffect } from 'react';
import { ChevronRight, ChevronDown, Folder, FolderOpen } from 'lucide-react';
import { Button } from './ui/button';
import { ScrollArea } from './ui/scroll-area';
import { Checkbox } from './ui/checkbox';

interface DirectoryNode {
  path: string;
  name: string;
  isDirectory: boolean;
  children?: DirectoryNode[];
  expanded?: boolean;
  selected?: boolean;
}

interface DirectorySelectorProps {
  multiple?: boolean;
  value?: string | string[];
  onChange?: (value: string) => void;
  onSelectionChange?: (selected: string[]) => void;
  placeholder?: string;
}

// 模拟API调用获取目录结构
const mockGetDirectories = async (path?: string): Promise<DirectoryNode[]> => {
  await new Promise(resolve => setTimeout(resolve, 300));
  
  if (!path || path === '/') {
    return [
      { path: 'C:', name: 'C:', isDirectory: true },
      { path: 'D:', name: 'D:', isDirectory: true },
      { path: 'E:', name: 'E:', isDirectory: true },
    ];
  }
  
  if (path === 'C:') {
    return [
      { path: 'C:\\Users', name: 'Users', isDirectory: true },
      { path: 'C:\\Program Files', name: 'Program Files', isDirectory: true },
      { path: 'C:\\Windows', name: 'Windows', isDirectory: true },
      { path: 'C:\\Temp', name: 'Temp', isDirectory: true },
    ];
  }
  
  if (path === 'C:\\Users') {
    return [
      { path: 'C:\\Users\\Administrator', name: 'Administrator', isDirectory: true },
      { path: 'C:\\Users\\Public', name: 'Public', isDirectory: true },
      { path: 'C:\\Users\\Default', name: 'Default', isDirectory: true },
    ];
  }
  
  if (path === 'C:\\Users\\Administrator') {
    return [
      { path: 'C:\\Users\\Administrator\\Desktop', name: 'Desktop', isDirectory: true },
      { path: 'C:\\Users\\Administrator\\Documents', name: 'Documents', isDirectory: true },
      { path: 'C:\\Users\\Administrator\\Downloads', name: 'Downloads', isDirectory: true },
      { path: 'C:\\Users\\Administrator\\Pictures', name: 'Pictures', isDirectory: true },
    ];
  }
  
  if (path === 'D:') {
    return [
      { path: 'D:\\Projects', name: 'Projects', isDirectory: true },
      { path: 'D:\\Backups', name: 'Backups', isDirectory: true },
      { path: 'D:\\Media', name: 'Media', isDirectory: true },
      { path: 'D:\\Data', name: 'Data', isDirectory: true },
    ];
  }
  
  return [];
};

export function DirectorySelector({ multiple = false, value, onChange, onSelectionChange, placeholder }: DirectorySelectorProps) {
  const [directories, setDirectories] = useState<DirectoryNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<string[]>([]);

  useEffect(() => {
    loadDirectories();
  }, []);

  useEffect(() => {
    if (value) {
      if (Array.isArray(value)) {
        setSelected(value);
      } else {
        setSelected([value]);
      }
    }
  }, [value]);

  const loadDirectories = async (path?: string) => {
    setLoading(true);
    try {
      const dirs = await mockGetDirectories(path);
      if (!path) {
        setDirectories(dirs);
      } else {
        updateDirectoryChildren(directories, path, dirs);
      }
    } catch (error) {
      console.error('Failed to load directories:', error);
    } finally {
      setLoading(false);
    }
  };

  const updateDirectoryChildren = (nodes: DirectoryNode[], targetPath: string, children: DirectoryNode[]): DirectoryNode[] => {
    return nodes.map(node => {
      if (node.path === targetPath) {
        return { ...node, children, expanded: true };
      } else if (node.children) {
        return { ...node, children: updateDirectoryChildren(node.children, targetPath, children) };
      }
      return node;
    });
  };

  const toggleExpand = async (node: DirectoryNode) => {
    if (!node.children) {
      await loadDirectories(node.path);
    } else {
      const newDirectories = updateNodeExpanded(directories, node.path, !node.expanded);
      setDirectories(newDirectories);
    }
  };

  const updateNodeExpanded = (nodes: DirectoryNode[], targetPath: string, expanded: boolean): DirectoryNode[] => {
    return nodes.map(node => {
      if (node.path === targetPath) {
        return { ...node, expanded };
      } else if (node.children) {
        return { ...node, children: updateNodeExpanded(node.children, targetPath, expanded) };
      }
      return node;
    });
  };

  const handleSelect = (path: string) => {
    let newSelected: string[];
    
    if (multiple) {
      if (selected.includes(path)) {
        newSelected = selected.filter(p => p !== path);
      } else {
        newSelected = [...selected, path];
      }
    } else {
      newSelected = [path];
    }
    
    setSelected(newSelected);
    
    if (onChange) {
      if (multiple) {
        // For multiple mode, use onSelectionChange callback
        if (onSelectionChange) {
          onSelectionChange(newSelected);
        }
      } else {
        // For single mode, use onChange callback
        onChange(newSelected[0] || '');
      }
    }
    
    if (onSelectionChange) {
      onSelectionChange(newSelected);
    }
  };

  const renderNode = (node: DirectoryNode, level: number = 0) => {
    const isExpanded = node.expanded;
    const isSelected = selected.includes(node.path);
    const hasChildren = node.children !== undefined || node.isDirectory;

    return (
      <div key={node.path} className="select-none">
        <div 
          className={`flex items-center gap-2 px-2 py-1 hover:bg-accent/50 cursor-pointer rounded-sm ${
            isSelected ? 'bg-primary/10' : ''
          }`}
          style={{ paddingLeft: `${8 + level * 16}px` }}
        >
          {hasChildren && (
            <Button
              variant="ghost"
              size="sm"
              className="h-4 w-4 p-0 hover:bg-transparent"
              onClick={(e) => {
                e.stopPropagation();
                toggleExpand(node);
              }}
            >
              {isExpanded ? (
                <ChevronDown className="h-3 w-3" />
              ) : (
                <ChevronRight className="h-3 w-3" />
              )}
            </Button>
          )}
          {!hasChildren && <div className="w-4" />}
          
          {multiple && (
            <Checkbox
              checked={isSelected}
              onCheckedChange={() => handleSelect(node.path)}
              onClick={(e) => e.stopPropagation()}
            />
          )}
          
          <div 
            className="flex items-center gap-2 flex-1 min-w-0"
            onClick={() => handleSelect(node.path)}
          >
            {isExpanded ? (
              <FolderOpen className="h-4 w-4 text-muted-foreground flex-shrink-0" />
            ) : (
              <Folder className="h-4 w-4 text-muted-foreground flex-shrink-0" />
            )}
            <span className="truncate text-sm">{node.name}</span>
          </div>
        </div>
        
        {isExpanded && node.children && (
          <div>
            {node.children.map(child => renderNode(child, level + 1))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="border rounded-md">
      <ScrollArea className="h-[300px] p-2">
        {loading && directories.length === 0 ? (
          <div className="flex items-center justify-center h-20 text-sm text-muted-foreground">
            加载中...
          </div>
        ) : (
          <div className="space-y-1">
            {directories.map(node => renderNode(node))}
          </div>
        )}
      </ScrollArea>
    </div>
  );
}