//! C ABI ownership: handles are opaque; all exposed strings remain Rust-owned.
use crate::ui::App;
use std::{
    ffi::{CStr, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AppDraw {
    pub kind: u32,
    pub rgba: u32,
    pub font: u32,
    pub align: u32,
    pub rect: [f32; 4],
    pub clip: [f32; 4],
    pub uv: [f32; 4],
    pub font_size: f32,
    pub rotation: f32,
    pub min_font_size: f32,
    pub reserved: u32,
    pub text: *const c_char,
    pub xy: [f32; 8],
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AppSound {
    pub pcm: *const f32,
    pub frames: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AppSoundCommand {
    pub clip: u32,
    pub voice: u32,
    pub action: u32,
    pub gain: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AppFrame {
    pub draws: *const AppDraw,
    pub count: u32,
    pub wants_text: u32,
    pub width: f32,
    pub height: f32,
    pub input_rect: [f32; 4],
    pub audio_pcm: *const f32,
    pub audio_frames: u32,
    pub audio_epoch: u32,
    pub audio_state: u32,
    pub reserved: u32,
    pub sounds: *const AppSound,
    pub sound_count: u32,
    pub sound_state: u32,
    pub sound_commands: *const AppSoundCommand,
    pub sound_command_count: u32,
    pub sound_sequence: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AppEvent {
    pub kind: u32,
    pub key: u32,
    pub x: f32,
    pub y: f32,
    pub delta: f32,
    pub modifiers: u32,
    pub text: *const c_char,
}
#[repr(C)]
pub struct ApplicationV5 {
    abi: u32,
    size: u32,
    name: *const c_char,
    create: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void,
    destroy: unsafe extern "C" fn(*mut c_void),
    font_path: unsafe extern "C" fn(*mut c_void, u32) -> *const c_char,
    font_samples: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    frame: unsafe extern "C" fn(*mut c_void, f32, u32, *mut AppFrame) -> i32,
    event: unsafe extern "C" fn(*mut c_void, *const AppEvent) -> i32,
    take_request: unsafe extern "C" fn(*mut c_void) -> u32,
}
// Immutable function table and static string; mutable application state is never
// stored here and is accessed only by the owning editor thread.
unsafe impl Sync for ApplicationV5 {}
unsafe fn string<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("")
    }
}
unsafe extern "C" fn create(project: *const c_char, data: *const c_char) -> *mut c_void {
    match catch_unwind(|| App::new(unsafe { string(project) }, unsafe { string(data) })) {
        Ok(Ok(app)) => Box::into_raw(Box::new(app)).cast(),
        Ok(Err(e)) => {
            eprintln!("OpenSekai: {e}");
            ptr::null_mut()
        }
        Err(_) => ptr::null_mut(),
    }
}
unsafe extern "C" fn destroy(p: *mut c_void) {
    if !p.is_null() {
        drop(unsafe { Box::from_raw(p.cast::<App>()) });
    }
}
unsafe extern "C" fn font_path(p: *mut c_void, index: u32) -> *const c_char {
    if p.is_null() || !(1..=2).contains(&index) {
        return ptr::null();
    }
    unsafe { &*p.cast::<App>() }.fonts[index as usize - 1].as_ptr()
}
unsafe extern "C" fn font_samples(p: *mut c_void) -> *const c_char {
    if p.is_null() {
        return ptr::null();
    }
    unsafe { &*p.cast::<App>() }.samples.as_ptr()
}
unsafe extern "C" fn frame(p: *mut c_void, dt: f32, locale: u32, out: *mut AppFrame) -> i32 {
    if p.is_null() || out.is_null() {
        return 0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let app = unsafe { &mut *p.cast::<App>() };
        app.update(dt);
        let result = app.build(locale != 0);
        unsafe {
            *out = result;
        }
        1
    }))
    .unwrap_or(0)
}
unsafe extern "C" fn event(p: *mut c_void, e: *const AppEvent) -> i32 {
    if p.is_null() || e.is_null() {
        return 0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let e = unsafe { &*e };
        i32::from(unsafe { &mut *p.cast::<App>() }.event(e, unsafe { string(e.text) }))
    }))
    .unwrap_or(0)
}
unsafe extern "C" fn take_request(p: *mut c_void) -> u32 {
    if p.is_null() {
        return 0;
    }
    std::mem::take(&mut unsafe { &mut *p.cast::<App>() }.pending_request)
}
static API: ApplicationV5 = ApplicationV5 {
    abi: 5,
    size: std::mem::size_of::<ApplicationV5>() as u32,
    name: c"OpenSekai / Ojsk Community".as_ptr(),
    create,
    destroy,
    font_path,
    font_samples,
    frame,
    event,
    take_request,
};
#[unsafe(no_mangle)]
pub extern "C" fn asmui_application_get_api(abi: u32) -> *const ApplicationV5 {
    if abi == 5 { &API } else { ptr::null() }
}
const _: () = assert!(std::mem::size_of::<AppDraw>() == 120);
const _: () = assert!(std::mem::size_of::<AppFrame>() == 96);
const _: () = assert!(std::mem::size_of::<AppSound>() == 16);
const _: () = assert!(std::mem::size_of::<AppSoundCommand>() == 16);
const _: () = assert!(std::mem::size_of::<AppEvent>() == 32);
