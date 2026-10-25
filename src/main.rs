#![windows_subsystem = "windows"]
#![allow(non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::mem::{size_of, transmute, zeroed};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering::SeqCst};

type HWND = *mut c_void;
type HANDLE = *mut c_void;
type HINSTANCE = *mut c_void;
type HMODULE = *mut c_void;
type HICON = *mut c_void;
type HCURSOR = *mut c_void;
type HBRUSH = *mut c_void;
type HGDIOBJ = *mut c_void;
type BOOL = i32;
type WPARAM = usize;
type LPARAM = isize;
type LRESULT = isize;
type WNDPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;
type NtSetSystemInformationFn = unsafe extern "system" fn(u32, *mut c_void, u32) -> i32;

// ---- Hằng số Win32 ----
const WM_COMMAND: u32 = 0x0111;
const WM_TIMER: u32 = 0x0113;
const WM_CLOSE: u32 = 0x0010;
const WM_DESTROY: u32 = 0x0002;
const WM_SETFONT: u32 = 0x0030;
const WM_APP: u32 = 0x8000;
const WM_CLEAN_DONE: u32 = WM_APP + 1;

const WS_OVERLAPPED_STYLE: u32 = 0x00CA_0000; // caption | sysmenu | minimizebox
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const CBS_DROPDOWNLIST: u32 = 0x0003;
const CBS_HASSTRINGS: u32 = 0x0200;
const BN_CLICKED: u32 = 0;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;
const SW_SHOWNORMAL: i32 = 1;
const CW_USEDEFAULT: i32 = i32::MIN; // 0x80000000
const MB_ICONERROR: u32 = 0x0000_0010;
const COLOR_BTNFACE: usize = 15;
const IDC_ARROW: usize = 32512;
const IDI_APP: usize = 1; // ID icon trong res/app.rc
const DEFAULT_GUI_FONT: i32 = 17;

const PROCESS_SET_QUOTA: u32 = 0x0100;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
const TOKEN_QUERY: u32 = 0x0008;
const SE_PRIVILEGE_ENABLED: u32 = 0x0000_0002;
const ERROR_NOT_ALL_ASSIGNED: u32 = 1300;

const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 80;
const MEMORY_PURGE_STANDBY_LIST: u32 = 4;

// ---- ID điều khiển và timer ----
const ID_LABEL_RAM: i32 = 101;
const ID_LABEL_USAGE: i32 = 102;
const ID_LABEL_CYCLE: i32 = 105;
const ID_COMBO: i32 = 103;
const ID_BUTTON: i32 = 104;
const TIMER_UI: usize = 1;
const TIMER_CLEAN: usize = 2;

const INTERVALS_MS: [u32; 9] = [
    5_000, 10_000, 15_000, 30_000, 60_000, 300_000, 600_000, 900_000, 1_800_000,
];
const INTERVAL_LABELS: [&str; 9] = [
    "5 giây", "10 giây", "15 giây", "30 giây", "60 giây",
    "5 phút", "10 phút", "15 phút", "30 phút",
];

// ---- Trạng thái dùng chung (atomic, không cần khóa) ----
static RUNNING: AtomicBool = AtomicBool::new(false);  // người dùng đang bật Clean
static CLEANING: AtomicBool = AtomicBool::new(false); // worker đang dọn
static INTERVAL: AtomicU32 = AtomicU32::new(5_000);
static WORKER: AtomicIsize = AtomicIsize::new(0);     // handle của worker hiện tại

// ---- Cấu trúc Win32 ----
#[repr(C)]
struct MEMORYSTATUSEX {
    dwLength: u32,
    dwMemoryLoad: u32,
    ullTotalPhys: u64,
    ullAvailPhys: u64,
    ullTotalPageFile: u64,
    ullAvailPageFile: u64,
    ullTotalVirtual: u64,
    ullAvailVirtual: u64,
    ullAvailExtendedVirtual: u64,
}

#[repr(C)]
struct POINT { x: i32, y: i32 }

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: u32,
    wParam: WPARAM,
    lParam: LPARAM,
    time: u32,
    pt: POINT,
}

#[repr(C)]
struct WNDCLASSW {
    style: u32,
    lpfnWndProc: WNDPROC,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: HICON,
    hCursor: HCURSOR,
    hbrBackground: HBRUSH,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
}

#[repr(C)]
struct LUID { LowPart: u32, HighPart: i32 }

#[repr(C)]
struct LUID_AND_ATTRIBUTES { Luid: LUID, Attributes: u32 }

