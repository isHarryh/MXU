//! MaaFramework FFI bindings
//! 
//! 通过 libloading 动态加载 MaaFramework 库，实现运行时绑定
//! 不同平台的动态库文件：
//! - Windows: MaaFramework.dll, MaaToolkit.dll
//! - macOS: libMaaFramework.dylib, libMaaToolkit.dylib
//! - Linux: libMaaFramework.so, libMaaToolkit.so

// 这是纯 FFI 绑定模块：符号/常量/字段会随着上层功能逐步启用。
// 若严格开启 dead_code，会在开发期产生大量无意义 warning，反而干扰排查真正问题。
#![allow(dead_code)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::Path;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use libloading::Library;

// 类型定义 (对应 MaaDef.h)
pub type MaaBool = u8;
pub type MaaSize = u64;
pub type MaaId = i64;
pub type MaaStatus = i32;

pub const MAA_STATUS_INVALID: MaaStatus = 0;
pub const MAA_STATUS_PENDING: MaaStatus = 1000;
pub const MAA_STATUS_RUNNING: MaaStatus = 2000;
pub const MAA_STATUS_SUCCEEDED: MaaStatus = 3000;
pub const MAA_STATUS_FAILED: MaaStatus = 4000;

pub const MAA_INVALID_ID: MaaId = 0;

// ADB 截图方法
pub type MaaAdbScreencapMethod = u64;
pub const MAA_ADB_SCREENCAP_DEFAULT: MaaAdbScreencapMethod = 0xFFFFFFFFFFFFFFFF 
    & !(1 << 3)  // RawByNetcat
    & !(1 << 4)  // MinicapDirect
    & !(1 << 5); // MinicapStream

// ADB 输入方法
pub type MaaAdbInputMethod = u64;
pub const MAA_ADB_INPUT_DEFAULT: MaaAdbInputMethod = 0xFFFFFFFFFFFFFFFF & !(1 << 3); // EmulatorExtras

// Win32 截图方法
pub type MaaWin32ScreencapMethod = u64;
pub const MAA_WIN32_SCREENCAP_GDI: MaaWin32ScreencapMethod = 1;
pub const MAA_WIN32_SCREENCAP_FRAMEPOOL: MaaWin32ScreencapMethod = 1 << 1;
pub const MAA_WIN32_SCREENCAP_DXGI_DESKTOPDUP: MaaWin32ScreencapMethod = 1 << 2;

// Win32 输入方法
pub type MaaWin32InputMethod = u64;
pub const MAA_WIN32_INPUT_SEIZE: MaaWin32InputMethod = 1;
pub const MAA_WIN32_INPUT_SENDMESSAGE: MaaWin32InputMethod = 1 << 1;

// Gamepad 类型
pub type MaaGamepadType = i32;
pub const MAA_GAMEPAD_TYPE_XBOX360: MaaGamepadType = 1;
pub const MAA_GAMEPAD_TYPE_DUALSHOCK4: MaaGamepadType = 2;

// 选项
pub type MaaCtrlOption = i32;
pub const MAA_CTRL_OPTION_SCREENSHOT_TARGET_SHORT_SIDE: MaaCtrlOption = 2;

// 不透明句柄类型
pub enum MaaResource {}
pub enum MaaController {}
pub enum MaaTasker {}
pub enum MaaStringBuffer {}
pub enum MaaImageBuffer {}
pub enum MaaToolkitAdbDeviceList {}
pub enum MaaToolkitAdbDevice {}
pub enum MaaToolkitDesktopWindowList {}
pub enum MaaToolkitDesktopWindow {}

// 回调类型
pub type MaaEventCallback = Option<extern "C" fn(
    handle: *mut c_void,
    message: *const c_char,
    details_json: *const c_char,
    trans_arg: *mut c_void,
)>;

// 函数指针类型定义
type FnMaaVersion = unsafe extern "C" fn() -> *const c_char;
type FnMaaStringBufferCreate = unsafe extern "C" fn() -> *mut MaaStringBuffer;
type FnMaaStringBufferDestroy = unsafe extern "C" fn(*mut MaaStringBuffer);
type FnMaaStringBufferGet = unsafe extern "C" fn(*const MaaStringBuffer) -> *const c_char;

