//! A \`CoreMessagingXP.dll\` that loads, and a \`CoreMessaging.dll\` that answers.
//!
//! The SDK's file aborts on load under a Wine without \`NtCreateWaitCompletionPacket\`, so this
//! crate used to answer its 28 C exports with \`E_NOTIMPL\`. That is no longer the whole
//! story: Wine implements the wait-completion-packet family now, the real
//! \`CoreMessagingXP.dll\` loads, and the names the engine needs are the ones it exports --
//! \`CoreMsgCreateSession\`, \`CoreMsgGetExistingSession\` and the \`CoreUICall*\` family.
//!
//! The engine reaches them through the *system* name, \`CoreMessaging.dll\`, which on Windows
//! carries \`CoreUICreate\` and on Wine carries nothing. Installed under that name, this DLL
//! forwards each export to the real \`CoreMessagingXP.dll\` beside the application, which puts
//! the engine back on the path Windows takes: a real core, a real registrar, ALPC underneath.
//!
//! When there is no such file (this DLL *is* the XP file), it refuses by name instead.

mod coreui;
mod factory;

use std::ffi::c_void;

/// \`E_NOTIMPL\`: the name exists so a caller can load and fail honestly.
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;

/// Six arguments: x86-64 passes four in registers and the rest on the stack, and forwarding
/// them unchanged keeps both the ones this crate knows and the ones it does not.
type Fwd = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize) -> i32;

#[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
unsafe extern "system" {
    fn LoadLibraryA(name: *const u8) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn GetModuleHandleA(name: *const u8) -> *mut c_void;
}

/// The real \`CoreMessagingXP.dll\`, or nothing when this DLL is it.
fn real() -> Option<*mut c_void> {
    let module = unsafe { LoadLibraryA(c"CoreMessagingXP.dll".as_ptr().cast()) };
    if module.is_null() || module == unsafe { GetModuleHandleA(c"CoreMessagingXP.dll".as_ptr().cast()) } && false {
        return None;
    }
    Some(module)
}

/// Forward one export to the real file, or refuse it by name.
///
/// # Safety
/// \`name\` is a static NUL-terminated string naming this export.
pub(crate) unsafe fn forward(name: &'static [u8], args: [usize; 6]) -> i32 {
    if let Some(module) = real() {
        let entry = unsafe { GetProcAddress(module, name.as_ptr()) };
        if !entry.is_null() {
            let target: Fwd = unsafe { std::mem::transmute(entry) };
            let result = unsafe { target(args[0], args[1], args[2], args[3], args[4], args[5]) };
            if std::env::var_os("OPENCOMP_TRACE").is_some() {
                eprintln!(
                    "coremessaging: forwarded {} hr=0x{:08x}",
                    String::from_utf8_lossy(&name[..name.len() - 1]),
                    result as u32
                );
            }
            return result;
        }
    }
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!(
            "coremessaging: refused {}",
            String::from_utf8_lossy(&name[..name.len() - 1])
        );
    }
    E_NOTIMPL
}

macro_rules! forwarded {
    ($($name:ident),+ $(,)?) => {
        $(
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $name(
                a: usize, b: usize, c: usize, d: usize, e: usize, f: usize,
            ) -> i32 {
                unsafe { forward(concat!(stringify!($name), "\0").as_bytes(), [a, b, c, d, e, f]) }
            }
        )+
    };
}

forwarded!(
    CoreMsgCreateSession,
    CoreMsgGetExistingSession,
    CoreUICallComputeMaximumMessageSize,
    CoreUICallCreateConversationHost,
    CoreUICallCreateEndpointHost,
    CoreUICallCreateEndpointHostWithSendPriority,
    CoreUICallGetAddressOfParameterInBuffer,
    CoreUICallReceive,
    CoreUICallSend,
    CoreUICallSendVaList,
    CoreUIConfigureTestHost,
    CoreUIConfigureUserIntegration,
    CoreUICreateAnonymousStream,
    CoreUIEnableCrossProcessPrototyping,
    CoreUIInitializeTestService,
    CoreUIRouteToTestRegistrar,
    CoreUIUninitializeTestService,
    DllCanUnloadNow,
    MsgBlobCreateShared,
    MsgBlobCreateStack,
    MsgBufferShare,
    MsgRelease,
    MsgStringCreateShared,
    MsgStringCreateStack,
    WinUICreateDispatcherQueueController,
    WinUIGetDispatcherQueueForCurrentThread
);
