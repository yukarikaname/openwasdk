//! The two `Windows.Foundation` types the dispatcher ABI needs.
//!
//! The App SDK `.winmd` set does not carry `Windows.Foundation`; the Windows SDK
//! metadata does, but that is not part of this repository's inputs. Only two
//! types are actually named by `Microsoft.UI.Dispatching`, so they are defined
//! here by hand from their metadata signatures:
//!
//! - `TypedEventHandler<TSender, TResult>` — the generic delegate every event
//!   on the queue and the timer is declared with.
//! - `Deferral` — the return type of `GetDeferral`, which this DLL answers with
//!   `E_NOTIMPL` and therefore never constructs.
//!
//! `TimeSpan` is not here: `windows-time` defines it, and the generated queue
//! bindings already name that crate. `EventRegistrationToken` is an `i64` in the
//! ABI, which is how the generated methods spell it.

use core::marker::PhantomData;

use windows_core::{GUID, HRESULT, IUnknown, Interface, Ref, Result, RuntimeType};

/// `TypedEventHandler<TSender, TResult>`: `IUnknown` plus `Invoke(sender, args)`.
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedEventHandler<TSender, TResult>(
    IUnknown,
    PhantomData<TSender>,
    PhantomData<TResult>,
)
where
    TSender: RuntimeType + 'static,
    TResult: RuntimeType + 'static;

impl<TSender, TResult> RuntimeType for TypedEventHandler<TSender, TResult>
where
    TSender: RuntimeType + 'static,
    TResult: RuntimeType + 'static,
{
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
        .push_slice(b"pinterface({9de1c534-6ae1-11e0-84e1-18a905bcc53f}")
        .push_slice(b";")
        .push_other(TSender::SIGNATURE)
        .push_slice(b";")
        .push_other(TResult::SIGNATURE)
        .push_slice(b")");
}

unsafe impl<TSender, TResult> Interface for TypedEventHandler<TSender, TResult>
where
    TSender: RuntimeType + 'static,
    TResult: RuntimeType + 'static,
{
    type Vtable = TypedEventHandlerVtbl<TSender, TResult>;
    const IID: GUID = GUID::from_signature(<Self as RuntimeType>::SIGNATURE);
}

impl<TSender, TResult> TypedEventHandler<TSender, TResult>
where
    TSender: RuntimeType + 'static,
    TResult: RuntimeType + 'static,
{
    pub fn new<F: Fn(Ref<TSender>, Ref<TResult>) + 'static>(invoke: F) -> Self {
        let com = windows_core::imp::DelegateBox::<Self, F>::new(
            &TypedEventHandlerBox::<TSender, TResult, F>::VTABLE,
            invoke,
        );
        // Safety: the box was built from this delegate's vtable and is owned.
        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
    }
}

#[repr(C)]
pub struct TypedEventHandlerVtbl<TSender, TResult>
where
    TSender: RuntimeType + 'static,
    TResult: RuntimeType + 'static,
{
    base__: windows_core::IUnknown_Vtbl,
    invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: windows_core::imp::AbiType<TSender>,
        args: windows_core::imp::AbiType<TResult>,
    ) -> HRESULT,
    sender: PhantomData<TSender>,
    result: PhantomData<TResult>,
}

pub struct TypedEventHandlerBox<TSender, TResult, F: Fn(Ref<TSender>, Ref<TResult>) + 'static>(
    PhantomData<TSender>,
    PhantomData<TResult>,
    PhantomData<fn() -> F>,
)
where
    TSender: RuntimeType + 'static,
    TResult: RuntimeType + 'static;

impl<TSender, TResult, F> TypedEventHandlerBox<TSender, TResult, F>
where
    TSender: RuntimeType + 'static,
    TResult: RuntimeType + 'static,
    F: Fn(Ref<TSender>, Ref<TResult>) + 'static,
{
    pub const VTABLE: TypedEventHandlerVtbl<TSender, TResult> =
        TypedEventHandlerVtbl::<TSender, TResult> {
            base__: windows_core::IUnknown_Vtbl {
                QueryInterface: windows_core::imp::DelegateBox::<
                    TypedEventHandler<TSender, TResult>,
                    F,
                >::QueryInterface,
                AddRef: windows_core::imp::DelegateBox::<TypedEventHandler<TSender, TResult>, F>::AddRef,
                Release: windows_core::imp::DelegateBox::<TypedEventHandler<TSender, TResult>, F>::Release,
            },
            invoke: Self::invoke,
            sender: PhantomData,
            result: PhantomData,
        };

    unsafe extern "system" fn invoke(
        this: *mut core::ffi::c_void,
        sender: windows_core::imp::AbiType<TSender>,
        args: windows_core::imp::AbiType<TResult>,
    ) -> HRESULT {
        // Safety: `this` is the DelegateBox this vtable was installed on.
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<TypedEventHandler<TSender, TResult>, F>);
            (this.invoke)(
                core::mem::transmute_copy(&sender),
                core::mem::transmute_copy(&args),
            );
        }
        HRESULT(0)
    }
}

windows_core::imp::define_interface!(
    IDeferral,
    IDeferral_Vtbl,
    0xd6269732_3b7f_46a7_b40b_4fdca2a2c693
);

impl RuntimeType for IDeferral {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
    const NAME: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"Windows.Foundation.IDeferral");
}

#[repr(C)]
pub struct IDeferral_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Complete: unsafe extern "system" fn(*mut core::ffi::c_void) -> HRESULT,
}

/// `Windows.Foundation.Deferral`, the class that carries `IDeferral`.
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deferral(IUnknown);

impl RuntimeType for Deferral {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDeferral>();
}

impl windows_core::RuntimeName for Deferral {
    const NAME: &'static str = "Windows.Foundation.Deferral";
}

unsafe impl Interface for Deferral {
    type Vtable = IDeferral_Vtbl;
    const IID: GUID = <IDeferral as Interface>::IID;
}

impl core::ops::Deref for Deferral {
    type Target = IDeferral;
    fn deref(&self) -> &Self::Target {
        // Safety: the wrapper is transparent over the interface pointer.
        unsafe { core::mem::transmute(self) }
    }
}

unsafe impl Send for Deferral {}
unsafe impl Sync for Deferral {}

/// A deferral this DLL never creates still has to answer its one method.
impl IDeferral {
    pub fn Complete(&self) -> Result<()> {
        // Safety: the vtable slot belongs to a live `IDeferral`.
        unsafe { (Interface::vtable(self).Complete)(Interface::as_raw(self)).ok() }
    }
}
