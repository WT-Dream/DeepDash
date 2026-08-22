use std::net::Ipv4Addr;

use get_if_addrs::{get_if_addrs, IfAddr};

use crate::{error_mapper::LauncherError, models::LanHost};

pub fn lan_hosts() -> Result<Vec<LanHost>, LauncherError> {
    let mut hosts = get_if_addrs()
        .map_err(|error| {
            LauncherError::new("lanDiscoveryFailed", "无法读取本机局域网地址。")
                .with_detail(error.to_string())
        })?
        .into_iter()
        .filter_map(|interface| match interface.addr {
            IfAddr::V4(address) if is_private_lan_ip(address.ip) => Some(LanHost {
                name: interface.name,
                address: address.ip.to_string(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    hosts.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.address.cmp(&right.address))
    });
    hosts.dedup_by(|left, right| left.address == right.address);
    Ok(hosts)
}

pub fn selected_lan_host(value: &str) -> Result<LanHost, LauncherError> {
    let hosts = lan_hosts()?;
    hosts
        .into_iter()
        .find(|host| host.address == value)
        .ok_or_else(|| {
            LauncherError::new("lanHostUnavailable", "所选局域网地址当前不可用。")
                .with_action("确认电脑已连接到可信 Wi-Fi，然后重新选择地址并重启 DSH。")
        })
}

pub fn is_private_lan_address(value: &str) -> bool {
    value.parse::<Ipv4Addr>().is_ok_and(is_private_lan_ip)
}

fn is_private_lan_ip(ip: Ipv4Addr) -> bool {
    ip.is_private()
        && !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_unspecified()
        && !ip.is_multicast()
        && !ip.is_broadcast()
}

#[cfg(test)]
mod tests {
    use super::is_private_lan_address;

    #[test]
    fn accepts_private_ipv4_addresses() {
        assert!(is_private_lan_address("192.168.2.9"));
        assert!(is_private_lan_address("10.0.0.12"));
        assert!(is_private_lan_address("172.16.0.1"));
    }

    #[test]
    fn rejects_public_and_special_addresses() {
        assert!(!is_private_lan_address("127.0.0.1"));
        assert!(!is_private_lan_address("0.0.0.0"));
        assert!(!is_private_lan_address("8.8.8.8"));
        assert!(!is_private_lan_address("169.254.1.2"));
    }
}
