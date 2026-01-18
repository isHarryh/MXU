//! Tauri 命令实现
//! 
//! 提供前端调用的 MaaFramework 功能接口

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::maa_ffi::{
    from_cstr, get_maa_version, init_maa_library, to_cstring, MaaAgentClient, MaaController,
    MaaImageBuffer, MaaLibrary, MaaResource, MaaTasker, MaaToolkitAdbDeviceList,
    MaaToolkitDesktopWindowList, MAA_CTRL_OPTION_SCREENSHOT_TARGET_SHORT_SIDE,
    MAA_GAMEPAD_TYPE_DUALSHOCK4, MAA_GAMEPAD_TYPE_XBOX360, MAA_INVALID_ID, MAA_LIBRARY,
    MAA_STATUS_PENDING, MAA_STATUS_RUNNING, MAA_STATUS_SUCCEEDED,
    MAA_WIN32_SCREENCAP_DXGI_DESKTOPDUP,
};

// ============================================================================
// 数据类型定义
// ============================================================================

/// ADB 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdbDevice {
    pub name: String,
    pub adb_path: String,
    pub address: String,
    #[serde(with = "u64_as_string")]
    pub screencap_methods: u64,
    #[serde(with = "u64_as_string")]
    pub input_methods: u64,
    pub config: String,
}

/// 将 u64 序列化/反序列化为字符串，避免 JavaScript 精度丢失
mod u64_as_string {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<u64>().map_err(serde::de::Error::custom)
    }
}

/// Win32 窗口信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Win32Window {
    pub handle: u64,
    pub class_name: String,
    pub window_name: String,
}

/// 控制器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControllerConfig {
    Adb {
        adb_path: String,
        address: String,
        screencap_methods: String,  // u64 作为字符串传递，避免 JS 精度丢失
        input_methods: String,       // u64 作为字符串传递
        config: String,
    },
    Win32 {
        handle: u64,
        screencap_method: u64,
        mouse_method: u64,
        keyboard_method: u64,
    },
    Gamepad {
        handle: u64,
        #[serde(default)]
        gamepad_type: Option<String>,
        #[serde(default)]
        screencap_method: Option<u64>,
    },
    PlayCover {
        address: String,
    },
}

/// 连接状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

/// 实例运行时状态
pub struct InstanceRuntime {
    pub resource: Option<*mut MaaResource>,
    pub controller: Option<*mut MaaController>,
    pub tasker: Option<*mut MaaTasker>,
    pub agent_client: Option<*mut MaaAgentClient>,
    pub agent_child: Option<Child>,
    pub connection_status: ConnectionStatus,
    pub resource_loaded: bool,
}

// 为原始指针实现 Send 和 Sync
// MaaFramework 的 API 是线程安全的
unsafe impl Send for InstanceRuntime {}
unsafe impl Sync for InstanceRuntime {}

impl Default for InstanceRuntime {
    fn default() -> Self {
        Self {
            resource: None,
            controller: None,
            tasker: None,
            agent_client: None,
            agent_child: None,
            connection_status: ConnectionStatus::Disconnected,
            resource_loaded: false,
        }
    }
}

impl Drop for InstanceRuntime {
    fn drop(&mut self) {
        if let Ok(guard) = MAA_LIBRARY.lock() {
            if let Some(lib) = guard.as_ref() {
                unsafe {
                    // 断开并销毁 agent
                    if let Some(agent) = self.agent_client.take() {
                        (lib.maa_agent_client_disconnect)(agent);
                        (lib.maa_agent_client_destroy)(agent);
                    }
                    // 终止 agent 子进程
                    if let Some(mut child) = self.agent_child.take() {
                        let _ = child.kill();
                    }
                    if let Some(tasker) = self.tasker.take() {
                        (lib.maa_tasker_destroy)(tasker);
                    }
                    if let Some(controller) = self.controller.take() {
                        (lib.maa_controller_destroy)(controller);
                    }
                    if let Some(resource) = self.resource.take() {
                        (lib.maa_resource_destroy)(resource);
                    }
                }
            }
        }
    }
}

/// MaaFramework 运行时状态
pub struct MaaState {
    pub lib_dir: Mutex<Option<PathBuf>>,
    pub resource_dir: Mutex<Option<PathBuf>>,
    pub instances: Mutex<HashMap<String, InstanceRuntime>>,
}

impl Default for MaaState {
    fn default() -> Self {
        Self {
            lib_dir: Mutex::new(None),
            resource_dir: Mutex::new(None),
            instances: Mutex::new(HashMap::new()),
        }
    }
}

// ============================================================================
// Tauri 命令
// ============================================================================