type FnMaaResourceCreate = unsafe extern "C" fn() -> *mut MaaResource;
type FnMaaResourceDestroy = unsafe extern "C" fn(*mut MaaResource);
type FnMaaResourcePostBundle = unsafe extern "C" fn(*mut MaaResource, *const c_char) -> MaaId;
type FnMaaResourceStatus = unsafe extern "C" fn(*mut MaaResource, MaaId) -> MaaStatus;
type FnMaaResourceWait = unsafe extern "C" fn(*mut MaaResource, MaaId) -> MaaStatus;
type FnMaaResourceLoaded = unsafe extern "C" fn(*mut MaaResource) -> MaaBool;
type FnMaaResourceAddSink = unsafe extern "C" fn(*mut MaaResource, MaaEventCallback, *mut c_void) -> MaaId;

type FnMaaAdbControllerCreate = unsafe extern "C" fn(*const c_char, *const c_char, MaaAdbScreencapMethod, MaaAdbInputMethod, *const c_char, *const c_char) -> *mut MaaController;
type FnMaaWin32ControllerCreate = unsafe extern "C" fn(*mut c_void, MaaWin32ScreencapMethod, MaaWin32InputMethod, MaaWin32InputMethod) -> *mut MaaController;
type FnMaaGamepadControllerCreate = unsafe extern "C" fn(*mut c_void, MaaGamepadType, MaaWin32ScreencapMethod) -> *mut MaaController;
type FnMaaControllerDestroy = unsafe extern "C" fn(*mut MaaController);
type FnMaaControllerPostConnection = unsafe extern "C" fn(*mut MaaController) -> MaaId;
type FnMaaControllerStatus = unsafe extern "C" fn(*mut MaaController, MaaId) -> MaaStatus;
type FnMaaControllerWait = unsafe extern "C" fn(*mut MaaController, MaaId) -> MaaStatus;
type FnMaaControllerConnected = unsafe extern "C" fn(*mut MaaController) -> MaaBool;
type FnMaaControllerSetOption = unsafe extern "C" fn(*mut MaaController, MaaCtrlOption, *const c_void, MaaSize) -> MaaBool;
type FnMaaControllerPostScreencap = unsafe extern "C" fn(*mut MaaController) -> MaaId;
type FnMaaControllerCachedImage = unsafe extern "C" fn(*mut MaaController, *mut MaaImageBuffer) -> MaaBool;
type FnMaaControllerAddSink = unsafe extern "C" fn(*mut MaaController, MaaEventCallback, *mut c_void) -> MaaId;

// ImageBuffer
type FnMaaImageBufferCreate = unsafe extern "C" fn() -> *mut MaaImageBuffer;
type FnMaaImageBufferDestroy = unsafe extern "C" fn(*mut MaaImageBuffer);
type FnMaaImageBufferGetEncoded = unsafe extern "C" fn(*const MaaImageBuffer) -> *const u8;
type FnMaaImageBufferGetEncodedSize = unsafe extern "C" fn(*const MaaImageBuffer) -> MaaSize;

type FnMaaTaskerCreate = unsafe extern "C" fn() -> *mut MaaTasker;
type FnMaaTaskerDestroy = unsafe extern "C" fn(*mut MaaTasker);
type FnMaaTaskerBindResource = unsafe extern "C" fn(*mut MaaTasker, *mut MaaResource) -> MaaBool;
type FnMaaTaskerBindController = unsafe extern "C" fn(*mut MaaTasker, *mut MaaController) -> MaaBool;
type FnMaaTaskerInited = unsafe extern "C" fn(*mut MaaTasker) -> MaaBool;
type FnMaaTaskerPostTask = unsafe extern "C" fn(*mut MaaTasker, *const c_char, *const c_char) -> MaaId;
type FnMaaTaskerStatus = unsafe extern "C" fn(*mut MaaTasker, MaaId) -> MaaStatus;
type FnMaaTaskerWait = unsafe extern "C" fn(*mut MaaTasker, MaaId) -> MaaStatus;
type FnMaaTaskerRunning = unsafe extern "C" fn(*mut MaaTasker) -> MaaBool;
type FnMaaTaskerPostStop = unsafe extern "C" fn(*mut MaaTasker) -> MaaId;
type FnMaaTaskerAddSink = unsafe extern "C" fn(*mut MaaTasker, MaaEventCallback, *mut c_void) -> MaaId;

