//! mDNS: sunucu `_kopru._udp` ilan eder (TXT: id, fp, ad); istemci sürekli dinler.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::types::ServerInfo;

pub const SERVICE: &str = "_kopru._udp.local.";

pub struct Advertiser {
    daemon: ServiceDaemon,
}

impl Advertiser {
    pub fn start(id: &str, name: &str, fp: &str, port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new()?;
        let host = format!("kopru-{id}.local.");
        let props = [("id", id), ("fp", fp), ("n", name), ("v", "1")];
        let info = ServiceInfo::new(SERVICE, &format!("kopru-{id}"), &host, "", port, &props[..])?
            .enable_addr_auto();
        daemon.register(info)?;
        Ok(Self { daemon })
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

/// Keşfedilen sunucular (id → bilgi). Bulma kapatılınca daemon da kapanır.
pub struct Browser {
    daemon: ServiceDaemon,
    pub found: Arc<Mutex<HashMap<String, ServerInfo>>>,
}

impl Browser {
    /// `on_change` her değişiklikte güncel listeyle çağrılır (daemon iş parçacığından).
    pub fn start(on_change: impl Fn(Vec<ServerInfo>) + Send + 'static) -> Result<Self> {
        let daemon = ServiceDaemon::new()?;
        let rx = daemon.browse(SERVICE)?;
        let found: Arc<Mutex<HashMap<String, ServerInfo>>> = Default::default();
        let f2 = found.clone();
        std::thread::Builder::new().name("kopru-mdns".into()).spawn(move || {
            // fullname → id eşlemesi, kaldırma olayları için.
            let mut names: HashMap<String, String> = HashMap::new();
            while let Ok(ev) = rx.recv() {
                let changed = match ev {
                    ServiceEvent::ServiceResolved(info) => {
                        let (Some(id), Some(fp)) = (info.get_property_val_str("id"), info.get_property_val_str("fp")) else {
                            continue;
                        };
                        let name = info.get_property_val_str("n").unwrap_or("PC").to_string();
                        let mut addrs: Vec<String> =
                            info.get_addresses_v4().iter().map(|ip| format!("{ip}:{}", info.port)).collect();
                        addrs.sort();
                        let s = ServerInfo { id: id.to_string(), name, fingerprint: fp.to_string(), addrs, paired: false };
                        names.insert(info.get_fullname().to_string(), s.id.clone());
                        let mut g = f2.lock().unwrap();
                        let prev = g.insert(s.id.clone(), s.clone());
                        prev.as_ref() != Some(&s)
                    }
                    ServiceEvent::ServiceRemoved(_, full) => match names.remove(&full) {
                        Some(id) => f2.lock().unwrap().remove(&id).is_some(),
                        None => false,
                    },
                    ServiceEvent::SearchStopped(_) => break,
                    _ => false,
                };
                if changed {
                    let list: Vec<ServerInfo> = f2.lock().unwrap().values().cloned().collect();
                    on_change(list);
                }
            }
        })?;
        Ok(Self { daemon, found })
    }

    pub fn snapshot(&self) -> Vec<ServerInfo> {
        self.found.lock().unwrap().values().cloned().collect()
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.daemon.stop_browse(SERVICE);
        let _ = self.daemon.shutdown();
    }
}

/// Bu makinenin yerel IPv4 adresleri (QR için). Loopback ve link-local hariç;
/// özel ağ (192.168/10/172.16) adresleri öne alınır.
pub fn local_ipv4s() -> Vec<String> {
    let mut v: Vec<std::net::Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.ip() {
            std::net::IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => Some(ip),
            _ => None,
        })
        .collect();
    v.sort_by_key(|ip| (!ip.is_private(), ip.octets()));
    v.dedup();
    v.into_iter().map(|ip| ip.to_string()).collect()
}
