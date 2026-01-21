import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Minus, Square, X, Copy } from 'lucide-react';
import { useAppStore } from '@/stores/appStore';
import { getInterfaceLangKey } from '@/i18n';
import clsx from 'clsx';

// 检测是否在 Tauri 环境中
const isTauri = () => {
  return typeof window !== 'undefined' && '__TAURI__' in window;
};

export function TitleBar() {
  const { t } = useTranslation();
  const [isMaximized, setIsMaximized] = useState(false);

  const { projectInterface, interfaceTranslations, language, resolveI18nText } = useAppStore();

  const langKey = getInterfaceLangKey(language);

  // 监听窗口最大化状态变化
  useEffect(() => {
    if (!isTauri()) return;

    let unlisten: (() => void) | null = null;

    const setup = async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const appWindow = getCurrentWindow();

        // 获取初始状态
        setIsMaximized(await appWindow.isMaximized());

        // 监听窗口状态变化
        unlisten = await appWindow.onResized(async () => {
          setIsMaximized(await appWindow.isMaximized());
        });
      } catch (err) {
        console.warn('Failed to setup window state listener:', err);
      }
    };

    setup();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleMinimize = async () => {
    if (!isTauri()) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().minimize();
    } catch (err) {
      console.warn('Failed to minimize window:', err);
    }
  };

  const handleToggleMaximize = async () => {
    if (!isTauri()) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().toggleMaximize();
    } catch (err) {
      console.warn('Failed to toggle maximize:', err);
    }
  };

  const handleClose = async () => {
    if (!isTauri()) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().close();
    } catch (err) {
      console.warn('Failed to close window:', err);
    }
  };

  // 计算窗口标题
  const getWindowTitle = () => {
    if (!projectInterface) return 'MXU';

    const translations = interfaceTranslations[langKey];

    // 优先使用 title 字段（支持国际化），否则使用 name + version
    if (projectInterface.title) {
      return resolveI18nText(projectInterface.title, langKey);
    }

    const version = projectInterface.version;
    return version ? `${projectInterface.name} ${version}` : projectInterface.name;
  };

  return (
    <div
      data-tauri-drag-region
      className="h-8 flex items-center justify-between bg-bg-secondary border-b border-border select-none shrink-0"
    >
      {/* 左侧：窗口图标和标题 */}
      <div className="flex items-center h-full" data-tauri-drag-region>
        {/* 窗口图标 */}
        {projectInterface?.icon && (
          <div className="w-8 h-8 flex items-center justify-center">
            <img
              src={`${useAppStore.getState().basePath}/${resolveI18nText(projectInterface.icon, langKey)}`}
              alt="icon"
              className="w-4 h-4"
              onError={(e) => {
                // 图标加载失败时隐藏
                (e.target as HTMLImageElement).style.display = 'none';
              }}
            />
          </div>
        )}
        <span
          className="text-xs text-text-secondary px-2 truncate max-w-[200px]"
          data-tauri-drag-region
        >
          {getWindowTitle()}
        </span>
      </div>

      {/* 右侧：窗口控制按钮 */}
      {isTauri() && (
        <div className="flex h-full">
          <button
            onClick={handleMinimize}
            className="w-12 h-full flex items-center justify-center text-text-secondary hover:bg-bg-hover transition-colors"
            title={t('windowControls.minimize')}
          >
            <Minus className="w-4 h-4" />
          </button>
          <button
            onClick={handleToggleMaximize}
            className="w-12 h-full flex items-center justify-center text-text-secondary hover:bg-bg-hover transition-colors"
            title={isMaximized ? t('windowControls.restore') : t('windowControls.maximize')}
          >
            {isMaximized ? <Copy className="w-3.5 h-3.5 rotate-180" /> : <Square className="w-3 h-3" />}
          </button>
          <button
            onClick={handleClose}
            className="w-12 h-full flex items-center justify-center text-text-secondary hover:bg-red-500 hover:text-white transition-colors"
            title={t('windowControls.close')}
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      )}
    </div>
  );
}
