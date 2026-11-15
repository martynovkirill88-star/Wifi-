use std::net::{IpAddr, Ipv4Addr};
use windows_sys::Win32::NetworkManagement::IpHelper::{GetIfEntry2, MIB_IF_ROW2};

fn priority(ip: Ipv4Addr, physical: bool, up: bool) -> u8 {
    if ip.is_link_local() {
        8
    } else if !up {
        7
    } else if physical {
        0
    } else {
        3
    }
}

pub fn addresses() -> std::io::Result<Vec<String>> {
    let mut addresses = Vec::new();
    for adapter in if_addrs::get_if_addrs()? {
        let IpAddr::V4(ip) = adapter.ip() else {
            continue;
        };
        if ip.is_loopback() {
            continue;
        }
        // Query the OS once at startup; no polling or network packets.
        let (physical, up) = adapter
            .index
            .map(|index| unsafe {
                let mut row: MIB_IF_ROW2 = std::mem::zeroed();
                row.InterfaceIndex = index;
                if GetIfEntry2(&mut row) == 0 {
                    (
                        row.InterfaceAndOperStatusFlags._bitfield & 1 != 0,
                        row.OperStatus == 1,
                    )
                } else {
                    (false, true)
                }
            })
            .unwrap_or((false, true));
        addresses.push((
            priority(ip, physical, up),
            format!("{ip} — {}", adapter.name),
        ));
    }
    addresses.sort();
    addresses.dedup();
    Ok(addresses.into_iter().map(|(_, label)| label).collect())
}
pub fn advertise(fingerprint:&str)->anyhow::Result<mdns_sd::ServiceDaemon> {
    let address = addresses()?.into_iter().next().ok_or_else(||anyhow::anyhow!("Нет локального сетевого адреса"))?;
    let ip = address.split(" — ").next().unwrap();
    let daemon = mdns_sd::ServiceDaemon::new()?;
    let service = mdns_sd::ServiceInfo::new("_localdrop._tcp.local.",&format!("LocalDrop-{}",&fingerprint[..8]),&format!("localdrop-{}.local.",&fingerprint[..12]),ip,45831,&[("fingerprint",fingerprint),("v","1")][..])?;
    daemon.register(service)?;
    Ok(daemon)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_lan_precedes_vpn_and_disconnected_wifi() {
        assert!(
            priority(Ipv4Addr::new(192, 168, 0, 129), true, true)
                < priority(Ipv4Addr::new(172, 19, 0, 1), false, true)
        );
        assert!(
            priority(Ipv4Addr::new(192, 168, 0, 129), true, true)
                < priority(Ipv4Addr::new(169, 254, 57, 252), true, false)
        );
    }
}