/// 获取可执行文件所在目录下的 maafw 子目录
fn get_maafw_dir() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;
    let exe_dir = exe_path.parent()
        .ok_or_else(|| "Failed to get executable directory".to_string())?;
    
    // macOS app bundle 需要特殊处理：exe 在 Contents/MacOS 下，maafw 应在 Contents/Resources 下
    #[cfg(target_os = "macos")]
    {
        if exe_dir.ends_with("Contents/MacOS") {
            let resources_dir = exe_dir.parent().unwrap().join("Resources").join("maafw");
            if resources_dir.exists() {
                return Ok(resources_dir);
            }
        }
    }
    
    Ok(exe_dir.join("maafw"))
}

/// 初始化 MaaFramework
/// 如果提供 lib_dir 则使用该路径，否则自动从 exe 目录/maafw 加载
#[tauri::command]
pub fn maa_init(state: State<MaaState>, lib_dir: Option<String>) -> Result<String, String> {
    println!("[MaaCommands] maa_init called, lib_dir: {:?}", lib_dir);
    
    let lib_path = match lib_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(&dir),
        _ => get_maafw_dir()?,
    };
    
    println!("[MaaCommands] maa_init using path: {:?}", lib_path);
    
    if !lib_path.exists() {
        let err = format!(
            "MaaFramework library directory not found: {}",
            lib_path.display()
        );
        println!("[MaaCommands] {}", err);
        return Err(err);
    }
    
    println!("[MaaCommands] maa_init loading library...");
    init_maa_library(&lib_path)?;
    
    let version = get_maa_version().unwrap_or_default();
    println!("[MaaCommands] maa_init success, version: {}", version);
    
    *state.lib_dir.lock().map_err(|e| e.to_string())? = Some(lib_path);
    
    Ok(version)
}

/// 设置资源目录
#[tauri::command]
pub fn maa_set_resource_dir(state: State<MaaState>, resource_dir: String) -> Result<(), String> {
    *state.resource_dir.lock().map_err(|e| e.to_string())? = Some(PathBuf::from(resource_dir));
    Ok(())
}

/// 获取 MaaFramework 版本
#[tauri::command]
pub fn maa_get_version() -> Result<String, String> {
    get_maa_version().ok_or_else(|| "MaaFramework not initialized".to_string())
}

/// 查找 ADB 设备
#[tauri::command]
pub fn maa_find_adb_devices() -> Result<Vec<AdbDevice>, String> {
    println!("[MaaCommands] maa_find_adb_devices called");
    
    let guard = MAA_LIBRARY.lock().map_err(|e| {
        println!("[MaaCommands] Failed to lock MAA_LIBRARY: {}", e);
        e.to_string()
    })?;
    
    let lib = guard.as_ref().ok_or_else(|| {
        println!("[MaaCommands] MaaFramework not initialized");
        "MaaFramework not initialized".to_string()
    })?;
    
    println!("[MaaCommands] MaaFramework library loaded");
    
    unsafe {
        println!("[MaaCommands] Creating ADB device list...");
        let list = (lib.maa_toolkit_adb_device_list_create)();
        if list.is_null() {
            println!("[MaaCommands] Failed to create device list (null pointer)");
            return Err("Failed to create device list".to_string());
        }
        println!("[MaaCommands] Device list created successfully");
        
        // 确保清理
        struct ListGuard<'a> {
            list: *mut MaaToolkitAdbDeviceList,
            lib: &'a MaaLibrary,
        }
        impl Drop for ListGuard<'_> {
            fn drop(&mut self) {
                println!("[MaaCommands] Destroying ADB device list...");
                unsafe { (self.lib.maa_toolkit_adb_device_list_destroy)(self.list); }
            }
        }
        let _guard = ListGuard { list, lib };
        
        println!("[MaaCommands] Calling MaaToolkitAdbDeviceFind...");
        let found = (lib.maa_toolkit_adb_device_find)(list);
        println!("[MaaCommands] MaaToolkitAdbDeviceFind returned: {}", found);
        
        // MaaToolkitAdbDeviceFind 只在 buffer 为 null 时返回 false
        // 即使没找到设备也会返回 true，所以不应该用返回值判断是否找到设备
        if found == 0 {
            println!("[MaaCommands] MaaToolkitAdbDeviceFind returned false (unexpected)");
            // 继续执行而不是直接返回，检查 list size
        }
        
        let size = (lib.maa_toolkit_adb_device_list_size)(list);
        println!("[MaaCommands] Found {} ADB device(s)", size);
        
        let mut devices = Vec::with_capacity(size as usize);
        
        for i in 0..size {
            let device = (lib.maa_toolkit_adb_device_list_at)(list, i);
            if device.is_null() {
                println!("[MaaCommands] Device at index {} is null, skipping", i);
                continue;
            }
            
            let name = from_cstr((lib.maa_toolkit_adb_device_get_name)(device));
            let adb_path = from_cstr((lib.maa_toolkit_adb_device_get_adb_path)(device));
            let address = from_cstr((lib.maa_toolkit_adb_device_get_address)(device));
            
            println!("[MaaCommands] Device {}: name='{}', adb_path='{}', address='{}'", i, name, adb_path, address);
            
            devices.push(AdbDevice {
                name,
                adb_path,
                address,
                screencap_methods: (lib.maa_toolkit_adb_device_get_screencap_methods)(device),
                input_methods: (lib.maa_toolkit_adb_device_get_input_methods)(device),
                config: from_cstr((lib.maa_toolkit_adb_device_get_config)(device)),
            });
        }
        
        println!("[MaaCommands] Returning {} device(s)", devices.len());
        Ok(devices)
    }
}