type FnMaaToolkitAdbDeviceListCreate = unsafe extern "C" fn() -> *mut MaaToolkitAdbDeviceList;
type FnMaaToolkitAdbDeviceListDestroy = unsafe extern "C" fn(*mut MaaToolkitAdbDeviceList);
type FnMaaToolkitAdbDeviceFind = unsafe extern "C" fn(*mut MaaToolkitAdbDeviceList) -> MaaBool;
type FnMaaToolkitAdbDeviceListSize = unsafe extern "C" fn(*const MaaToolkitAdbDeviceList) -> MaaSize;
type FnMaaToolkitAdbDeviceListAt = unsafe extern "C" fn(*const MaaToolkitAdbDeviceList, MaaSize) -> *const MaaToolkitAdbDevice;
type FnMaaToolkitAdbDeviceGetName = unsafe extern "C" fn(*const MaaToolkitAdbDevice) -> *const c_char;
type FnMaaToolkitAdbDeviceGetAdbPath = unsafe extern "C" fn(*const MaaToolkitAdbDevice) -> *const c_char;
type FnMaaToolkitAdbDeviceGetAddress = unsafe extern "C" fn(*const MaaToolkitAdbDevice) -> *const c_char;
type FnMaaToolkitAdbDeviceGetScreencapMethods = unsafe extern "C" fn(*const MaaToolkitAdbDevice) -> MaaAdbScreencapMethod;
type FnMaaToolkitAdbDeviceGetInputMethods = unsafe extern "C" fn(*const MaaToolkitAdbDevice) -> MaaAdbInputMethod;
type FnMaaToolkitAdbDeviceGetConfig = unsafe extern "C" fn(*const MaaToolkitAdbDevice) -> *const c_char;

type FnMaaToolkitDesktopWindowListCreate = unsafe extern "C" fn() -> *mut MaaToolkitDesktopWindowList;
type FnMaaToolkitDesktopWindowListDestroy = unsafe extern "C" fn(*mut MaaToolkitDesktopWindowList);
type FnMaaToolkitDesktopWindowFindAll = unsafe extern "C" fn(*mut MaaToolkitDesktopWindowList) -> MaaBool;
type FnMaaToolkitDesktopWindowListSize = unsafe extern "C" fn(*const MaaToolkitDesktopWindowList) -> MaaSize;
type FnMaaToolkitDesktopWindowListAt = unsafe extern "C" fn(*const MaaToolkitDesktopWindowList, MaaSize) -> *const MaaToolkitDesktopWindow;
type FnMaaToolkitDesktopWindowGetHandle = unsafe extern "C" fn(*const MaaToolkitDesktopWindow) -> *mut c_void;
type FnMaaToolkitDesktopWindowGetClassName = unsafe extern "C" fn(*const MaaToolkitDesktopWindow) -> *const c_char;
type FnMaaToolkitDesktopWindowGetWindowName = unsafe extern "C" fn(*const MaaToolkitDesktopWindow) -> *const c_char;

/// MaaFramework 库包装器
pub struct MaaLibrary {
    _framework_lib: Library,
    _toolkit_lib: Library,
    
    // MaaFramework 函数
    pub maa_version: FnMaaVersion,
    
    // StringBuffer
    pub maa_string_buffer_create: FnMaaStringBufferCreate,
    pub maa_string_buffer_destroy: FnMaaStringBufferDestroy,
    pub maa_string_buffer_get: FnMaaStringBufferGet,
    
    // Resource
    pub maa_resource_create: FnMaaResourceCreate,
    pub maa_resource_destroy: FnMaaResourceDestroy,
    pub maa_resource_post_bundle: FnMaaResourcePostBundle,
    pub maa_resource_status: FnMaaResourceStatus,
    pub maa_resource_wait: FnMaaResourceWait,
    pub maa_resource_loaded: FnMaaResourceLoaded,
    pub maa_resource_add_sink: FnMaaResourceAddSink,
    
