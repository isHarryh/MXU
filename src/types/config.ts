// MXU 配置文件结构 (mxu.json)

import type { OptionValue } from './interface';

// 保存的任务配置
export interface SavedTask {
  id: string;
  taskName: string;           // 对应 interface 中的 task.name
  customName?: string;        // 用户自定义名称
  enabled: boolean;
  optionValues: Record<string, OptionValue>;
}

// 保存的设备信息
export interface SavedDeviceInfo {
  // ADB 设备：保存设备名称
  adbDeviceName?: string;
  // Win32/Gamepad：保存窗口名称
  windowName?: string;
  // PlayCover：保存地址
  playcoverAddress?: string;
}

// 保存的实例配置
export interface SavedInstance {
  id: string;
  name: string;
  controllerId?: string;
  resourceId?: string;
  // 保存的控制器和资源名称
  controllerName?: string;
  resourceName?: string;
  // 保存的设备信息，用于自动重连
  savedDevice?: SavedDeviceInfo;
  tasks: SavedTask[];
}

// 窗口大小配置
export interface WindowSize {
  width: number;
  height: number;
}

// 应用设置
export interface AppSettings {
  theme: 'light' | 'dark';
  language: 'zh-CN' | 'en-US';
  windowSize?: WindowSize;
}

// MXU 配置文件完整结构
export interface MxuConfig {
  version: string;
  instances: SavedInstance[];
  settings: AppSettings;
}

// 默认窗口大小
export const defaultWindowSize: WindowSize = {
  width: 1000,
  height: 600,
};

// 默认配置
export const defaultConfig: MxuConfig = {
  version: '1.0',
  instances: [],
  settings: {
    theme: 'light',
    language: 'zh-CN',
    windowSize: defaultWindowSize,
  },
};
