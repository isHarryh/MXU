// MaaFramework 服务层
// 封装 Tauri 命令调用，提供前端友好的 API

import { invoke } from '@tauri-apps/api/core';
import type {
  AdbDevice,
  Win32Window,
  ControllerConfig,
  ConnectionStatus,
  TaskStatus,
} from '@/types/maa';

// 检测是否在 Tauri 环境中
const isTauri = () => {
  return typeof window !== 'undefined' && '__TAURI__' in window;
};

/** MaaFramework 服务 */
export const maaService = {
  /**
   * 初始化 MaaFramework
   * @param libDir MaaFramework 库目录（可选，默认从 exe 目录/maafw 加载）
   * @returns 版本号
   */
  async init(libDir?: string): Promise<string> {
    // if (!isTauri()) {
    //   console.warn('MaaService: Not in Tauri environment');
    //   return 'mock-version';
    // }
    return await invoke<string>('maa_init', { libDir: libDir || null });
  },

  /**
   * 设置资源目录
   * @param resourceDir 资源目录路径
   */
  async setResourceDir(resourceDir: string): Promise<void> {
    if (!isTauri()) return;
    return await invoke('maa_set_resource_dir', { resourceDir });
  },

  /**
   * 获取 MaaFramework 版本
   */
  async getVersion(): Promise<string> {
    // if (!isTauri()) return 'mock-version';
    return await invoke<string>('maa_get_version');
  },

  /**
   * 查找 ADB 设备
   */
  async findAdbDevices(): Promise<AdbDevice[]> {
    // if (!isTauri()) {
    //   // 返回模拟数据用于开发
    //   return [
    //     {
    //       name: 'Mock Emulator',
    //       adb_path: 'C:\\Program Files\\Mock\\adb.exe',
    //       address: '127.0.0.1:5555',
    //       screencap_methods: 0xFFFFFFFF,
    //       input_methods: 0xFFFFFFFF,
    //       config: '{}',
    //     },
    //   ];
    // }
    return await invoke<AdbDevice[]>('maa_find_adb_devices');
  },

  /**
   * 查找 Win32 窗口
   * @param classRegex 窗口类名正则表达式（可选）
   * @param windowRegex 窗口标题正则表达式（可选）
   */
  async findWin32Windows(classRegex?: string, windowRegex?: string): Promise<Win32Window[]> {
    // if (!isTauri()) {
    //   // 返回模拟数据用于开发
    //   return [
    //     {
    //       handle: 12345,
    //       class_name: 'MockWindowClass',
    //       window_name: 'Mock Window',
    //     },
    //   ];
    // }
    return await invoke<Win32Window[]>('maa_find_win32_windows', {
      classRegex: classRegex || null,
      windowRegex: windowRegex || null,
    });
  },

  /**
   * 创建实例
   * @param instanceId 实例 ID
   */
  async createInstance(instanceId: string): Promise<void> {
    if (!isTauri()) return;
    return await invoke('maa_create_instance', { instanceId });
  },

  /**
   * 销毁实例
   * @param instanceId 实例 ID
   */
  async destroyInstance(instanceId: string): Promise<void> {
    if (!isTauri()) return;
    return await invoke('maa_destroy_instance', { instanceId });
  },

  /**
   * 连接控制器
   * @param instanceId 实例 ID
   * @param config 控制器配置
   * @param agentPath MaaAgentBinary 路径（可选）
   */
  async connectController(
    instanceId: string,
    config: ControllerConfig,
    agentPath?: string
  ): Promise<void> {
    console.log('[maaService] connectController called');
    console.log('[maaService] instanceId:', instanceId);
    console.log('[maaService] config:', JSON.stringify(config, null, 2));
    console.log('[maaService] agentPath:', agentPath);
    console.log('[maaService] isTauri():', isTauri());
    
    if (!isTauri()) {
      console.log('[maaService] Not in Tauri env, simulating delay...');
      // 模拟连接延迟
      await new Promise(resolve => setTimeout(resolve, 1000));
      return;
    }
    
    console.log('[maaService] Invoking maa_connect_controller...');
    try {
      const result = await invoke('maa_connect_controller', {
        instanceId,
        config,
        agentPath: agentPath || null,
      });
      console.log('[maaService] maa_connect_controller result:', result);
      return result as void;
    } catch (err) {
      console.error('[maaService] maa_connect_controller error:', err);
      throw err;
    }
  },

  /**
   * 获取连接状态
   * @param instanceId 实例 ID
   */
  async getConnectionStatus(instanceId: string): Promise<ConnectionStatus> {
    if (!isTauri()) return 'Disconnected';
    return await invoke<ConnectionStatus>('maa_get_connection_status', { instanceId });
  },

  /**
   * 加载资源
   * @param instanceId 实例 ID
   * @param paths 资源路径列表
   */
  async loadResource(instanceId: string, paths: string[]): Promise<void> {
    if (!isTauri()) {
      // 模拟加载延迟
      await new Promise(resolve => setTimeout(resolve, 500));
      return;
    }
    return await invoke('maa_load_resource', { instanceId, paths });
  },

  /**
   * 检查资源是否已加载
   * @param instanceId 实例 ID
   */
  async isResourceLoaded(instanceId: string): Promise<boolean> {
    if (!isTauri()) return false;
    return await invoke<boolean>('maa_is_resource_loaded', { instanceId });
  },

  /**
   * 运行任务
   * @param instanceId 实例 ID
   * @param entry 任务入口
   * @param pipelineOverride Pipeline 覆盖 JSON
   * @returns 任务 ID
   */
  async runTask(instanceId: string, entry: string, pipelineOverride: string = '{}'): Promise<number> {
    if (!isTauri()) {
      // 返回模拟任务 ID
      return Math.floor(Math.random() * 10000);
    }
    return await invoke<number>('maa_run_task', {
      instanceId,
      entry,
      pipelineOverride,
    });
  },

  /**
   * 等待任务完成
   * @param instanceId 实例 ID
   * @param taskId 任务 ID
   */
  async waitTask(instanceId: string, taskId: number): Promise<TaskStatus> {
    if (!isTauri()) {
      // 模拟任务完成
      await new Promise(resolve => setTimeout(resolve, 2000));
      return 'Succeeded';
    }
    return await invoke<TaskStatus>('maa_wait_task', { instanceId, taskId });
  },

  /**
   * 获取任务状态
   * @param instanceId 实例 ID
   * @param taskId 任务 ID
   */
  async getTaskStatus(instanceId: string, taskId: number): Promise<TaskStatus> {
    if (!isTauri()) return 'Pending';
    return await invoke<TaskStatus>('maa_get_task_status', { instanceId, taskId });
  },

  /**
   * 停止任务
   * @param instanceId 实例 ID
   */
  async stopTask(instanceId: string): Promise<void> {
    if (!isTauri()) return;
    return await invoke('maa_stop_task', { instanceId });
  },

  /**
   * 检查是否正在运行
   * @param instanceId 实例 ID
   */
  async isRunning(instanceId: string): Promise<boolean> {
    if (!isTauri()) return false;
    return await invoke<boolean>('maa_is_running', { instanceId });
  },

  /**
   * 发起截图请求
   * @param instanceId 实例 ID
   * @returns 截图请求 ID
   */
  async postScreencap(instanceId: string): Promise<number> {
    if (!isTauri()) return -1;
    return await invoke<number>('maa_post_screencap', { instanceId });
  },

  /**
   * 等待截图完成
   * @param instanceId 实例 ID
   * @param screencapId 截图请求 ID
   */
  async screencapWait(instanceId: string, screencapId: number): Promise<boolean> {
    if (!isTauri()) return false;
    return await invoke<boolean>('maa_screencap_wait', { instanceId, screencapId });
  },

  /**
   * 获取缓存的截图
   * @param instanceId 实例 ID
   * @returns base64 编码的图像 data URL
   */
  async getCachedImage(instanceId: string): Promise<string> {
    if (!isTauri()) return '';
    return await invoke<string>('maa_get_cached_image', { instanceId });
  },
};

export default maaService;