    // Controller
    pub maa_adb_controller_create: FnMaaAdbControllerCreate,
    pub maa_win32_controller_create: FnMaaWin32ControllerCreate,
    pub maa_gamepad_controller_create: FnMaaGamepadControllerCreate,
    pub maa_controller_destroy: FnMaaControllerDestroy,
    pub maa_controller_post_connection: FnMaaControllerPostConnection,
    pub maa_controller_status: FnMaaControllerStatus,
    pub maa_controller_wait: FnMaaControllerWait,
    pub maa_controller_connected: FnMaaControllerConnected,
    pub maa_controller_set_option: FnMaaControllerSetOption,
    pub maa_controller_post_screencap: FnMaaControllerPostScreencap,
    pub maa_controller_cached_image: FnMaaControllerCachedImage,
    pub maa_controller_add_sink: FnMaaControllerAddSink,
    
    // ImageBuffer
    pub maa_image_buffer_create: FnMaaImageBufferCreate,
    pub maa_image_buffer_destroy: FnMaaImageBufferDestroy,
    pub maa_image_buffer_get_encoded: FnMaaImageBufferGetEncoded,
    pub maa_image_buffer_get_encoded_size: FnMaaImageBufferGetEncodedSize,
    
    // Tasker
    pub maa_tasker_create: FnMaaTaskerCreate,
    pub maa_tasker_destroy: FnMaaTaskerDestroy,
    pub maa_tasker_bind_resource: FnMaaTaskerBindResource,
    pub maa_tasker_bind_controller: FnMaaTaskerBindController,
    pub maa_tasker_inited: FnMaaTaskerInited,
    pub maa_tasker_post_task: FnMaaTaskerPostTask,
    pub maa_tasker_status: FnMaaTaskerStatus,
    pub maa_tasker_wait: FnMaaTaskerWait,
    pub maa_tasker_running: FnMaaTaskerRunning,
    pub maa_tasker_post_stop: FnMaaTaskerPostStop,
    pub maa_tasker_add_sink: FnMaaTaskerAddSink,
    
    // Toolkit - ADB Device
    pub maa_toolkit_adb_device_list_create: FnMaaToolkitAdbDeviceListCreate,
    pub maa_toolkit_adb_device_list_destroy: FnMaaToolkitAdbDeviceListDestroy,
    pub maa_toolkit_adb_device_find: FnMaaToolkitAdbDeviceFind,
    pub maa_toolkit_adb_device_list_size: FnMaaToolkitAdbDeviceListSize,
    pub maa_toolkit_adb_device_list_at: FnMaaToolkitAdbDeviceListAt,
    pub maa_toolkit_adb_device_get_name: FnMaaToolkitAdbDeviceGetName,
    pub maa_toolkit_adb_device_get_adb_path: FnMaaToolkitAdbDeviceGetAdbPath,
    pub maa_toolkit_adb_device_get_address: FnMaaToolkitAdbDeviceGetAddress,
    pub maa_toolkit_adb_device_get_screencap_methods: FnMaaToolkitAdbDeviceGetScreencapMethods,
    pub maa_toolkit_adb_device_get_input_methods: FnMaaToolkitAdbDeviceGetInputMethods,
    pub maa_toolkit_adb_device_get_config: FnMaaToolkitAdbDeviceGetConfig,
    
    // Toolkit - Desktop Window
    pub maa_toolkit_desktop_window_list_create: FnMaaToolkitDesktopWindowListCreate,
    pub maa_toolkit_desktop_window_list_destroy: FnMaaToolkitDesktopWindowListDestroy,
    pub maa_toolkit_desktop_window_find_all: FnMaaToolkitDesktopWindowFindAll,
    pub maa_toolkit_desktop_window_list_size: FnMaaToolkitDesktopWindowListSize,
    pub maa_toolkit_desktop_window_list_at: FnMaaToolkitDesktopWindowListAt,
    pub maa_toolkit_desktop_window_get_handle: FnMaaToolkitDesktopWindowGetHandle,
    pub maa_toolkit_desktop_window_get_class_name: FnMaaToolkitDesktopWindowGetClassName,
    pub maa_toolkit_desktop_window_get_window_name: FnMaaToolkitDesktopWindowGetWindowName,
}

// 注意：函数指针是 Send 和 Sync 的
unsafe impl Send for MaaLibrary {}
unsafe impl Sync for MaaLibrary {}