/// 查找 Win32 窗口
#[tauri::command]
pub fn maa_find_win32_windows(class_regex: Option<String>, window_regex: Option<String>) -> Result<Vec<Win32Window>, String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    unsafe {
        let list = (lib.maa_toolkit_desktop_window_list_create)();
        if list.is_null() {
            return Err("Failed to create window list".to_string());
        }
        
        struct ListGuard<'a> {
            list: *mut MaaToolkitDesktopWindowList,
            lib: &'a MaaLibrary,
        }
        impl Drop for ListGuard<'_> {
            fn drop(&mut self) {
                unsafe { (self.lib.maa_toolkit_desktop_window_list_destroy)(self.list); }
            }
        }
        let _guard = ListGuard { list, lib };
        
        let found = (lib.maa_toolkit_desktop_window_find_all)(list);
        if found == 0 {
            return Ok(Vec::new());
        }
        
        let size = (lib.maa_toolkit_desktop_window_list_size)(list);
        let mut windows = Vec::with_capacity(size as usize);
        
        // 编译正则表达式
        let class_re = class_regex.as_ref().and_then(|r| regex::Regex::new(r).ok());
        let window_re = window_regex.as_ref().and_then(|r| regex::Regex::new(r).ok());
        
        for i in 0..size {
            let window = (lib.maa_toolkit_desktop_window_list_at)(list, i);
            if window.is_null() {
                continue;
            }
            
            let class_name = from_cstr((lib.maa_toolkit_desktop_window_get_class_name)(window));
            let window_name = from_cstr((lib.maa_toolkit_desktop_window_get_window_name)(window));
            
            // 过滤
            if let Some(re) = &class_re {
                if !re.is_match(&class_name) {
                    continue;
                }
            }
            if let Some(re) = &window_re {
                if !re.is_match(&window_name) {
                    continue;
                }
            }
            
            let handle = (lib.maa_toolkit_desktop_window_get_handle)(window);
            
            windows.push(Win32Window {
                handle: handle as u64,
                class_name,
                window_name,
            });
        }
        
        Ok(windows)
    }
}

/// 创建实例
#[tauri::command]
pub fn maa_create_instance(state: State<MaaState>, instance_id: String) -> Result<(), String> {
    let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
    
    if instances.contains_key(&instance_id) {
        return Err("Instance already exists".to_string());
    }
    
    instances.insert(instance_id, InstanceRuntime::default());
    Ok(())
}

/// 销毁实例
#[tauri::command]
pub fn maa_destroy_instance(state: State<MaaState>, instance_id: String) -> Result<(), String> {
    let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
    instances.remove(&instance_id);
    Ok(())
}

