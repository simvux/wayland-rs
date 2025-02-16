use std::cell::RefCell;
use std::io;
use std::sync::Arc;

use crate::{AnonymousObject, Main, RawEvent};
use wayland_sys::client::*;

use super::DisplayInner;

scoped_tls::scoped_thread_local! {
    pub(crate) static FALLBACK: RefCell<&mut dyn FnMut(RawEvent, Main<AnonymousObject>)>
}

fn with_fallback<T, FB, F>(mut fb: FB, f: F) -> T
where
    FB: FnMut(RawEvent, Main<AnonymousObject>),
    F: FnOnce() -> T,
{
    // We erase the lifetime of the callback to be able to store it in the tls,
    // it's safe as it'll only last until the end of this function call anyway
    let fb = unsafe { std::mem::transmute(&mut fb as &mut dyn FnMut(_, _)) };
    FALLBACK.set(&RefCell::new(fb), || f())
}

pub(crate) struct EventQueueInner {
    wlevq: *mut wl_event_queue,
    inner: Arc<super::DisplayInner>,
}

impl EventQueueInner {
    pub(crate) fn new(inner: Arc<DisplayInner>, wlevq: *mut wl_event_queue) -> EventQueueInner {
        EventQueueInner { inner, wlevq }
    }

    pub(crate) fn get_connection_fd(&self) -> ::std::os::unix::io::RawFd {
        unsafe { ffi_dispatch!(WAYLAND_CLIENT_HANDLE, wl_display_get_fd, self.inner.ptr()) }
    }

    pub fn dispatch<F>(&self, fallback: F) -> io::Result<u32>
    where
        F: FnMut(RawEvent, Main<AnonymousObject>),
    {
        with_fallback(fallback, || {
            let ret = unsafe {
                ffi_dispatch!(
                    WAYLAND_CLIENT_HANDLE,
                    wl_display_dispatch_queue,
                    self.inner.ptr(),
                    self.wlevq
                )
            };
            if ret >= 0 {
                Ok(ret as u32)
            } else {
                Err(io::Error::last_os_error())
            }
        })
    }

    pub fn dispatch_pending<F>(&self, fallback: F) -> io::Result<u32>
    where
        F: FnMut(RawEvent, Main<AnonymousObject>),
    {
        with_fallback(fallback, || {
            let ret = unsafe {
                ffi_dispatch!(
                    WAYLAND_CLIENT_HANDLE,
                    wl_display_dispatch_queue_pending,
                    self.inner.ptr(),
                    self.wlevq
                )
            };
            if ret >= 0 {
                Ok(ret as u32)
            } else {
                Err(io::Error::last_os_error())
            }
        })
    }

    pub fn sync_roundtrip<F>(&self, fallback: F) -> io::Result<u32>
    where
        F: FnMut(RawEvent, Main<AnonymousObject>),
    {
        with_fallback(fallback, || {
            let ret = unsafe {
                ffi_dispatch!(
                    WAYLAND_CLIENT_HANDLE,
                    wl_display_roundtrip_queue,
                    self.inner.ptr(),
                    self.wlevq
                )
            };
            if ret >= 0 {
                Ok(ret as u32)
            } else {
                Err(io::Error::last_os_error())
            }
        })
    }

    pub(crate) fn prepare_read(&self) -> Result<(), ()> {
        let ret = unsafe {
            ffi_dispatch!(
                WAYLAND_CLIENT_HANDLE,
                wl_display_prepare_read_queue,
                self.inner.ptr(),
                self.wlevq
            )
        };
        if ret >= 0 {
            Ok(())
        } else {
            Err(())
        }
    }

    pub(crate) fn read_events(&self) -> io::Result<i32> {
        let ret = unsafe { ffi_dispatch!(WAYLAND_CLIENT_HANDLE, wl_display_read_events, self.inner.ptr()) };
        if ret >= 0 {
            Ok(ret)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(crate) fn cancel_read(&self) {
        unsafe { ffi_dispatch!(WAYLAND_CLIENT_HANDLE, wl_display_cancel_read, self.inner.ptr()) }
    }

    pub(crate) unsafe fn assign_proxy(&self, proxy: *mut wl_proxy) {
        ffi_dispatch!(WAYLAND_CLIENT_HANDLE, wl_proxy_set_queue, proxy, self.wlevq)
    }
}

impl Drop for EventQueueInner {
    fn drop(&mut self) {
        unsafe {
            ffi_dispatch!(WAYLAND_CLIENT_HANDLE, wl_event_queue_destroy, self.wlevq);
        }
    }
}