impl MaaLibrary {
    pub fn load(lib_dir: &Path) -> Result<Self, String> {
        unsafe {
            // 加载库
            #[cfg(windows)]
            let framework_path = lib_dir.join("MaaFramework.dll");
            #[cfg(target_os = "macos")]
            let framework_path = lib_dir.join("libMaaFramework.dylib");
            #[cfg(target_os = "linux")]
            let framework_path = lib_dir.join("libMaaFramework.so");
            
            #[cfg(windows)]
            let toolkit_path = lib_dir.join("MaaToolkit.dll");
            #[cfg(target_os = "macos")]
            let toolkit_path = lib_dir.join("libMaaToolkit.dylib");
            #[cfg(target_os = "linux")]
            let toolkit_path = lib_dir.join("libMaaToolkit.so");
            
            let framework_lib = Library::new(&framework_path)
                .map_err(|e| format!("Failed to load MaaFramework: {} (path: {:?})", e, framework_path))?;
            let toolkit_lib = Library::new(&toolkit_path)
                .map_err(|e| format!("Failed to load MaaToolkit: {} (path: {:?})", e, toolkit_path))?;
            
            // 加载函数宏 - 使用 transmute 进行类型转换
            macro_rules! load_fn {
                ($lib:expr, $name:literal) => {{
                    let sym = $lib.get::<*const ()>($name.as_bytes())
                        .map_err(|e| format!("Failed to load {}: {}", $name, e))?;
                    std::mem::transmute(*sym)
                }};
            }
            
            Ok(Self {
                // Version
                maa_version: load_fn!(framework_lib, "MaaVersion"),
                
                // StringBuffer
                maa_string_buffer_create: load_fn!(framework_lib, "MaaStringBufferCreate"),
                maa_string_buffer_destroy: load_fn!(framework_lib, "MaaStringBufferDestroy"),
                maa_string_buffer_get: load_fn!(framework_lib, "MaaStringBufferGet"),
                
                // Resource
                maa_resource_create: load_fn!(framework_lib, "MaaResourceCreate"),
                maa_resource_destroy: load_fn!(framework_lib, "MaaResourceDestroy"),
                maa_resource_post_bundle: load_fn!(framework_lib, "MaaResourcePostBundle"),
                maa_resource_status: load_fn!(framework_lib, "MaaResourceStatus"),
                maa_resource_wait: load_fn!(framework_lib, "MaaResourceWait"),
                maa_resource_loaded: load_fn!(framework_lib, "MaaResourceLoaded"),
                maa_resource_add_sink: load_fn!(framework_lib, "MaaResourceAddSink"),
                
                // Controller
                maa_adb_controller_create: load_fn!(framework_lib, "MaaAdbControllerCreate"),
                maa_win32_controller_create: load_fn!(framework_lib, "MaaWin32ControllerCreate"),
                maa_gamepad_controller_create: load_fn!(framework_lib, "MaaGamepadControllerCreate"),
                maa_controller_destroy: load_fn!(framework_lib, "MaaControllerDestroy"),
                maa_controller_post_connection: load_fn!(framework_lib, "MaaControllerPostConnection"),
                maa_controller_status: load_fn!(framework_lib, "MaaControllerStatus"),
                maa_controller_wait: load_fn!(framework_lib, "MaaControllerWait"),
                maa_controller_connected: load_fn!(framework_lib, "MaaControllerConnected"),
                maa_controller_set_option: load_fn!(framework_lib, "MaaControllerSetOption"),
                maa_controller_post_screencap: load_fn!(framework_lib, "MaaControllerPostScreencap"),
                maa_controller_cached_image: load_fn!(framework_lib, "MaaControllerCachedImage"),
                maa_controller_add_sink: load_fn!(framework_lib, "MaaControllerAddSink"),
                
                // ImageBuffer
                maa_image_buffer_create: load_fn!(framework_lib, "MaaImageBufferCreate"),
                maa_image_buffer_destroy: load_fn!(framework_lib, "MaaImageBufferDestroy"),
                maa_image_buffer_get_encoded: load_fn!(framework_lib, "MaaImageBufferGetEncoded"),
                maa_image_buffer_get_encoded_size: load_fn!(framework_lib, "MaaImageBufferGetEncodedSize"),
                
                // Tasker
                maa_tasker_create: load_fn!(framework_lib, "MaaTaskerCreate"),
                maa_tasker_destroy: load_fn!(framework_lib, "MaaTaskerDestroy"),
                maa_tasker_bind_resource: load_fn!(framework_lib, "MaaTaskerBindResource"),
                maa_tasker_bind_controller: load_fn!(framework_lib, "MaaTaskerBindController"),
                maa_tasker_inited: load_fn!(framework_lib, "MaaTaskerInited"),
                maa_tasker_post_task: load_fn!(framework_lib, "MaaTaskerPostTask"),
                maa_tasker_status: load_fn!(framework_lib, "MaaTaskerStatus"),
                maa_tasker_wait: load_fn!(framework_lib, "MaaTaskerWait"),
                maa_tasker_running: load_fn!(framework_lib, "MaaTaskerRunning"),
                maa_tasker_post_stop: load_fn!(framework_lib, "MaaTaskerPostStop"),
                maa_tasker_add_sink: load_fn!(framework_lib, "MaaTaskerAddSink"),
                
                // Toolkit - ADB Device
                maa_toolkit_adb_device_list_create: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceListCreate"),
                maa_toolkit_adb_device_list_destroy: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceListDestroy"),
                maa_toolkit_adb_device_find: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceFind"),
                maa_toolkit_adb_device_list_size: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceListSize"),
                maa_toolkit_adb_device_list_at: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceListAt"),
                maa_toolkit_adb_device_get_name: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceGetName"),
                maa_toolkit_adb_device_get_adb_path: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceGetAdbPath"),
                maa_toolkit_adb_device_get_address: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceGetAddress"),
                maa_toolkit_adb_device_get_screencap_methods: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceGetScreencapMethods"),
                maa_toolkit_adb_device_get_input_methods: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceGetInputMethods"),
                maa_toolkit_adb_device_get_config: load_fn!(toolkit_lib, "MaaToolkitAdbDeviceGetConfig"),
                
                // Toolkit - Desktop Window
                maa_toolkit_desktop_window_list_create: load_fn!(toolkit_lib, "MaaToolkitDesktopWindowListCreate"),
                maa_toolkit_desktop_window_list_destroy: load_fn!(toolkit_lib, "MaaToolkitDesktopWindowListDestroy"),
                maa_toolkit_desktop_window_find_all: load_fn!(toolkit_lib, "MaaToolkitDesktopWindowFindAll"),
                maa_toolkit_desktop_window_list_size: load_fn!(toolkit_lib, "MaaToolkitDesktopWindowListSize"),
                maa_toolkit_desktop_window_list_at: load_fn!(toolkit_lib, "MaaToolkitDesktopWindowListAt"),
                maa_toolkit_desktop_window_get_handle: load_fn!(toolkit_lib, "MaaToolkitDesktopWindowGetHandle"),
                maa_toolkit_desktop_window_get_class_name: load_fn!(toolkit_lib, "MaaToolkitDesktopWindowGetClassName"),
                maa_toolkit_desktop_window_get_window_name: load_fn!(toolkit_lib, "MaaToolkitDesktopWindowGetWindowName"),
                
                _framework_lib: framework_lib,
                _toolkit_lib: toolkit_lib,
            })
        }
    }
    
    pub fn version(&self) -> String {
        unsafe {
            let ptr = (self.maa_version)();
            if ptr.is_null() {
                return String::new();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

/// 全局 MaaLibrary 实例
pub static MAA_LIBRARY: Lazy<Mutex<Option<MaaLibrary>>> = Lazy::new(|| Mutex::new(None));

/// 初始化 MaaFramework 库
pub fn init_maa_library(lib_dir: &Path) -> Result<(), String> {
    let lib = MaaLibrary::load(lib_dir)?;
    let mut guard = MAA_LIBRARY.lock().map_err(|e| e.to_string())?;
    *guard = Some(lib);
    Ok(())
}

/// 获取 MaaFramework 版本
pub fn get_maa_version() -> Option<String> {
    let guard = MAA_LIBRARY.lock().ok()?;
    guard.as_ref().map(|lib| lib.version())
}

/// 辅助函数：将 &str 转换为 CString
pub fn to_cstring(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new("").unwrap())
}

/// 辅助函数：从 C 字符串指针读取字符串
pub unsafe fn from_cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}