/// 连接控制器
#[tauri::command]
pub async fn maa_connect_controller(
    state: State<'_, MaaState>,
    instance_id: String,
    config: ControllerConfig,
    agent_path: Option<String>,
) -> Result<(), String> {
    println!("[MaaCommands] maa_connect_controller called");
    println!("[MaaCommands] instance_id: {}", instance_id);
    println!("[MaaCommands] config: {:?}", config);
    println!("[MaaCommands] agent_path: {:?}", agent_path);
    
    let guard = MAA_LIBRARY.lock().map_err(|e| {
        println!("[MaaCommands] Failed to lock MAA_LIBRARY: {}", e);
        e.to_string()
    })?;
    let lib = guard.as_ref().ok_or_else(|| {
        println!("[MaaCommands] MaaFramework not initialized");
        "MaaFramework not initialized".to_string()
    })?;
    
    println!("[MaaCommands] MaaFramework library loaded, creating controller...");
    
    let controller = unsafe {
        match &config {
            ControllerConfig::Adb { adb_path, address, screencap_methods, input_methods, config } => {
                // 将字符串解析为 u64
                let screencap_methods_u64 = screencap_methods.parse::<u64>().map_err(|e| {
                    format!("Invalid screencap_methods '{}': {}", screencap_methods, e)
                })?;
                let input_methods_u64 = input_methods.parse::<u64>().map_err(|e| {
                    format!("Invalid input_methods '{}': {}", input_methods, e)
                })?;
                
                println!("[MaaCommands] Creating ADB controller:");
                println!("[MaaCommands]   adb_path: {}", adb_path);
                println!("[MaaCommands]   address: {}", address);
                println!("[MaaCommands]   screencap_methods: {} (parsed: {})", screencap_methods, screencap_methods_u64);
                println!("[MaaCommands]   input_methods: {} (parsed: {})", input_methods, input_methods_u64);
                println!("[MaaCommands]   config: {}", config);
                
                let adb_path_c = to_cstring(adb_path);
                let address_c = to_cstring(address);
                let config_c = to_cstring(config);
                let agent_path_c = to_cstring(agent_path.as_deref().unwrap_or(""));
                
                println!("[MaaCommands] Calling MaaAdbControllerCreate...");
                let ctrl = (lib.maa_adb_controller_create)(
                    adb_path_c.as_ptr(),
                    address_c.as_ptr(),
                    screencap_methods_u64,
                    input_methods_u64,
                    config_c.as_ptr(),
                    agent_path_c.as_ptr(),
                );
                println!("[MaaCommands] MaaAdbControllerCreate returned: {:?}", ctrl);
                ctrl
            }
            ControllerConfig::Win32 { handle, screencap_method, mouse_method, keyboard_method } => {
                (lib.maa_win32_controller_create)(
                    *handle as *mut std::ffi::c_void,
                    *screencap_method,
                    *mouse_method,
                    *keyboard_method,
                )
            }
            ControllerConfig::Gamepad { handle, gamepad_type, screencap_method } => {
                // 解析 gamepad_type，默认为 Xbox360
                let gp_type = match gamepad_type.as_deref() {
                    Some("DualShock4") | Some("DS4") => MAA_GAMEPAD_TYPE_DUALSHOCK4,
                    _ => MAA_GAMEPAD_TYPE_XBOX360,
                };
                // 截图方法，默认为 DXGI_DesktopDup
                let screencap = screencap_method.unwrap_or(MAA_WIN32_SCREENCAP_DXGI_DESKTOPDUP);
                
                (lib.maa_gamepad_controller_create)(
                    *handle as *mut std::ffi::c_void,
                    gp_type,
                    screencap,
                )
            }
            ControllerConfig::PlayCover { .. } => {
                // PlayCover 仅支持 macOS
                return Err("PlayCover controller is only supported on macOS".to_string());
            }
        }
    };
    
    if controller.is_null() {
        println!("[MaaCommands] Controller creation failed (null pointer)");
        return Err("Failed to create controller".to_string());
    }
    
    println!("[MaaCommands] Controller created successfully: {:?}", controller);
    
    // 设置默认截图分辨率
    println!("[MaaCommands] Setting screenshot target short side to 720...");
    unsafe {
        let short_side: i32 = 720;
        (lib.maa_controller_set_option)(
            controller,
            MAA_CTRL_OPTION_SCREENSHOT_TARGET_SHORT_SIDE,
            &short_side as *const i32 as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u64,
        );
    }
    
    // 发起连接
    println!("[MaaCommands] Calling MaaControllerPostConnection...");
    let conn_id = unsafe { (lib.maa_controller_post_connection)(controller) };
    println!("[MaaCommands] MaaControllerPostConnection returned conn_id: {}", conn_id);
    
    // 更新实例状态
    println!("[MaaCommands] Updating instance state...");
    {
        let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;
        
        // 清理旧的控制器
        if let Some(old_controller) = instance.controller.take() {
            println!("[MaaCommands] Destroying old controller...");
            unsafe { (lib.maa_controller_destroy)(old_controller); }
        }
        
        instance.controller = Some(controller);
        instance.connection_status = ConnectionStatus::Connecting;
    }
    
    // 释放锁后等待连接
    drop(guard);
    
    // 等待连接完成（在实际应用中应该使用异步轮询）
    println!("[MaaCommands] Waiting for connection to complete...");
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let status = unsafe { (lib.maa_controller_wait)(controller, conn_id) };
    println!("[MaaCommands] MaaControllerWait returned status: {}", status);
    
    let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
    let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;
    
    if status == MAA_STATUS_SUCCEEDED {
        println!("[MaaCommands] Connection succeeded!");
        instance.connection_status = ConnectionStatus::Connected;
        Ok(())
    } else {
        println!("[MaaCommands] Connection failed with status: {}", status);
        instance.connection_status = ConnectionStatus::Failed("Connection failed".to_string());
        Err("Controller connection failed".to_string())
    }
}

