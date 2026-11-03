use std::cell::Cell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

type WndProc = unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize;

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Default)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(point: *mut Point) -> i32;
    fn ScreenToClient(window: *mut c_void, point: *mut Point) -> i32;
    fn GetClientRect(window: *mut c_void, rect: *mut WinRect) -> i32;
    fn GetForegroundWindow() -> *mut c_void;
    fn WindowFromDC(dc: *mut c_void) -> *mut c_void;
    fn GetWindowLongPtrW(window: *mut c_void, index: i32) -> isize;
    fn GetClassLongPtrW(window: *mut c_void, index: i32) -> usize;
    fn SetWindowLongPtrW(window: *mut c_void, index: i32, value: isize) -> isize;
    fn CallWindowProcW(
        previous: WndProc,
        window: *mut c_void,
        message: u32,
        w_param: usize,
        l_param: isize,
    ) -> isize;
}
#[link(name = "opengl32")]
extern "system" {
    fn wglGetCurrentDC() -> *mut c_void;
}

const GWLP_WNDPROC: i32 = -4;
const GCLP_WNDPROC: i32 = -24;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_CHAR: u32 = 0x0102;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_RBUTTONDBLCLK: u32 = 0x0206;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_MOUSEWHEEL: u32 = 0x020A;
const VK_ESCAPE: usize = 0x1B;

static WINDOW: AtomicUsize = AtomicUsize::new(0);
static PREVIOUS: AtomicUsize = AtomicUsize::new(0);
static CLASS_PROC: AtomicUsize = AtomicUsize::new(0);
static CAPTURE: AtomicBool = AtomicBool::new(false);
static BUTTON_HOT: AtomicBool = AtomicBool::new(false);
static LEFT_DOWN: AtomicBool = AtomicBool::new(false);
static RIGHT_DOWN: AtomicBool = AtomicBool::new(false);
static LEFT_PRESSED: AtomicBool = AtomicBool::new(false);
static RIGHT_PRESSED: AtomicBool = AtomicBool::new(false);
static ESCAPE: AtomicBool = AtomicBool::new(false);
static LEFT_HELD: AtomicBool = AtomicBool::new(false);
static RIGHT_HELD: AtomicBool = AtomicBool::new(false);
static TYPING: AtomicBool = AtomicBool::new(false);
static TYPED: Mutex<Vec<char>> = Mutex::new(Vec::new());

thread_local! {
    static FORWARDING: Cell<Option<(u32, usize, isize)>> = const { Cell::new(None) };
}

unsafe fn forward(target: usize, window: *mut c_void, message: u32, w_param: usize, l_param: isize) -> isize {
    if target == 0 {
        return 0;
    }
    unsafe {
        CallWindowProcW(
            std::mem::transmute::<usize, WndProc>(target),
            window,
            message,
            w_param,
            l_param,
        )
    }
}

unsafe extern "system" fn window_proc(
    window: *mut c_void,
    message: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    let current = (message, w_param, l_param);
    if FORWARDING.get() == Some(current) {
        return unsafe { forward(CLASS_PROC.load(Ordering::Acquire), window, message, w_param, l_param) };
    }
    let capture = CAPTURE.load(Ordering::Acquire);
    let swallow = match message {
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            LEFT_DOWN.store(true, Ordering::Release);
            LEFT_PRESSED.store(true, Ordering::Release);
            let ours = capture || BUTTON_HOT.load(Ordering::Acquire);
            LEFT_HELD.store(ours, Ordering::Release);
            ours
        }
        WM_LBUTTONUP => {
            LEFT_DOWN.store(false, Ordering::Release);
            LEFT_HELD.swap(false, Ordering::AcqRel)
        }
        WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
            RIGHT_DOWN.store(true, Ordering::Release);
            RIGHT_PRESSED.store(true, Ordering::Release);
            RIGHT_HELD.store(capture, Ordering::Release);
            capture
        }
        WM_RBUTTONUP => {
            RIGHT_DOWN.store(false, Ordering::Release);
            RIGHT_HELD.swap(false, Ordering::AcqRel)
        }
        WM_MBUTTONDOWN | WM_MBUTTONUP | WM_MOUSEWHEEL => capture,
        WM_KEYDOWN | WM_KEYUP | WM_CHAR if TYPING.load(Ordering::Acquire) => {
            if message == WM_CHAR {
                if let Some(c) = char::from_u32(w_param as u32) {
                    TYPED.lock().unwrap_or_else(|e| e.into_inner()).push(c);
                }
            }
            true
        }
        WM_KEYDOWN | WM_KEYUP | WM_CHAR if capture && w_param == VK_ESCAPE => {
            if message == WM_KEYDOWN {
                ESCAPE.store(true, Ordering::Release);
            }
            true
        }
        WM_KILLFOCUS => {
            LEFT_DOWN.store(false, Ordering::Release);
            RIGHT_DOWN.store(false, Ordering::Release);
            false
        }
        _ => false,
    };
    if swallow {
        return 0;
    }
    let outer = FORWARDING.replace(Some(current));
    let result = unsafe { forward(PREVIOUS.load(Ordering::Acquire), window, message, w_param, l_param) };
    FORWARDING.set(outer);
    result
}

