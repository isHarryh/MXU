import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Download, ChevronRight, Maximize2 } from 'lucide-react';
import { useAppStore, type UpdateInfo } from '@/stores/appStore';
import { simpleMarkdownToHtml } from '@/services/contentResolver';
import clsx from 'clsx';

interface UpdatePanelProps {
  onClose: () => void;
  anchorRef: React.RefObject<HTMLButtonElement | null>;
}

// 模拟下载状态
interface DownloadState {
  isDownloading: boolean;
  progress: number; // 0-100
  downloadedSize: number; // bytes
  totalSize: number; // bytes
  speed: number; // bytes per second
}

// 格式化文件大小
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

// 格式化速度
function formatSpeed(bytesPerSecond: number): string {
  if (bytesPerSecond < 1024) return `${bytesPerSecond} B/s`;
  if (bytesPerSecond < 1024 * 1024) return `${(bytesPerSecond / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSecond / (1024 * 1024)).toFixed(1)} MB/s`;
}

export function UpdatePanel({ onClose, anchorRef }: UpdatePanelProps) {
  const { t } = useTranslation();
  const panelRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ top: 0, right: 0 });
  const [isModalMode, setIsModalMode] = useState(false);
  
  const {
    updateInfo,
    projectInterface,
  } = useAppStore();

  // 模拟下载状态
  const [downloadState, setDownloadState] = useState<DownloadState>({
    isDownloading: true,
    progress: 0,
    downloadedSize: 0,
    totalSize: 128.5 * 1024 * 1024, // 128.5 MB
    speed: 0,
  });

  // 模拟下载进度
  useEffect(() => {
    if (!downloadState.isDownloading) return;

    const interval = setInterval(() => {
      setDownloadState(prev => {
        // 随机波动的下载速度（1.5-3.5 MB/s）
        const baseSpeed = 2.5 * 1024 * 1024;
        const speedVariation = (Math.random() - 0.5) * 2 * 1024 * 1024;
        const newSpeed = Math.max(0.5 * 1024 * 1024, baseSpeed + speedVariation);
        
        const increment = newSpeed * 0.1; // 每100ms的增量
        const newDownloaded = Math.min(prev.downloadedSize + increment, prev.totalSize);
        const newProgress = (newDownloaded / prev.totalSize) * 100;

        // 下载完成时停止
        if (newProgress >= 100) {
          return {
            ...prev,
            progress: 100,
            downloadedSize: prev.totalSize,
            speed: 0,
            isDownloading: false,
          };
        }

        return {
          ...prev,
          progress: newProgress,
          downloadedSize: newDownloaded,
          speed: newSpeed,
        };
      });
    }, 100);

    return () => clearInterval(interval);
  }, [downloadState.isDownloading]);

  // 计算面板位置
  useEffect(() => {
    if (anchorRef.current) {
      const rect = anchorRef.current.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 8,
        right: window.innerWidth - rect.right,
      });
    }
  }, [anchorRef]);

  // 点击外部关闭（气泡模式下点击外部关闭，模态框模式下点击背景关闭）
  useEffect(() => {
    if (isModalMode) return; // 模态框模式下不需要这个处理
    
    const handleClickOutside = (e: MouseEvent) => {
      if (
        panelRef.current &&
        !panelRef.current.contains(e.target as Node) &&
        anchorRef.current &&
        !anchorRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose, anchorRef, isModalMode]);

  // ESC 键关闭（模态框模式下先退出模态框，再按一次关闭整个面板）
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (isModalMode) {
          setIsModalMode(false);
        } else {
          onClose();
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose, isModalMode]);

  if (!updateInfo?.hasUpdate) return null;

  const currentVersion = projectInterface?.version || '';

  // 模态框模式 - 大型居中弹窗显示完整更新日志
  if (isModalMode) {
    return (
      <div 
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm animate-in fade-in duration-200"
        onClick={() => setIsModalMode(false)}
      >
        <div
          ref={panelRef}
          className="w-[600px] max-w-[90vw] max-h-[80vh] bg-bg-secondary rounded-xl shadow-2xl border border-border overflow-hidden animate-in zoom-in-95 duration-200 flex flex-col"
          onClick={(e) => e.stopPropagation()}
        >
          {/* 标题栏 */}
          <div className="flex items-center justify-between px-4 py-2.5 bg-bg-tertiary border-b border-border shrink-0">
            <div className="flex items-center gap-2">
              <Download className="w-4 h-4 text-accent" />
              <span className="text-sm font-medium text-text-primary">
                {t('mirrorChyan.releaseNotes')}
              </span>
              <span className="font-mono text-sm text-accent font-semibold">{updateInfo.versionName}</span>
              {updateInfo.channel && updateInfo.channel !== 'stable' && (
                <span className="px-1.5 py-0.5 bg-warning/20 text-warning text-xs rounded font-medium">
                  {updateInfo.channel}
                </span>
              )}
            </div>
            <button
              onClick={() => setIsModalMode(false)}
              className="p-1.5 rounded-lg hover:bg-bg-hover transition-colors"
            >
              <X className="w-4 h-4 text-text-muted" />
            </button>
          </div>

          {/* 更新日志内容 */}
          <div className="flex-1 overflow-y-auto p-4 min-h-0">
            {updateInfo.releaseNote ? (
              <div
                className="text-sm text-text-secondary prose prose-sm max-w-none leading-relaxed"
                dangerouslySetInnerHTML={{ __html: simpleMarkdownToHtml(updateInfo.releaseNote) }}
              />
            ) : (
              <p className="text-sm text-text-muted italic">{t('mirrorChyan.noReleaseNotes')}</p>
            )}
          </div>
        </div>
      </div>
    );
  }

  // 气泡模式 - 紧凑的弹出面板
  return (
    <div
      ref={panelRef}
      className="fixed z-50 w-80 bg-bg-secondary rounded-xl shadow-lg border border-border overflow-hidden animate-in"
      style={{
        top: position.top,
        right: position.right,
      }}
    >
      {/* 标题栏 */}
      <div className="flex items-center justify-between px-4 py-3 bg-bg-tertiary border-b border-border">
        <div className="flex items-center gap-2">
          <Download className="w-4 h-4 text-accent" />
          <span className="text-sm font-medium text-text-primary">
            {t('mirrorChyan.newVersion')}
          </span>
        </div>
        <button
          onClick={onClose}
          className="p-1 rounded-md hover:bg-bg-hover transition-colors"
        >
          <X className="w-4 h-4 text-text-muted" />
        </button>
      </div>

      {/* 内容区 */}
      <div className="p-4 space-y-4">
        {/* 版本信息 */}
        <div className="space-y-2">
          <div className="flex items-center justify-between text-sm">
            <span className="text-text-muted">{t('mirrorChyan.currentVersion')}</span>
            <span className="font-mono text-text-secondary">{currentVersion}</span>
          </div>
          <div className="flex items-center justify-between text-sm">
            <span className="text-text-muted">{t('mirrorChyan.latestVersion')}</span>
            <div className="flex items-center gap-2">
              <span className="font-mono text-accent font-semibold">{updateInfo.versionName}</span>
              {updateInfo.channel && updateInfo.channel !== 'stable' && (
                <span className="px-1.5 py-0.5 bg-warning/20 text-warning text-xs rounded font-medium">
                  {updateInfo.channel}
                </span>
              )}
            </div>
          </div>
        </div>

        {/* 更新日志 */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1 text-sm font-medium text-text-primary">
              <ChevronRight className="w-3 h-3" />
              <span>{t('mirrorChyan.releaseNotes')}</span>
            </div>
            {updateInfo.releaseNote && (
              <button
                onClick={() => setIsModalMode(true)}
                className="flex items-center gap-1 px-2 py-1 text-xs text-accent hover:bg-accent/10 rounded-md transition-colors"
                title={t('mirrorChyan.viewDetails')}
              >
                <Maximize2 className="w-3 h-3" />
                <span>{t('mirrorChyan.viewDetails')}</span>
              </button>
            )}
          </div>
          <div className="max-h-32 overflow-y-auto bg-bg-tertiary rounded-lg p-3 border border-border">
            {updateInfo.releaseNote ? (
              <div
                className="text-xs text-text-secondary prose prose-sm max-w-none leading-relaxed"
                dangerouslySetInnerHTML={{ __html: simpleMarkdownToHtml(updateInfo.releaseNote) }}
              />
            ) : (
              <p className="text-xs text-text-muted italic">{t('mirrorChyan.noReleaseNotes')}</p>
            )}
          </div>
        </div>

        {/* 下载进度 */}
        <div className="space-y-2 pt-2 border-t border-border">
          <div className="flex items-center justify-between text-xs text-text-muted">
            <span>{t('mirrorChyan.downloading')}</span>
            <span>{downloadState.progress.toFixed(1)}%</span>
          </div>
          
          {/* 进度条 */}
          <div className="h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              className={clsx(
                'h-full rounded-full transition-all duration-100',
                downloadState.progress >= 100
                  ? 'bg-success'
                  : 'bg-accent'
              )}
              style={{ width: `${downloadState.progress}%` }}
            />
          </div>

          {/* 下载详情 */}
          <div className="flex items-center justify-between text-xs text-text-muted">
            <span>
              {formatSize(downloadState.downloadedSize)} / {formatSize(downloadState.totalSize)}
            </span>
            {downloadState.isDownloading && downloadState.speed > 0 && (
              <span>{formatSpeed(downloadState.speed)}</span>
            )}
            {!downloadState.isDownloading && downloadState.progress >= 100 && (
              <span className="text-success">{t('mirrorChyan.downloadComplete')}</span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