/// 获取连接状态
#[tauri::command]
pub fn maa_get_connection_status(state: State<MaaState>, instance_id: String) -> Result<ConnectionStatus, String> {
    let instances = state.instances.lock().map_err(|e| e.to_string())?;
    let instance = instances.get(&instance_id).ok_or("Instance not found")?;
    Ok(instance.connection_status.clone())
}

/// 加载资源
#[tauri::command]
pub async fn maa_load_resource(
    state: State<'_, MaaState>,
    instance_id: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    // 创建或获取资源
    let resource = {
        let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;
        
        if instance.resource.is_none() {
            let res = unsafe { (lib.maa_resource_create)() };
            if res.is_null() {
                return Err("Failed to create resource".to_string());
            }
            instance.resource = Some(res);
        }
        
        instance.resource.unwrap()
    };
    
    // 加载资源
    let mut last_id = MAA_INVALID_ID;
    for path in &paths {
        let path_c = to_cstring(path);
        last_id = unsafe { (lib.maa_resource_post_bundle)(resource, path_c.as_ptr()) };
    }
    
    // 释放锁后等待
    drop(guard);
    
    // 等待资源加载
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let status = unsafe { (lib.maa_resource_wait)(resource, last_id) };
    
    let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
    let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;
    
    if status == MAA_STATUS_SUCCEEDED {
        instance.resource_loaded = true;
        Ok(())
    } else {
        instance.resource_loaded = false;
        Err("Resource loading failed".to_string())
    }
}

/// 检查资源是否已加载
#[tauri::command]
pub fn maa_is_resource_loaded(state: State<MaaState>, instance_id: String) -> Result<bool, String> {
    let instances = state.instances.lock().map_err(|e| e.to_string())?;
    let instance = instances.get(&instance_id).ok_or("Instance not found")?;
    Ok(instance.resource_loaded)
}

/// 运行任务
#[tauri::command]
pub async fn maa_run_task(
    state: State<'_, MaaState>,
    instance_id: String,
    entry: String,
    pipeline_override: String,
) -> Result<i64, String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let (_resource, _controller, tasker) = {
        let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;
        
        let resource = instance.resource.ok_or("Resource not loaded")?;
        let controller = instance.controller.ok_or("Controller not connected")?;
        
        // 创建或获取 tasker
        if instance.tasker.is_none() {
            let tasker = unsafe { (lib.maa_tasker_create)() };
            if tasker.is_null() {
                return Err("Failed to create tasker".to_string());
            }
            
            // 绑定资源和控制器
            unsafe {
                (lib.maa_tasker_bind_resource)(tasker, resource);
                (lib.maa_tasker_bind_controller)(tasker, controller);
            }
            
            instance.tasker = Some(tasker);
        }
        
        (resource, controller, instance.tasker.unwrap())
    };
    
    // 检查初始化状态
    let inited = unsafe { (lib.maa_tasker_inited)(tasker) };
    if inited == 0 {
        return Err("Tasker not properly initialized".to_string());
    }
    
    // 提交任务
    let entry_c = to_cstring(&entry);
    let override_c = to_cstring(&pipeline_override);
    
    let task_id = unsafe {
        (lib.maa_tasker_post_task)(tasker, entry_c.as_ptr(), override_c.as_ptr())
    };
    
    if task_id == MAA_INVALID_ID {
        return Err("Failed to post task".to_string());
    }
    
    Ok(task_id)
}

/// 等待任务完成
#[tauri::command]
pub async fn maa_wait_task(
    state: State<'_, MaaState>,
    instance_id: String,
    task_id: i64,
) -> Result<TaskStatus, String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let tasker = {
        let instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get(&instance_id).ok_or("Instance not found")?;
        instance.tasker.ok_or("Tasker not created")?
    };
    
    let status = unsafe { (lib.maa_tasker_wait)(tasker, task_id) };
    
    Ok(match status {
        MAA_STATUS_PENDING => TaskStatus::Pending,
        MAA_STATUS_RUNNING => TaskStatus::Running,
        MAA_STATUS_SUCCEEDED => TaskStatus::Succeeded,
        _ => TaskStatus::Failed,
    })
}

/// 获取任务状态
#[tauri::command]
pub fn maa_get_task_status(
    state: State<MaaState>,
    instance_id: String,
    task_id: i64,
) -> Result<TaskStatus, String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let tasker = {
        let instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get(&instance_id).ok_or("Instance not found")?;
        instance.tasker.ok_or("Tasker not created")?
    };
    
    let status = unsafe { (lib.maa_tasker_status)(tasker, task_id) };
    
    Ok(match status {
        MAA_STATUS_PENDING => TaskStatus::Pending,
        MAA_STATUS_RUNNING => TaskStatus::Running,
        MAA_STATUS_SUCCEEDED => TaskStatus::Succeeded,
        _ => TaskStatus::Failed,
    })
}