#[repr(C)]
struct TOKEN_PRIVILEGES { PrivilegeCount: u32, Privileges: [LUID_AND_ATTRIBUTES; 1] }

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> HMODULE;
    fn GetProcAddress(module: HMODULE, name: *const u8) -> *mut c_void;
    fn GlobalMemoryStatusEx(buf: *mut MEMORYSTATUSEX) -> BOOL;
    fn GetCurrentProcess() -> HANDLE;
    fn OpenProcess(access: u32, inherit: BOOL, pid: u32) -> HANDLE;
    fn CloseHandle(h: HANDLE) -> BOOL;
    fn GetLastError() -> u32;
    fn SetLastError(code: u32);
    fn CreateThread(
        attrs: *mut c_void,
        stack: usize,
        start: unsafe extern "system" fn(*mut c_void) -> u32,
        param: *mut c_void,
        flags: u32,
        tid: *mut u32,
    ) -> HANDLE;
    fn WaitForSingleObject(h: HANDLE, ms: u32) -> u32;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(proc: HANDLE, access: u32, token: *mut HANDLE) -> BOOL;
    fn LookupPrivilegeValueW(sys: *const u16, name: *const u16, luid: *mut LUID) -> BOOL;
    fn AdjustTokenPrivileges(
        token: HANDLE,
        disable_all: BOOL,
        new: *const TOKEN_PRIVILEGES,
        len: u32,
        prev: *mut c_void,
        ret_len: *mut u32,
    ) -> BOOL;
}

#[link(name = "psapi")]
extern "system" {
    fn EnumProcesses(pids: *mut u32, cb: u32, needed: *mut u32) -> BOOL;
    fn EmptyWorkingSet(h: HANDLE) -> BOOL;
}

#[link(name = "gdi32")]
extern "system" {
    fn GetStockObject(obj: i32) -> HGDIOBJ;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(wc: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: *mut c_void,
        inst: HINSTANCE,
        param: *mut c_void,
    ) -> HWND;
    fn DefWindowProcW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT;
    fn DestroyWindow(hwnd: HWND) -> BOOL;
    fn ShowWindow(hwnd: HWND, cmd: i32) -> BOOL;
    fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32) -> BOOL;
    fn TranslateMessage(msg: *const MSG) -> BOOL;
    fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    fn PostQuitMessage(code: i32);
    fn PostMessageW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> BOOL;
    fn SendMessageW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT;
    fn SetTimer(hwnd: HWND, id: usize, ms: u32, cb: *mut c_void) -> usize;
    fn KillTimer(hwnd: HWND, id: usize) -> BOOL;
    fn SetWindowTextW(hwnd: HWND, text: *const u16) -> BOOL;
    fn GetDlgItem(hwnd: HWND, id: i32) -> HWND;
    fn EnableWindow(hwnd: HWND, enable: BOOL) -> BOOL;
    fn MessageBoxW(hwnd: HWND, text: *const u16, caption: *const u16, typ: u32) -> i32;
    fn LoadCursorW(inst: HINSTANCE, name: *const u16) -> HCURSOR;
    fn LoadIconW(inst: HINSTANCE, name: *const u16) -> HICON;
}

// ---- Tiện ích chuỗi và giao diện ----
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn set_text(h: HWND, s: &str) {
    let w = wide(s);
    SetWindowTextW(h, w.as_ptr());
}

unsafe fn make_child(
    parent: HWND,
    inst: HINSTANCE,
    class: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
) -> HWND {
    let c = wide(class);
    let t = wide(text);
    let child = CreateWindowExW(
        0,
        c.as_ptr(),
        t.as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        h,
        parent,
        id as usize as *mut c_void,
        inst,
        null_mut(),
    );
    SendMessageW(child, WM_SETFONT, GetStockObject(DEFAULT_GUI_FONT) as usize, 1);
    child
}

unsafe fn create_controls(hwnd: HWND, inst: HINSTANCE) {
    make_child(hwnd, inst, "STATIC", "RAM: -- / --", 0, 10, 10, 300, 20, ID_LABEL_RAM);
    make_child(hwnd, inst, "STATIC", "RAM Usage: --", 0, 10, 34, 300, 20, ID_LABEL_USAGE);
    make_child(hwnd, inst, "STATIC", "Chu kỳ:", 0, 10, 72, 60, 20, ID_LABEL_CYCLE);
    let combo = make_child(
        hwnd, inst, "COMBOBOX", "",
        CBS_DROPDOWNLIST | CBS_HASSTRINGS,
        70, 68, 120, 200, ID_COMBO,
    );
    for label in INTERVAL_LABELS.iter() {
        let w = wide(label);
        SendMessageW(combo, CB_ADDSTRING, 0, w.as_ptr() as isize);
    }
    SendMessageW(combo, CB_SETCURSEL, 0, 0); // mặc định 5 giây
    make_child(hwnd, inst, "BUTTON", "Clean", 0, 200, 66, 100, 28, ID_BUTTON);
}

