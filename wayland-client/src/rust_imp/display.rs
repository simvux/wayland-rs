use std::io;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex};

use wayland_commons::map::{Object, ObjectMap};
use wayland_commons::wire::Message;
use wayland_commons::MessageGroup;

use crate::protocol::wl_display::{self, WlDisplay};

use crate::{ConnectError, ProtocolError, Proxy};

use super::connection::{Connection, Error as CxError};
use super::proxy::{ObjectMeta, ProxyInner};
use super::{Dispatched, EventQueueInner, ProxyMap};

pub(crate) struct DisplayInner {
    connection: Arc<Mutex<Connection>>,
    proxy: Proxy<WlDisplay>,
}

impl DisplayInner {
    pub unsafe fn from_fd(fd: RawFd) -> Result<Arc<DisplayInner>, ConnectError> {
        // The special buffer for display events
        let buffer = super::queues::create_queue_buffer();
        let display_object = Object::from_interface::<WlDisplay>(1, ObjectMeta::new(buffer.clone()));
        let (connection, map) = {
            let c = Connection::new(fd, display_object);
            let m = c.map.clone();
            (Arc::new(Mutex::new(c)), m)
        };

        // Setup the display dispatcher
        map.lock()
            .unwrap()
            .with(1, |obj| {
                obj.meta.dispatcher = Arc::new(Mutex::new(DisplayDispatcher {
                    map: map.clone(),
                    last_error: connection.lock().unwrap().last_error.clone(),
                }));
            })
            .unwrap();

        let display_proxy = ProxyInner::from_id(1, map.clone(), connection.clone()).unwrap();

        let display = DisplayInner {
            proxy: Proxy::wrap(display_proxy),
            connection,
        };

        Ok(Arc::new(display))
    }

    pub(crate) fn flush(&self) -> io::Result<()> {
        match self.connection.lock().unwrap().flush() {
            Ok(()) => Ok(()),
            Err(::nix::Error::Sys(errno)) => Err(errno.into()),
            Err(_) => unreachable!(),
        }
    }

    pub(crate) fn create_event_queue(me: &Arc<DisplayInner>) -> EventQueueInner {
        EventQueueInner::new(me.connection.clone(), None)
    }

    pub(crate) fn get_proxy(&self) -> &Proxy<WlDisplay> {
        &self.proxy
    }

    pub(crate) fn protocol_error(&self) -> Option<ProtocolError> {
        let cx = self.connection.lock().unwrap();
        let last_error = cx.last_error.lock().unwrap();
        if let Some(CxError::Protocol(ref e)) = *last_error {
            Some(e.clone())
        } else {
            None
        }
    }
}

// WlDisplay needs its own dispatcher, as it can be dispatched from multiple threads
struct DisplayDispatcher {
    map: Arc<Mutex<ObjectMap<ObjectMeta>>>,
    last_error: Arc<Mutex<Option<CxError>>>,
}

impl super::Dispatcher for DisplayDispatcher {
    fn dispatch(&mut self, msg: Message, proxy: ProxyInner, map: &mut ProxyMap) -> Dispatched {
        let opcode = msg.opcode as usize;
        if ::std::env::var_os("WAYLAND_DEBUG").is_some() {
            eprintln!(
                " <- {}@{}: {} {:?}",
                proxy.object.interface, proxy.id, proxy.object.events[opcode].name, msg.args
            );
        }
        let event = match wl_display::Event::from_raw(msg, map) {
            Ok(v) => v,
            Err(()) => return Dispatched::BadMsg,
        };
        match event {
            wl_display::Event::Error {
                object_id,
                code,
                message,
            } => {
                eprintln!(
                    "[wayland-client] Protocol error {} on object {}@{}: {}",
                    code,
                    object_id.as_ref().inner.object.interface,
                    object_id.as_ref().id(),
                    message
                );
                *self.last_error.lock().unwrap() = Some(CxError::Protocol(ProtocolError {
                    code,
                    object_id: object_id.as_ref().id(),
                    object_interface: object_id.as_ref().inner.object.interface,
                    message,
                }));
            }
            wl_display::Event::DeleteId { id } => {
                // cleanup the map as appropriate
                let mut map = self.map.lock().unwrap();
                let client_destroyed = map
                    .with(id, |obj| {
                        obj.meta.server_destroyed = true;
                        obj.meta.client_destroyed
                    })
                    .unwrap_or(false);
                if client_destroyed {
                    map.remove(id);
                }
            }
            _ => unreachable!(),
        }
        Dispatched::Yes
    }
}