/// 停止任务
#[tauri::command]
pub fn maa_stop_task(state: State<MaaState>, instance_id: String) -> Result<(), String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let tasker = {
        let instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get(&instance_id).ok_or("Instance not found")?;
        instance.tasker.ok_or("Tasker not created")?
    };
    
    unsafe { (lib.maa_tasker_post_stop)(tasker) };
    
    Ok(())
}

/// 检查是否正在运行
#[tauri::command]
pub fn maa_is_running(state: State<MaaState>, instance_id: String) -> Result<bool, String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let tasker = {
        let instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get(&instance_id).ok_or("Instance not found")?;
        match instance.tasker {
            Some(t) => t,
            None => return Ok(false),
        }
    };
    
    let running = unsafe { (lib.maa_tasker_running)(tasker) };
    Ok(running != 0)
}

/// 发起截图请求
#[tauri::command]
pub fn maa_post_screencap(state: State<MaaState>, instance_id: String) -> Result<i64, String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let controller = {
        let instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get(&instance_id).ok_or("Instance not found")?;
        instance.controller.ok_or("Controller not connected")?
    };
    
    let screencap_id = unsafe { (lib.maa_controller_post_screencap)(controller) };
    
    if screencap_id == MAA_INVALID_ID {
        return Err("Failed to post screencap".to_string());
    }
    
    Ok(screencap_id)
}

/// 等待截图完成
#[tauri::command]
pub async fn maa_screencap_wait(
    state: State<'_, MaaState>,
    instance_id: String,
    screencap_id: i64,
) -> Result<bool, String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let controller = {
        let instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get(&instance_id).ok_or("Instance not found")?;
        instance.controller.ok_or("Controller not connected")?
    };
    
    let status = unsafe { (lib.maa_controller_wait)(controller, screencap_id) };
    
    Ok(status == MAA_STATUS_SUCCEEDED)
}

/// 获取缓存的截图（返回 base64 编码的 PNG 图像）
#[tauri::command]
pub fn maa_get_cached_image(state: State<MaaState>, instance_id: String) -> Result<String, String> {
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let controller = {
        let instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get(&instance_id).ok_or("Instance not found")?;
        instance.controller.ok_or("Controller not connected")?
    };
    
    unsafe {
        // 创建图像缓冲区
        let image_buffer = (lib.maa_image_buffer_create)();
        if image_buffer.is_null() {
            return Err("Failed to create image buffer".to_string());
        }
        
        // 确保缓冲区被释放
        struct ImageBufferGuard<'a> {
            buffer: *mut MaaImageBuffer,
            lib: &'a MaaLibrary,
        }
        impl Drop for ImageBufferGuard<'_> {
            fn drop(&mut self) {
                unsafe { (self.lib.maa_image_buffer_destroy)(self.buffer); }
            }
        }
        let _guard = ImageBufferGuard { buffer: image_buffer, lib };
        
        // 获取缓存的图像
        let success = (lib.maa_controller_cached_image)(controller, image_buffer);
        if success == 0 {
            return Err("Failed to get cached image".to_string());
        }
        
        // 获取编码后的图像数据
        let encoded_ptr = (lib.maa_image_buffer_get_encoded)(image_buffer);
        let encoded_size = (lib.maa_image_buffer_get_encoded_size)(image_buffer);
        
        if encoded_ptr.is_null() || encoded_size == 0 {
            return Err("No image data available".to_string());
        }
        
        // 复制数据并转换为 base64
        let data = std::slice::from_raw_parts(encoded_ptr, encoded_size as usize);
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let base64_str = STANDARD.encode(data);
        
        // 返回带 data URL 前缀的 base64 字符串
        Ok(format!("data:image/png;base64,{}", base64_str))
    }
}

/// Agent 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub child_exec: String,
    pub child_args: Option<Vec<String>>,
    pub identifier: Option<String>,
    /// 连接超时时间（毫秒），-1 表示无限等待
    pub timeout: Option<i64>,
}

/// 任务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub entry: String,
    pub pipeline_override: String,
}