// ---- Đọc RAM (dùng GlobalMemoryStatusEx cho cả tổng lẫn khả dụng) ----
unsafe fn read_memory() -> Option<(u64, u64)> {
    let mut st: MEMORYSTATUSEX = zeroed();
    st.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    if GlobalMemoryStatusEx(&mut st) == 0 || st.ullTotalPhys == 0 {
        return None;
    }
    Some((st.ullTotalPhys, st.ullAvailPhys))
}

unsafe fn update_ram(hwnd: HWND) {
    let (line1, line2) = match read_memory() {
        Some((total, avail)) => {
            let used = total.saturating_sub(avail);
            const GB: f64 = 1_073_741_824.0; // 1024^3, cùng đơn vị với Task Manager
            let pct = (used as f64 / total as f64 * 100.0).round() as u32;
            (
                format!("RAM: {:.2} GB / {:.2} GB", used as f64 / GB, total as f64 / GB),
                format!("RAM Usage: {}%", pct),
            )
        }
        None => ("RAM: -- / --".to_string(), "RAM Usage: --".to_string()),
    };
    set_text(GetDlgItem(hwnd, ID_LABEL_RAM), &line1);
    set_text(GetDlgItem(hwnd, ID_LABEL_USAGE), &line2);
}

// ---- Dọn RAM ----
unsafe fn enable_privilege(name: &str) -> Result<(), u32> {
    let mut token: HANDLE = null_mut();
    if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
        return Err(GetLastError());
    }
    let mut luid = LUID { LowPart: 0, HighPart: 0 };
    let wname = wide(name);
    if LookupPrivilegeValueW(null(), wname.as_ptr(), &mut luid) == 0 {
        let e = GetLastError();
        CloseHandle(token);
        return Err(e);
    }
    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
    };
    SetLastError(0);
    let ok = AdjustTokenPrivileges(
        token,
        0,
        &tp,
        size_of::<TOKEN_PRIVILEGES>() as u32,
        null_mut(),
        null_mut(),
    );
    let err = GetLastError();
    CloseHandle(token);
    if ok == 0 {
        Err(err)
    } else if err == ERROR_NOT_ALL_ASSIGNED {
        Err(err)
    } else {
        Ok(())
    }
}

unsafe fn empty_working_sets() {
    let mut cap: usize = 1024;
    loop {
        let mut pids = vec![0u32; cap];
        let mut needed: u32 = 0;
        let bytes = (cap * size_of::<u32>()) as u32;
        if EnumProcesses(pids.as_mut_ptr(), bytes, &mut needed) == 0 {
            return;
        }
        let count = needed as usize / size_of::<u32>();
        if count < cap {
            for &pid in &pids[..count] {
                if pid == 0 {
                    continue;
                }
                // Tiến trình không mở được (quyền hệ thống) thì bỏ qua
                let h = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA, 0, pid);
                if !h.is_null() {
                    EmptyWorkingSet(h);
                    CloseHandle(h);
                }
            }
            return;
        }
        cap *= 2;
        if cap > 1 << 16 {
            return;
        }
    }
}

unsafe fn purge_standby_list() -> Result<(), u32> {
    let ntdll = GetModuleHandleW(wide("ntdll.dll").as_ptr());
    if ntdll.is_null() {
        return Err(GetLastError());
    }
    let p = GetProcAddress(ntdll, b"NtSetSystemInformation\0".as_ptr());
    if p.is_null() {
        return Err(GetLastError());
    }
    let f: NtSetSystemInformationFn = transmute(p);
    let mut cmd: u32 = MEMORY_PURGE_STANDBY_LIST;
    let status = f(
        SYSTEM_MEMORY_LIST_INFORMATION,
        &mut cmd as *mut u32 as *mut c_void,
        size_of::<u32>() as u32,
    );
    if status == 0 { Ok(()) } else { Err(status as u32) }
}

unsafe fn clean_once() -> Result<(), u32> {
    enable_privilege("SeProfileSingleProcessPrivilege")?; // bắt buộc để purge
    let _ = enable_privilege("SeDebugPrivilege");          // không có thì chỉ bỏ qua tiến trình khó mở
    empty_working_sets();
    purge_standby_list()
}

unsafe extern "system" fn worker(param: *mut c_void) -> u32 {
    let hwnd = param as HWND;
    let (failed, detail) = match clean_once() {
        Ok(()) => (0usize, 0u32),
        Err(e) => (1usize, e),
    };
    PostMessageW(hwnd, WM_CLEAN_DONE, failed, detail as isize);
    0
}

