import { useTranslation } from 'react-i18next';
import { ChevronRight, RefreshCw, PackageCheck } from 'lucide-react';
import { useAppStore, type UpdateInfo, type DownloadProgress, type DownloadStatus } from '@/stores/appStore';
import { simpleMarkdownToHtml } from '@/services/contentResolver';
import clsx from 'clsx';

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

interface VersionInfoProps {
  currentVersion: string;
  latestVersion: string;
  channel?: string;
  /** 是否为已更新完成模式，显示"更新前版本"和"当前版本" */
  isUpdated?: boolean;
}

/** 版本信息组件 */
export function VersionInfo({ currentVersion, latestVersion, channel, isUpdated = false }: VersionInfoProps) {
  const { t } = useTranslation();
  
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-sm">
        <span className="text-text-muted">
          {isUpdated ? t('mirrorChyan.previousVersion') : t('mirrorChyan.currentVersion')}
        </span>
        <span className="font-mono text-text-secondary">{currentVersion}</span>
      </div>
      <div className="flex items-center justify-between text-sm">
        <span className="text-text-muted">
          {isUpdated ? t('mirrorChyan.currentVersion') : t('mirrorChyan.latestVersion')}
        </span>
        <div className="flex items-center gap-2">
          <span className="font-mono text-accent font-semibold">{latestVersion}</span>
          {channel && channel !== 'stable' && (
            <span className="px-1.5 py-0.5 bg-warning/20 text-warning text-xs rounded font-medium">
              {channel}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

interface ReleaseNotesProps {
  releaseNote?: string;
  /** 是否显示标题，默认 true */
  showTitle?: boolean;
  /** 是否使用折叠样式的标题（带 ChevronRight 图标） */
  collapsibleTitle?: boolean;
  /** 最大高度类名，如 'max-h-32' 或 'max-h-48' */
  maxHeightClass?: string;
  /** 背景色类名 */
  bgClass?: string;
  /** 文字大小类名 */
  textSizeClass?: string;
  /** 外层容器类名，用于 flex 布局等 */
  className?: string;
  /** 使用 flex 布局填充可用高度 */
  fillHeight?: boolean;
}

/** 更新日志组件 */
export function ReleaseNotes({ 
  releaseNote, 
  showTitle = true,
  collapsibleTitle = false,
  maxHeightClass = 'max-h-48',
  bgClass = 'bg-bg-tertiary',
  textSizeClass = 'text-sm',
  className,
  fillHeight = false,
}: ReleaseNotesProps) {
  const { t } = useTranslation();
  
  return (
    <div className={clsx(fillHeight ? 'flex flex-col min-h-0' : 'space-y-2', className)}>
      {showTitle && (
        collapsibleTitle ? (
          <div className={clsx('flex items-center gap-1 text-sm font-medium text-text-primary', fillHeight && 'shrink-0 mb-2')}>
            <ChevronRight className="w-3 h-3" />
            <span>{t('mirrorChyan.releaseNotes')}</span>
          </div>
        ) : (
          <p className={clsx('text-sm font-medium text-text-primary', fillHeight && 'shrink-0 mb-2')}>
            {t('mirrorChyan.releaseNotes')}
          </p>
        )
      )}
      <div className={clsx(
        'overflow-y-auto rounded-lg p-3 border border-border',
        fillHeight ? 'flex-1 min-h-0' : maxHeightClass,
        bgClass
      )}>
        {releaseNote ? (
          <div
            className={clsx(textSizeClass, 'text-text-secondary prose prose-sm max-w-none leading-relaxed')}
            dangerouslySetInnerHTML={{ __html: simpleMarkdownToHtml(releaseNote) }}
          />
        ) : (
          <p className={clsx(textSizeClass, 'text-text-muted italic')}>{t('mirrorChyan.noReleaseNotes')}</p>
        )}
      </div>
    </div>
  );
}

interface DownloadProgressBarProps {
  downloadStatus: DownloadStatus;
  downloadProgress: DownloadProgress | null;
  fileSize?: number;
  downloadSource?: 'mirrorchyan' | 'github';
  /** 下载完成时的安装按钮点击回调 */
  onInstallClick?: () => void;
  /** 下载失败时的重试按钮点击回调 */
  onRetryClick?: () => void;
  /** 是否显示操作按钮（立即安装/重试），默认 true */
  showActions?: boolean;
  /** 进度条背景色类名 */
  progressBgClass?: string;
}

/** 下载进度组件 */
export function DownloadProgressBar({
  downloadStatus,
  downloadProgress,
  fileSize,
  downloadSource,
  onInstallClick,
  onRetryClick,
  showActions = true,
  progressBgClass = 'bg-bg-tertiary',
}: DownloadProgressBarProps) {
  const { t } = useTranslation();
  
  if (downloadStatus === 'idle') return null;
  
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-xs text-text-muted">
        <span>
          {downloadStatus === 'downloading' && t('mirrorChyan.downloading')}
          {downloadStatus === 'completed' && t('mirrorChyan.downloadComplete')}
          {downloadStatus === 'failed' && t('mirrorChyan.downloadFailed')}
        </span>
        {downloadProgress && (
          <span>{downloadProgress.progress.toFixed(1)}%</span>
        )}
      </div>
      
      {/* 进度条 */}
      <div className={clsx('h-2 rounded-full overflow-hidden', progressBgClass)}>
        <div
          className={clsx(
            'h-full rounded-full transition-all duration-100',
            downloadStatus === 'completed' && 'bg-success',
            downloadStatus === 'downloading' && 'bg-accent',
            downloadStatus === 'failed' && 'bg-error'
          )}
          style={{ width: `${downloadProgress?.progress || 0}%` }}
        />
      </div>

      {/* 下载详情 */}
      <div className="flex items-center justify-between text-xs text-text-muted">
        <span>
          {downloadProgress && downloadProgress.totalSize > 0 
            ? `${formatSize(downloadProgress.downloadedSize)} / ${formatSize(downloadProgress.totalSize)}`
            : fileSize 
              ? formatSize(fileSize)
              : ''
          }
        </span>
        {downloadStatus === 'downloading' && downloadProgress && downloadProgress.speed > 0 && (
          <span>{formatSpeed(downloadProgress.speed)}</span>
        )}
        {showActions && downloadStatus === 'completed' && onInstallClick && (
          <button
            onClick={onInstallClick}
            className="flex items-center gap-1 text-accent hover:underline"
          >
            <PackageCheck className="w-3 h-3" />
            <span>{t('mirrorChyan.installNow')}</span>
          </button>
        )}
        {showActions && downloadStatus === 'failed' && onRetryClick && (
          <button
            onClick={onRetryClick}
            className="flex items-center gap-1 text-accent hover:underline"
          >
            <RefreshCw className="w-3 h-3" />
            <span>{t('mirrorChyan.retry')}</span>
          </button>
        )}
      </div>
      
      {/* 下载来源标识 */}
      {downloadSource && (
        <div className="text-xs text-text-muted">
          {downloadSource === 'github' 
            ? t('mirrorChyan.downloadFromGitHub')
            : t('mirrorChyan.downloadFromMirrorChyan')
          }
        </div>
      )}
    </div>
  );
}

// 导出格式化函数供其他组件使用
export { formatSize, formatSpeed };
