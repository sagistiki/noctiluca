//! Finds screens on the local network: every Android TV (by its remote
//! control service, there whether or not our app is) and every box with
//! the Noctiluca Screen app open (by the app's own announcement).
//!
//! A screen stays on the list until the network says it is gone (its
//! announcement removed or expired): announcements are not repeated while
//! nothing changes, so silence does not mean gone.

use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::thread;

use mdns_sd::{ServiceDaemon, ServiceEvent};

use super::protocol::{PORT, SERVICE_TYPE};

const ANDROID_TV: &str = "_androidtvremote2._tcp.local.";

/// One screen on the network.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Its name as the box calls itself (Settings > Device name).
    pub name: String,
    pub ip: IpAddr,
    /// The app is open on it and listening.
    pub app_open: bool,
    /// It is an Android TV (the app can be installed on it).
    pub android_tv: bool,
    pub model: String,
    pub size: Option<(u32, u32)>,
    pub port: u16,
    /// The app's version, when it is open ("1.1").
    pub app_version: Option<String>,
}

/// An announcement heard.
#[derive(Clone, Debug, Default)]
pub struct Heard {
    pub fullname: String,
    pub ip: Option<IpAddr>,
    pub port: u16,
    pub model: Option<String>,
    pub size: Option<(u32, u32)>,
    pub app_version: Option<String>,
}

/// What is on the network, by address (one box can be heard both as an
/// Android TV and as the app).
#[derive(Default)]
pub struct State {
    found: BTreeMap<IpAddr, Found>,
    /// Which address each announcement was for, to know what a removal means.
    names: HashMap<String, (IpAddr, bool)>,
}

/// The name a service instance gives itself, without the service type.
fn instance_name(fullname: &str, service: &str) -> String {
    fullname.strip_suffix(service).unwrap_or(fullname).trim_end_matches('.').replace("\\032", " ").to_string()
}

impl State {
    /// An announcement resolved: the app's (`app`) or an Android TV's.
    pub fn resolved(&mut self, app: bool, h: Heard) {
        let Some(ip) = h.ip else { return };
        let service = if app { SERVICE_TYPE } else { ANDROID_TV };
        let name = instance_name(&h.fullname, service);
        self.names.insert(h.fullname.clone(), (ip, app));
        let f = self.found.entry(ip).or_insert_with(|| Found {
            name: name.clone(),
            ip,
            app_open: false,
            android_tv: false,
            model: String::new(),
            size: None,
            port: PORT,
            app_version: None,
        });
        if app {
            // The app's name for the box is the one to show.
            f.name = name;
            f.app_open = true;
            f.port = if h.port > 0 { h.port } else { PORT };
            f.model = h.model.unwrap_or_default();
            f.size = h.size;
            // An app from before versions were announced is 1.0.
            f.app_version = Some(h.app_version.unwrap_or_else(|| "1.0".into()));
        } else {
            if !f.app_open {
                f.name = name;
            }
            f.android_tv = true;
        }
    }

    /// An announcement removed (or expired).
    pub fn removed(&mut self, fullname: &str) {
        let Some((ip, app)) = self.names.remove(fullname) else { return };
        let gone = match self.found.get_mut(&ip) {
            Some(f) => {
                if app {
                    f.app_open = false;
                    f.app_version = None;
                } else {
                    f.android_tv = false;
                }
                !f.app_open && !f.android_tv
            }
            None => false,
        };
        if gone {
            self.found.remove(&ip);
        }
    }

    pub fn list(&self) -> Vec<Found> {
        let mut out: Vec<Found> = self.found.values().cloned().collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

/// Listens on the network in the background.
pub struct Discovery {
    state: Arc<Mutex<State>>,
    _daemon: Option<ServiceDaemon>,
}

impl Discovery {
    pub fn start() -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let Ok(daemon) = ServiceDaemon::new() else {
            return Discovery { state, _daemon: None };
        };
        for service in [SERVICE_TYPE, ANDROID_TV] {
            let Ok(rx) = daemon.browse(service) else { continue };
            let st = state.clone();
            thread::Builder::new()
                .name(format!("browse {service}"))
                .spawn(move || {
                    let app = service == SERVICE_TYPE;
                    while let Ok(event) = rx.recv() {
                        match event {
                            ServiceEvent::ServiceResolved(info) => {
                                let prop = |k: &str| info.get_property_val_str(k).map(str::to_string);
                                let size = prop("w").zip(prop("h")).and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                                let heard = Heard {
                                    fullname: info.get_fullname().to_string(),
                                    ip: info.get_addresses_v4().into_iter().next().map(IpAddr::V4),
                                    port: info.get_port(),
                                    model: prop("model"),
                                    size,
                                    app_version: prop("app"),
                                };
                                st.lock().unwrap().resolved(app, heard);
                            }
                            ServiceEvent::ServiceRemoved(_, fullname) => st.lock().unwrap().removed(&fullname),
                            _ => {}
                        }
                    }
                })
                .ok();
        }
        Discovery { state, _daemon: Some(daemon) }
    }

    /// Everything on the network now, by name.
    pub fn found(&self) -> Vec<Found> {
        self.state.lock().unwrap().list()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names_lose_the_service_type() {
        assert_eq!(instance_name("living-room._noctiluca._tcp.local.", SERVICE_TYPE), "living-room");
        assert_eq!(instance_name("סלון TV._androidtvremote2._tcp.local.", ANDROID_TV), "סלון TV");
        assert_eq!(instance_name("Living\\032Room._noctiluca._tcp.local.", SERVICE_TYPE), "Living Room");
    }

    fn heard(name: &str, service: &str, ip: [u8; 4]) -> Heard {
        Heard { fullname: format!("{name}.{service}"), ip: Some(IpAddr::from(ip)), port: PORT, app_version: Some("1.1".into()), ..Heard::default() }
    }

    /// The box stays listed however long it is quiet; it goes only when its
    /// announcements are removed, and the TV part stays when only the app closes.
    #[test]
    fn a_screen_stays_until_the_network_says_it_is_gone() {
        let mut s = State::default();
        s.resolved(false, heard("סלון TV", ANDROID_TV, [192, 168, 1, 50]));
        s.resolved(true, heard("living-room", SERVICE_TYPE, [192, 168, 1, 50]));
        let f = &s.list()[0];
        assert_eq!((f.name.as_str(), f.app_open, f.android_tv, f.app_version.as_deref()), ("living-room", true, true, Some("1.1")));
        // The app closes: still an Android TV on the network.
        s.removed("living-room._noctiluca._tcp.local.");
        let f = &s.list()[0];
        assert!(!f.app_open && f.android_tv && f.app_version.is_none());
        // And back.
        s.resolved(true, heard("living-room", SERVICE_TYPE, [192, 168, 1, 50]));
        assert!(s.list()[0].app_open);
        // Switched off: gone.
        s.removed("living-room._noctiluca._tcp.local.");
        s.removed("סלון TV._androidtvremote2._tcp.local.");
        assert!(s.list().is_empty());
        // A removal of something never heard changes nothing.
        s.removed("nothing._noctiluca._tcp.local.");
    }
}