/// 启动任务（支持 Agent）
#[tauri::command]
pub async fn maa_start_tasks(
    state: State<'_, MaaState>,
    instance_id: String,
    tasks: Vec<TaskConfig>,
    agent_config: Option<AgentConfig>,
    cwd: String,
) -> Result<Vec<i64>, String> {
    println!("[MaaCommands] maa_start_tasks called");
    println!("[MaaCommands] instance_id: {}, tasks: {}, cwd: {}", instance_id, tasks.len(), cwd);
    
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    // 获取实例资源和控制器
    let (resource, _controller, tasker) = {
        let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
        let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;
        
        let resource = instance.resource.ok_or("Resource not loaded")?;
        let controller = instance.controller.ok_or("Controller not connected")?;
        
        // 创建或获取 tasker
        if instance.tasker.is_none() {
            let tasker = unsafe { (lib.maa_tasker_create)() };
            if tasker.is_null() {
                return Err("Failed to create tasker".to_string());
            }
            
            // 绑定资源和控制器
            unsafe {
                (lib.maa_tasker_bind_resource)(tasker, resource);
                (lib.maa_tasker_bind_controller)(tasker, controller);
            }
            
            instance.tasker = Some(tasker);
        }
        
        (resource, controller, instance.tasker.unwrap())
    };
    
    // 启动 Agent（如果配置了）
    if let Some(agent) = &agent_config {
        println!("[MaaCommands] Starting agent: {:?}", agent);
        
        // 创建 AgentClient
        let agent_client = unsafe { (lib.maa_agent_client_create_v2)(std::ptr::null()) };
        if agent_client.is_null() {
            return Err("Failed to create agent client".to_string());
        }
        
        // 绑定资源
        unsafe {
            (lib.maa_agent_client_bind_resource)(agent_client, resource);
        }
        
        // 获取 socket identifier
        let socket_id = unsafe {
            let id_buffer = (lib.maa_string_buffer_create)();
            if id_buffer.is_null() {
                (lib.maa_agent_client_destroy)(agent_client);
                return Err("Failed to create string buffer".to_string());
            }
            
            let success = (lib.maa_agent_client_identifier)(agent_client, id_buffer);
            if success == 0 {
                (lib.maa_string_buffer_destroy)(id_buffer);
                (lib.maa_agent_client_destroy)(agent_client);
                return Err("Failed to get agent identifier".to_string());
            }
            
            let id = from_cstr((lib.maa_string_buffer_get)(id_buffer));
            (lib.maa_string_buffer_destroy)(id_buffer);
            id
        };
        
        println!("[MaaCommands] Agent socket_id: {}", socket_id);
        
        // 构建子进程参数
        let mut args = agent.child_args.clone().unwrap_or_default();
        args.push(socket_id);
        
        println!("[MaaCommands] Starting child process: {} {:?} in {}", agent.child_exec, args, cwd);
        
        // 将相对路径转换为绝对路径（Windows 的 Command 不能正确处理 Unix 风格相对路径）
        let exec_path = std::path::Path::new(&cwd).join(&agent.child_exec);
        let exec_path = exec_path.canonicalize().unwrap_or(exec_path);
        println!("[MaaCommands] Resolved executable path: {:?}, exists: {}", exec_path, exec_path.exists());
        
        // 启动子进程，捕获 stdout 和 stderr
        // 设置 PYTHONIOENCODING 强制 Python 以 UTF-8 编码输出，避免 Windows 系统代码页乱码
        println!("[MaaCommands] Spawning child process...");
        let spawn_result = Command::new(&exec_path)
            .args(&args)
            .current_dir(&cwd)
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUTF8", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        
        let mut child = match spawn_result {
            Ok(c) => {
                println!("[MaaCommands] Spawn succeeded!");
                c
            }
            Err(e) => {
                let err_msg = format!("Failed to start agent process: {} (exec: {:?}, cwd: {})", e, exec_path, cwd);
                println!("[MaaCommands] {}", err_msg);
                return Err(err_msg);
            }
        };
        
        println!("[MaaCommands] Agent child process started, pid: {:?}", child.id());
        
        // 在单独线程中读取 stdout（使用有损转换处理非UTF-8输出）
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut buffer = Vec::new();
                loop {
                    buffer.clear();
                    match reader.read_until(b'\n', &mut buffer) {
                        Ok(0) => break,  // EOF
                        Ok(_) => {
                            // 移除末尾换行符后使用有损转换
                            if buffer.ends_with(&[b'\n']) {
                                buffer.pop();
                            }
                            if buffer.ends_with(&[b'\r']) {
                                buffer.pop();
                            }
                            let line = String::from_utf8_lossy(&buffer);
                            println!("[Agent stdout] {}", line);
                        }
                        Err(e) => {
                            eprintln!("[Agent stdout error] {}", e);
                            break;
                        }
                    }
                }
            });
        }
        
        // 在单独线程中读取 stderr（使用有损转换处理非UTF-8输出）
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut buffer = Vec::new();
                loop {
                    buffer.clear();
                    match reader.read_until(b'\n', &mut buffer) {
                        Ok(0) => break,  // EOF
                        Ok(_) => {
                            if buffer.ends_with(&[b'\n']) {
                                buffer.pop();
                            }
                            if buffer.ends_with(&[b'\r']) {
                                buffer.pop();
                            }
                            let line = String::from_utf8_lossy(&buffer);
                            eprintln!("[Agent stderr] {}", line);
                        }
                        Err(e) => {
                            eprintln!("[Agent stderr error] {}", e);
                            break;
                        }
                    }
                }
            });
        }
        
        // 设置连接超时（-1 表示无限等待）
        let timeout_ms = agent.timeout.unwrap_or(-1);
        println!("[MaaCommands] Setting agent connect timeout: {} ms", timeout_ms);
        unsafe { (lib.maa_agent_client_set_timeout)(agent_client, timeout_ms); }
        
        // 等待连接
        let connected = unsafe { (lib.maa_agent_client_connect)(agent_client) };
        if connected == 0 {
            let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
            if let Some(instance) = instances.get_mut(&instance_id) {
                instance.agent_child = Some(child);
            }
            unsafe { (lib.maa_agent_client_destroy)(agent_client); }
            return Err("Failed to connect to agent".to_string());
        }
        
        println!("[MaaCommands] Agent connected");
        
        // 保存 agent 状态
        {
            let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
            if let Some(instance) = instances.get_mut(&instance_id) {
                instance.agent_client = Some(agent_client);
                instance.agent_child = Some(child);
            }
        }
    }
    
    // 检查初始化状态
    let inited = unsafe { (lib.maa_tasker_inited)(tasker) };
    if inited == 0 {
        return Err("Tasker not properly initialized".to_string());
    }
    
    // 提交所有任务
    let mut task_ids = Vec::new();
    for task in &tasks {
        let entry_c = to_cstring(&task.entry);
        let override_c = to_cstring(&task.pipeline_override);
        
        let task_id = unsafe {
            (lib.maa_tasker_post_task)(tasker, entry_c.as_ptr(), override_c.as_ptr())
        };
        
        if task_id == MAA_INVALID_ID {
            println!("[MaaCommands] Failed to post task: {}", task.entry);
            continue;
        }
        
        println!("[MaaCommands] Posted task: {} -> id: {}", task.entry, task_id);
        task_ids.push(task_id);
    }
    
    Ok(task_ids)
}

