//! Common definitions for wayland
//!
//! This crate hosts common type and traits used to represent wayland messages
//! and routines in the `wayland-client` and `wayland-server` crates.
//!
//! This notably includes the `Interface` trait, which can exhaustively describe
//! any wayland interface. Its implementations are intended to be generated by the
//! `wayland-scanner` crate.
//!
//! The principal user-facing definition provided by this crate is the `Implementation`
//! trait, which as a user of `wayland-client` or `wayland-server` you will be using
//! to define objects able to handle the messages your program receives. Note that
//! this trait is auto-implemented for closures with appropriate signature, for
//! convenience.

#![warn(missing_docs)]

#[macro_use]
extern crate nix;

use std::os::raw::c_void;
use wayland_sys::common as syscom;

pub mod filter;
pub mod map;
pub mod socket;
pub mod user_data;
pub mod wire;

pub use smallvec::smallvec;

/// A group of messages
///
/// This represents a group of message that can be serialized on the protocol wire.
/// Typically the set of events or requests of a single interface.
///
/// Implementations of this trait are supposed to be
/// generated using the `wayland-scanner` crate.
pub trait MessageGroup: Sized {
    /// Wire representation of this MessageGroup
    const MESSAGES: &'static [wire::MessageDesc];
    /// The wrapper type for ObjectMap allowing the mapping of Object and
    /// NewId arguments to the object map during parsing.
    type Map;
    /// The opcode of this message
    fn opcode(&self) -> u16;
    /// Whether this message is a destructor
    ///
    /// If it is, once send or receive the associated object cannot be used any more.
    fn is_destructor(&self) -> bool;
    /// The minimal object version for which this message exists
    fn since(&self) -> u32;
    /// Retrieve the child `Object` associated with this message if any
    fn child<Meta: self::map::ObjectMetadata>(
        opcode: u16,
        version: u32,
        meta: &Meta,
    ) -> Option<crate::map::Object<Meta>>;
    /// Construct a message from its raw representation
    fn from_raw(msg: wire::Message, map: &mut Self::Map) -> Result<Self, ()>;
    /// Turn this message into its raw representation
    fn into_raw(self, send_id: u32) -> wire::Message;
    /// Construct a message of this group from its C representation
    unsafe fn from_raw_c(obj: *mut c_void, opcode: u32, args: *const syscom::wl_argument)
        -> Result<Self, ()>;
    /// Build a C representation of this message
    ///
    /// It can only be accessed from the provided closure, and this consumes
    /// the message.
    fn as_raw_c_in<F, T>(self, f: F) -> T
    where
        F: FnOnce(u32, &mut [syscom::wl_argument]) -> T;
}

/// The description of a wayland interface
///
/// Implementations of this trait are supposed to be
/// generated using the `wayland-scanner` crate.
pub trait Interface: 'static {
    /// Set of requests associated to this interface
    ///
    /// Requests are messages from the client to the server
    type Request: MessageGroup + 'static;
    /// Set of events associated to this interface
    ///
    /// Events are messages from the server to the client
    type Event: MessageGroup + 'static;
    /// Name of this interface
    const NAME: &'static str;
    /// Maximum supported version of this interface
    ///
    /// This is the maximum version supported by the protocol specification currently
    /// used by this library, and should not be used as-is in your code, as a version
    /// change can subtly change the behavior of some objects.
    ///
    /// Server are supposed to be able to handle all versions from 1 to the one they
    /// advertise through the registry, and clients can choose any version among the
    /// ones the server supports.
    const VERSION: u32;
    /// Pointer to the C representation of this interface
    fn c_interface() -> *const syscom::wl_interface;
}

/// An empty enum representing a MessageGroup with no messages
pub enum NoMessage {}

#[cfg_attr(tarpaulin, skip)]
impl MessageGroup for NoMessage {
    const MESSAGES: &'static [wire::MessageDesc] = &[];
    type Map = ();
    fn is_destructor(&self) -> bool {
        match *self {}
    }
    fn opcode(&self) -> u16 {
        match *self {}
    }
    fn since(&self) -> u32 {
        match *self {}
    }
    fn child<M: self::map::ObjectMetadata>(_: u16, _: u32, _: &M) -> Option<crate::map::Object<M>> {
        None
    }
    fn from_raw(_: wire::Message, _: &mut ()) -> Result<Self, ()> {
        Err(())
    }
    fn into_raw(self, _: u32) -> wire::Message {
        match self {}
    }
    unsafe fn from_raw_c(
        _obj: *mut c_void,
        _opcode: u32,
        _args: *const syscom::wl_argument,
    ) -> Result<Self, ()> {
        Err(())
    }
    fn as_raw_c_in<F, T>(self, _f: F) -> T
    where
        F: FnOnce(u32, &mut [syscom::wl_argument]) -> T,
    {
        match self {}
    }
}

/// Stores a value in a threadafe container that
/// only lets you access it from its owning thread
pub struct ThreadGuard<T: ?Sized> {
    thread: std::thread::ThreadId,
    val: T,
}

impl<T> ThreadGuard<T> {
    /// Create a new ThreadGuard wrapper
    pub fn new(val: T) -> ThreadGuard<T> {
        ThreadGuard {
            val,
            thread: std::thread::current().id(),
        }
    }
}

impl<T: ?Sized> ThreadGuard<T> {
    /// Access the underlying value
    ///
    /// Panics if done on the wrong thread
    pub fn get(&self) -> &T {
        self.try_get()
            .expect("Attempted to access a ThreadGuard contents from the wrong thread.")
    }

    /// Mutably access the underlying value
    ///
    /// Panics if done on the wrong thread
    pub fn get_mut(&mut self) -> &mut T {
        self.try_get_mut()
            .expect("Attempted to access a ThreadGuard contents from the wrong thread.")
    }

    /// Try to access the underlying value
    ///
    /// Returns `None` if done on the wrong thread
    pub fn try_get(&self) -> Option<&T> {
        if self.thread == ::std::thread::current().id() {
            Some(&self.val)
        } else {
            None
        }
    }

    /// Try to mutably access the underlying value
    ///
    /// Returns `None` if done on the wrong thread
    pub fn try_get_mut(&mut self) -> Option<&mut T> {
        if self.thread == ::std::thread::current().id() {
            Some(&mut self.val)
        } else {
            None
        }
    }
}

unsafe impl<T: ?Sized> Send for ThreadGuard<T> {}
unsafe impl<T: ?Sized> Sync for ThreadGuard<T> {}