unsafe fn start_worker(hwnd: HWND) -> Result<(), u32> {
    CLEANING.store(true, SeqCst);
    let mut tid: u32 = 0;
    let h = CreateThread(null_mut(), 0, worker, hwnd as *mut c_void, 0, &mut tid);
    if h.is_null() {
        CLEANING.store(false, SeqCst);
        return Err(GetLastError());
    }
    let old = WORKER.swap(h as isize, SeqCst) as HANDLE;
    if !old.is_null() {
        CloseHandle(old);
    }
    Ok(())
}

// ---- Điều khiển Clean/Stop ----
unsafe fn set_idle(hwnd: HWND) {
    RUNNING.store(false, SeqCst);
    KillTimer(hwnd, TIMER_CLEAN);
    EnableWindow(GetDlgItem(hwnd, ID_COMBO), 1);
    set_text(GetDlgItem(hwnd, ID_BUTTON), "Clean");
}

unsafe fn show_error(hwnd: HWND, code: u32) {
    let msg = format!(
        "Không thể dọn RAM (mã lỗi {:#010X}).\nHãy kiểm tra chương trình đang chạy với quyền Administrator.",
        code
    );
    let text = wide(&msg);
    let caption = wide("Toonie Clean RAM");
    MessageBoxW(hwnd, text.as_ptr(), caption.as_ptr(), MB_ICONERROR);
}

unsafe fn on_button(hwnd: HWND) {
    if RUNNING.load(SeqCst) {
        set_idle(hwnd);
        return;
    }
    let combo = GetDlgItem(hwnd, ID_COMBO);
    let sel = SendMessageW(combo, CB_GETCURSEL, 0, 0);
    let idx = if sel >= 0 && (sel as usize) < INTERVALS_MS.len() { sel as usize } else { 0 };
    INTERVAL.store(INTERVALS_MS[idx], SeqCst);
    RUNNING.store(true, SeqCst);
    EnableWindow(combo, 0); // khóa đổi chu kỳ khi đang chạy
    set_text(GetDlgItem(hwnd, ID_BUTTON), "Stop");
    // Nếu worker lần trước còn chạy, timer sẽ được bật lại khi nó báo xong
    if !CLEANING.load(SeqCst) {
        SetTimer(hwnd, TIMER_CLEAN, INTERVAL.load(SeqCst), null_mut());
    }
}

unsafe fn on_timer_clean(hwnd: HWND) {
    KillTimer(hwnd, TIMER_CLEAN);
    if !RUNNING.load(SeqCst) || CLEANING.load(SeqCst) {
        return;
    }
    if let Err(e) = start_worker(hwnd) {
        set_idle(hwnd);
        show_error(hwnd, e);
    }
}

unsafe fn on_clean_done(hwnd: HWND, failed: usize, detail: u32) {
    CLEANING.store(false, SeqCst);
    if failed != 0 {
        set_idle(hwnd);
        show_error(hwnd, detail);
    } else if RUNNING.load(SeqCst) {
        // Chu kỳ mới chỉ bắt đầu đếm sau khi lần dọn vừa rồi đã xong
        SetTimer(hwnd, TIMER_CLEAN, INTERVAL.load(SeqCst), null_mut());
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            let code = ((wp >> 16) & 0xFFFF) as u32;
            if id == ID_BUTTON && code == BN_CLICKED {
                on_button(hwnd);
            }
            0
        }
        WM_TIMER => {
            if wp == TIMER_UI {
                update_ram(hwnd);
            } else if wp == TIMER_CLEAN {
                on_timer_clean(hwnd);
            }
            0
        }
        WM_CLEAN_DONE => {
            on_clean_done(hwnd, wp, lp as u32);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, TIMER_UI);
            KillTimer(hwnd, TIMER_CLEAN);
            RUNNING.store(false, SeqCst);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn main() {
    unsafe {
        let inst = GetModuleHandleW(null()) as HINSTANCE;
        let class = wide("CleanRamWindow");
        let title = wide("Toonie Clean RAM");

        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: wnd_proc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: inst,
            hIcon: LoadIconW(inst, IDI_APP as *const u16),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW as *const u16),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }

        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED_STYLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            330,
            165,
            null_mut(),
            null_mut(),
            inst,
            null_mut(),
        );
        if hwnd.is_null() {
            return;
        }

        create_controls(hwnd, inst);
        update_ram(hwnd);
        SetTimer(hwnd, TIMER_UI, 1000, null_mut());
        ShowWindow(hwnd, SW_SHOWNORMAL);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Khi thoát, chờ worker đang dọn (tối đa 10 giây) để không cắt ngang
        let w = WORKER.swap(0, SeqCst) as HANDLE;
        if !w.is_null() {
            WaitForSingleObject(w, 10_000);
            CloseHandle(w);
        }
    }
}