pub(super) fn install() -> bool {
    let window = unsafe { WindowFromDC(wglGetCurrentDC()) };
    if window.is_null() {
        return false;
    }
    let ours = window_proc as WndProc as usize;
    unsafe {
        let current = GetWindowLongPtrW(window, GWLP_WNDPROC) as usize;
        if current == ours {
            WINDOW.store(window as usize, Ordering::Release);
            return true;
        }
        CLASS_PROC.store(GetClassLongPtrW(window, GCLP_WNDPROC), Ordering::Release);
        PREVIOUS.store(current, Ordering::Release);
        if SetWindowLongPtrW(window, GWLP_WNDPROC, ours as isize) == 0 {
            return false;
        }
    }
    WINDOW.store(window as usize, Ordering::Release);
    true
}

pub(super) fn route(editing: bool, button_hot: bool) {
    CAPTURE.store(editing, Ordering::Release);
    BUTTON_HOT.store(button_hot, Ordering::Release);
}

pub(super) fn set_typing(typing: bool) {
    TYPING.store(typing, Ordering::Release);
}

pub(super) fn take_typed() -> Vec<char> {
    std::mem::take(&mut *TYPED.lock().unwrap_or_else(|e| e.into_inner()))
}

pub(super) fn client_size() -> Option<[f32; 2]> {
    let window = WINDOW.load(Ordering::Acquire) as *mut c_void;
    let mut rect = WinRect::default();
    if window.is_null() || unsafe { GetClientRect(window, &mut rect) } == 0 {
        return None;
    }
    let size = [
        (rect.right - rect.left) as f32,
        (rect.bottom - rect.top) as f32,
    ];
    (size[0] >= 1.0 && size[1] >= 1.0).then_some(size)
}

#[derive(Clone, Copy, Default)]
pub(super) struct Button {
    pub down: bool,
    pub pressed: bool,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Mouse {
    pub x: f32,
    pub y: f32,
    pub inside: bool,
    pub left: Button,
    pub right: Button,
    pub escape: bool,
}

pub(super) fn take() -> Mouse {
    let mut mouse = Mouse {
        left: Button {
            down: LEFT_DOWN.load(Ordering::Acquire),
            pressed: LEFT_PRESSED.swap(false, Ordering::AcqRel),
        },
        right: Button {
            down: RIGHT_DOWN.load(Ordering::Acquire),
            pressed: RIGHT_PRESSED.swap(false, Ordering::AcqRel),
        },
        escape: ESCAPE.swap(false, Ordering::AcqRel),
        ..Mouse::default()
    };
    let window = WINDOW.load(Ordering::Acquire) as *mut c_void;
    let mut point = Point::default();
    unsafe {
        if !window.is_null()
            && GetForegroundWindow() == window
            && GetCursorPos(&mut point) != 0
            && ScreenToClient(window, &mut point) != 0
        {
            mouse.inside = true;
            mouse.x = point.x as f32;
            mouse.y = point.y as f32;
        }
    }
    mouse
}
