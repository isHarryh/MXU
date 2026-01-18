import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, X, Settings, Sun, Moon, Check, LayoutGrid } from 'lucide-react';
import { useAppStore } from '@/stores/appStore';
import clsx from 'clsx';

export function TabBar() {
  const { t } = useTranslation();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  
  const {
    instances,
    activeInstanceId,
    createInstance,
    removeInstance,
    setActiveInstance,
    renameInstance,
    theme,
    setTheme,
    setCurrentPage,
    projectInterface,
    resolveI18nText,
    language,
    dashboardView,
    toggleDashboardView,
  } = useAppStore();

  const handleNewTab = () => {
    createInstance();
  };

  const handleCloseTab = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (instances.length > 1) {
      removeInstance(id);
    }
  };

  const handleDoubleClick = (e: React.MouseEvent, id: string, name: string) => {
    e.stopPropagation();
    setEditingId(id);
    setEditName(name);
  };

  const handleSaveEdit = () => {
    if (editingId && editName.trim()) {
      renameInstance(editingId, editName.trim());
    }
    setEditingId(null);
    setEditName('');
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setEditName('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSaveEdit();
    } else if (e.key === 'Escape') {
      handleCancelEdit();
    }
  };

  const toggleTheme = () => {
    setTheme(theme === 'light' ? 'dark' : 'light');
  };

  const langKey = language === 'zh-CN' ? 'zh_cn' : 'en_us';

  return (
    <div className="flex items-center h-10 bg-bg-secondary border-b border-border select-none">
      {/* 标签页区域 */}
      <div className="flex-1 flex items-center h-full overflow-x-auto">
        {instances.map((instance) => (
          <div
            key={instance.id}
            onClick={() => setActiveInstance(instance.id)}
            onDoubleClick={(e) => handleDoubleClick(e, instance.id, instance.name)}
            className={clsx(
              'group flex items-center gap-2 h-full px-4 cursor-pointer border-r border-border transition-colors min-w-[120px] max-w-[200px]',
              instance.id === activeInstanceId
                ? 'bg-bg-primary text-accent border-b-2 border-b-accent'
                : 'bg-bg-tertiary text-text-secondary hover:bg-bg-hover border-b-2 border-b-transparent'
            )}
          >
            {editingId === instance.id ? (
              <div className="flex-1 flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                <input
                  type="text"
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  onKeyDown={handleKeyDown}
                  onBlur={handleSaveEdit}
                  autoFocus
                  className={clsx(
                    'flex-1 w-full px-1 py-0.5 text-sm rounded border border-accent',
                    'bg-bg-primary text-text-primary',
                    'focus:outline-none'
                  )}
                />
                <button
                  onMouseDown={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    handleSaveEdit();
                  }}
                  className="p-0.5 rounded hover:bg-success/10 text-success"
                >
                  <Check className="w-3 h-3" />
                </button>
                <button
                  onMouseDown={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    handleCancelEdit();
                  }}
                  className="p-0.5 rounded hover:bg-error/10 text-error"
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ) : (
              <>
                <span className="flex-1 truncate text-sm" title={t('titleBar.renameInstance')}>
                  {instance.name}
                </span>
                {instances.length > 1 && (
                  <button
                    onClick={(e) => handleCloseTab(e, instance.id)}
                    className={clsx(
                      'p-0.5 rounded opacity-0 group-hover:opacity-100 transition-opacity',
                      'hover:bg-bg-active'
                    )}
                    title={t('titleBar.closeTab')}
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                )}
              </>
            )}
          </div>
        ))}
        
        {/* 新建标签按钮 */}
        <button
          onClick={handleNewTab}
          className="flex items-center justify-center w-8 h-full hover:bg-bg-hover transition-colors"
          title={t('titleBar.newTab')}
        >
          <Plus className="w-4 h-4 text-text-secondary" />
        </button>
      </div>

      {/* 项目标题 */}
      <div className="px-4 text-sm font-medium text-text-secondary">
        {projectInterface?.title || 
         resolveI18nText(projectInterface?.label, langKey) || 
         projectInterface?.name || 
         'MXU'}
      </div>

      {/* 工具按钮 */}
      <div className="flex items-center gap-1 px-2">
        <button
          onClick={toggleDashboardView}
          className={clsx(
            'p-2 rounded-md transition-colors',
            dashboardView
              ? 'bg-accent/10 text-accent'
              : 'hover:bg-bg-hover text-text-secondary'
          )}
          title={t('dashboard.toggle')}
        >
          <LayoutGrid className="w-4 h-4" />
        </button>
        <button
          onClick={toggleTheme}
          className="p-2 rounded-md hover:bg-bg-hover transition-colors"
          title={theme === 'light' ? t('settings.themeDark') : t('settings.themeLight')}
        >
          {theme === 'light' ? (
            <Moon className="w-4 h-4 text-text-secondary" />
          ) : (
            <Sun className="w-4 h-4 text-text-secondary" />
          )}
        </button>
        <button
          onClick={() => setCurrentPage('settings')}
          className="p-2 rounded-md hover:bg-bg-hover transition-colors"
          title={t('titleBar.settings')}
        >
          <Settings className="w-4 h-4 text-text-secondary" />
        </button>
      </div>
    </div>
  );
}