/// 停止 Agent 并断开连接
#[tauri::command]
pub fn maa_stop_agent(state: State<MaaState>, instance_id: String) -> Result<(), String> {
    println!("[MaaCommands] maa_stop_agent called for instance: {}", instance_id);
    
    let guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    let lib = guard.as_ref().ok_or("MaaFramework not initialized")?;
    
    let mut instances = state.instances.lock().map_err(|e| e.to_string())?;
    let instance = instances.get_mut(&instance_id).ok_or("Instance not found")?;
    
    // 断开并销毁 agent
    if let Some(agent) = instance.agent_client.take() {
        println!("[MaaCommands] Disconnecting agent...");
        unsafe {
            (lib.maa_agent_client_disconnect)(agent);
            (lib.maa_agent_client_destroy)(agent);
        }
    }
    
    // 终止子进程
    if let Some(mut child) = instance.agent_child.take() {
        println!("[MaaCommands] Killing agent child process...");
        let _ = child.kill();
        let _ = child.wait();
    }
    
    Ok(())
}

// ============================================================================
// 文件读取
// ============================================================================

/// 获取 exe 所在目录路径
fn get_exe_directory() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取 exe 路径失败: {}", e))?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法获取 exe 所在目录".to_string())
}

/// 读取 exe 同目录下的文本文件
#[tauri::command]
pub fn read_local_file(filename: String) -> Result<String, String> {
    let exe_dir = get_exe_directory()?;
    let file_path = exe_dir.join(&filename);
    println!("[MaaCommands] Reading local file: {:?}", file_path);
    
    std::fs::read_to_string(&file_path)
        .map_err(|e| format!("读取文件失败 [{}]: {}", file_path.display(), e))
}

/// 读取 exe 同目录下的二进制文件，返回 base64 编码
#[tauri::command]
pub fn read_local_file_base64(filename: String) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    
    let exe_dir = get_exe_directory()?;
    let file_path = exe_dir.join(&filename);
    println!("[MaaCommands] Reading local file (base64): {:?}", file_path);
    
    let data = std::fs::read(&file_path)
        .map_err(|e| format!("读取文件失败 [{}]: {}", file_path.display(), e))?;
    
    Ok(STANDARD.encode(&data))
}

/// 检查 exe 同目录下的文件是否存在
#[tauri::command]
pub fn local_file_exists(filename: String) -> Result<bool, String> {
    let exe_dir = get_exe_directory()?;
    let file_path = exe_dir.join(&filename);
    Ok(file_path.exists())
}

/// 获取 exe 所在目录路径
#[tauri::command]
pub fn get_exe_dir() -> Result<String, String> {
    let exe_dir = get_exe_directory()?;
    Ok(exe_dir.to_string_lossy().to_string())
}
