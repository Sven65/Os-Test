use smoltcp::iface::{Config, Interface, SocketSet, SocketHandle};
use smoltcp::socket::{dhcpv4, icmp, dns, tcp};
use smoltcp::time::Instant;
use smoltcp::wire::*;
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::collections::BTreeMap;
use spin::Mutex;
use lazy_static::lazy_static;
use crate::serial_println;

pub const DNS_CACHE_FILE: &str = "dns.dat";

// ---- Device wrapper ----

pub struct E1000Device;
pub struct E1000RxToken(Vec<u8>);
pub struct E1000TxToken;

impl RxToken for E1000RxToken {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        f(&self.0)
    }
}

impl TxToken for E1000TxToken {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut buf = alloc::vec![0u8; len];
        let result = f(&mut buf);
        if let Some(dev) = crate::device::e1000::E1000_DEV.lock().as_mut() {
            dev.send(&buf);
        }
        result
    }
}

impl Device for E1000Device {
    type RxToken<'a> = E1000RxToken;
    type TxToken<'a> = E1000TxToken;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut buf = alloc::vec![0u8; 4096];
        let len = crate::device::e1000::E1000_DEV.lock().as_mut()?.recv(&mut buf)?;
        buf.truncate(len);
        Some((E1000RxToken(buf), E1000TxToken))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(E1000TxToken)
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1500;
        caps
    }
}

// ---- DNS Cache ----

pub struct DnsCacheEntry {
    pub ip: Ipv4Address,
    pub expires_ms: i64,
}

lazy_static! {
    pub static ref DNS_CACHE: Mutex<BTreeMap<String, DnsCacheEntry>> = Mutex::new(BTreeMap::new());
}

fn cache_get(hostname: &str) -> Option<Ipv4Address> {
    let cache = DNS_CACHE.lock();
    let now = NET.lock().as_ref()?.time_ms;
    serial_println!("[dns] cache_get: {} entries, now={}ms", cache.len(), now);
    let entry = cache.get(hostname)?;
    serial_println!("[dns] found entry expires_ms={}", entry.expires_ms);
    if now < entry.expires_ms {
        Some(entry.ip)
    } else {
        serial_println!("[dns] entry expired");
        None
    }
}

fn cache_set(hostname: &str, ip: Ipv4Address) {
    let ttl_ms = 300_000i64; // 5 minutes
    let now = NET.lock().as_ref().map(|s| s.time_ms).unwrap_or(0);
    DNS_CACHE.lock().insert(
        String::from(hostname),
        DnsCacheEntry { ip, expires_ms: now + ttl_ms },
    );
}

pub fn save_dns_cache() {
    let cache = DNS_CACHE.lock();
    if cache.is_empty() { return; }

    let now = NET.lock().as_ref().map(|s| s.time_ms).unwrap_or(0);
    let mut data = String::new();

    for (host, entry) in cache.iter() {
        if entry.expires_ms > now {
            let remaining = entry.expires_ms - now;
            data.push_str(&alloc::format!("{}={},{}\n", host, entry.ip, remaining));
        }
    }

    crate::fs::write_file(DNS_CACHE_FILE, data.as_bytes());
    serial_println!("[dns] Cache saved ({} entries)", cache.len());
}

pub fn load_dns_cache() {
    let data = match crate::fs::read_file(DNS_CACHE_FILE) {
        Some(d) => d,
        None => { serial_println!("[dns] No cache file"); return; }
    };

    let text = match core::str::from_utf8(&data) {
        Ok(t) => t,
        Err(_) => return,
    };

    let mut cache = DNS_CACHE.lock();
    let mut count = 0;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(eq) = line.find('=') {
            let host = &line[..eq];
            let rest = &line[eq+1..];
            if let Some(comma) = rest.find(',') {
                let ip_str = &rest[..comma];
                let ttl_str = &rest[comma+1..];
                if let (Ok(ip), Ok(ttl)) = (
                    ip_str.parse::<Ipv4Address>(),
                    ttl_str.parse::<i64>(),
                ) {
                    // At boot, time_ms will start near 0, so use ttl directly
                    // as the expiry — it represents remaining ms from last boot
                    cache.insert(String::from(host), DnsCacheEntry {
                        ip,
                        expires_ms: ttl, // ttl IS the remaining time, treat as absolute for now
                    });
                    count += 1;
                }
            }
        }
    }

    serial_println!("[dns] Cache loaded ({} entries)", count);
}

// ---- Network stack ----

pub struct NetStack {
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
    pub time_ms: i64,
    pub ip: Option<Ipv4Address>,
    pub gateway: Option<Ipv4Address>,
    pub prefix_len: u8,
    dhcp_handle: SocketHandle,
}

impl NetStack {
    pub fn poll(&mut self) {
        let mut device = E1000Device;
        let timestamp = Instant::from_millis(self.time_ms);
        self.iface.poll(timestamp, &mut device, &mut self.sockets);

        if let Some(event) = self.sockets.get_mut::<dhcpv4::Socket>(self.dhcp_handle).poll() {
            match event {
                dhcpv4::Event::Configured(config) => {
                    let ip = config.address.address();
                    let prefix = config.address.prefix_len();
                    let gw = config.router.unwrap_or(Ipv4Address::UNSPECIFIED);
                    self.iface.update_ip_addrs(|addrs| {
                        addrs.clear();
                        addrs.push(IpCidr::Ipv4(Ipv4Cidr::new(ip, prefix))).ok();
                    });
                    self.iface.routes_mut().add_default_ipv4_route(gw).ok();
                    self.ip = Some(ip);
                    self.gateway = Some(gw);
                    self.prefix_len = prefix;
                    serial_println!("[net] DHCP: {}/{} gw {}", ip, prefix, gw);
                }
                dhcpv4::Event::Deconfigured => {
                    self.ip = None;
                    self.iface.update_ip_addrs(|addrs| addrs.clear());
                }
            }
        }
    }

    pub fn tick(&mut self, ms: i64) {
        self.time_ms += ms;
    }

    pub fn ping(&mut self, target: Ipv4Address) -> Option<u128> {
        let icmp_socket = icmp::Socket::new(
            icmp::PacketBuffer::new(
                alloc::vec![icmp::PacketMetadata::EMPTY; 4],
                alloc::vec![0u8; 1024],
            ),
            icmp::PacketBuffer::new(
                alloc::vec![icmp::PacketMetadata::EMPTY; 4],
                alloc::vec![0u8; 1024],
            ),
        );
        let handle = self.sockets.add(icmp_socket);

        {
            let socket = self.sockets.get_mut::<icmp::Socket>(handle);
            socket.bind(icmp::Endpoint::Ident(0x1234)).ok()?;
        }

        let payload = b"ping from myos!";
        {
            let socket = self.sockets.get_mut::<icmp::Socket>(handle);
            let repr = Icmpv4Repr::EchoRequest { ident: 0x1234, seq_no: 1, data: payload };
            let buf = socket.send(repr.buffer_len(), IpAddress::Ipv4(target)).ok()?;
            let mut packet = Icmpv4Packet::new_unchecked(buf);
            repr.emit(&mut packet, &smoltcp::phy::ChecksumCapabilities::default());
        }

        let start = self.time_ms;
        for _ in 0..5000 {
            self.poll();
            self.tick(1);
            for _ in 0..100_000 { core::hint::spin_loop(); }

            let socket = self.sockets.get_mut::<icmp::Socket>(handle);
            if socket.can_recv() {
                let rtt = self.time_ms - start;
                self.sockets.remove(handle);
                return Some(rtt as u128);
            }
        }

        self.sockets.remove(handle);
        None
    }
}

lazy_static! {
    pub static ref NET: Mutex<Option<NetStack>> = Mutex::new(None);
}

pub fn init() {
    let mac = match crate::device::e1000::E1000_DEV.lock().as_ref() {
        Some(e) => e.mac,
        None => { serial_println!("[net] e1000 not initialized"); return; }
    };

    let mut device = E1000Device;
    let mac_addr = EthernetAddress(mac);
    let config = Config::new(mac_addr.into());
    let mut iface = Interface::new(config, &mut device, Instant::ZERO);
    iface.set_any_ip(true);

    let mut sockets = SocketSet::new(alloc::vec![]);
    let mut dhcp = dhcpv4::Socket::new();
    dhcp.reset();
    let dhcp_handle = sockets.add(dhcp);

    *NET.lock() = Some(NetStack {
        iface,
        sockets,
        time_ms: 0,
        ip: None,
        gateway: None,
        prefix_len: 0,
        dhcp_handle,
    });

    serial_println!("[net] Stack initialized");
}

pub fn poll() {
    if let Some(stack) = NET.lock().as_mut() {
        stack.poll();
    }
}

pub fn wait_for_dhcp() -> bool {
    serial_println!("[net] Waiting for DHCP...");
    for _ in 0..30_000 {
        {
            let mut guard = NET.lock();
            if let Some(stack) = guard.as_mut() {
                stack.poll();
                stack.tick(1);
                if stack.ip.is_some() {
                    return true;
                }
            }
        }
        for _ in 0..100_000 { core::hint::spin_loop(); }
    }
    serial_println!("[net] DHCP timed out");
    false
}

pub fn get_ip() -> Option<Ipv4Address> {
    NET.lock().as_ref()?.ip
}

pub fn resolve(hostname: &str) -> Option<Ipv4Address> {
    serial_println!("[dns] Resolving {}", hostname);
    if let Some(ip) = cache_get(hostname) {
        serial_println!("[dns] Cache hit: {} -> {}", hostname, ip);
        return Some(ip);
    }
    serial_println!("[dns] Cache miss, querying...");
    let ip = resolve_uncached(hostname)?;
    cache_set(hostname, ip);
    save_dns_cache();
    serial_println!("[dns] Cached {} -> {}", hostname, ip);
    Some(ip)
}

fn resolve_uncached(hostname: &str) -> Option<Ipv4Address> {
    use smoltcp::wire::DnsQueryType;

    let dns_server = IpAddress::Ipv4(Ipv4Address::new(10, 0, 2, 3));

    let mut guard = NET.lock();
    let stack = guard.as_mut()?;

    let socket = dns::Socket::new(&[dns_server], alloc::vec![]);
    let handle = stack.sockets.add(socket);

    let query = {
        let socket = stack.sockets.get_mut::<dns::Socket>(handle);
        match socket.start_query(stack.iface.context(), hostname, DnsQueryType::A) {
            Ok(q) => q,
            Err(e) => {
                serial_println!("[net] DNS start_query error: {:?}", e);
                stack.sockets.remove(handle);
                return None;
            }
        }
    };

    for _ in 0..5000 {
        stack.poll();
        stack.tick(1);
        for _ in 0..100_000 { core::hint::spin_loop(); }

        let socket = stack.sockets.get_mut::<dns::Socket>(handle);
        match socket.get_query_result(query) {
            Ok(addrs) => {
                stack.sockets.remove(handle);
                for addr in addrs.iter() {
                    if let IpAddress::Ipv4(v4) = addr {
                        return Some(*v4);
                    }
                }
                return None;
            }
            Err(dns::GetQueryResultError::Pending) => continue,
            Err(e) => {
                serial_println!("[net] DNS error: {:?}", e);
                break;
            }
        }
    }

    stack.sockets.remove(handle);
    None
}

pub fn http_get(host: &str, path: &str, ip: Ipv4Address, port: u16) -> Option<Vec<u8>> {
    let mut guard = NET.lock();
    let stack = guard.as_mut()?;

    let tcp_rx_buf = tcp::SocketBuffer::new(alloc::vec![0u8; 4096]);
    let tcp_tx_buf = tcp::SocketBuffer::new(alloc::vec![0u8; 4096]);
    let socket = tcp::Socket::new(tcp_rx_buf, tcp_tx_buf);
    let handle = stack.sockets.add(socket);

    {
        let socket = stack.sockets.get_mut::<tcp::Socket>(handle);
        let local_port = 49152 + (stack.time_ms as u16 % 16383);
        socket.connect(
            stack.iface.context(),
            (IpAddress::Ipv4(ip), port),
            local_port,
        ).ok()?;
    }

    let mut connected = false;
    for _ in 0..5000 {
        crate::task::keyboard::process_pending_scancodes();
        if crate::task::keyboard::check_ctrlc() {
            crate::task::keyboard::clear_ctrlc();
            crate::println!("^C");
            stack.sockets.remove(handle);
            return None;
        }

        stack.poll();
        stack.tick(1);
        for _ in 0..100_000 { core::hint::spin_loop(); }

        let socket = stack.sockets.get_mut::<tcp::Socket>(handle);
        if socket.is_active() && socket.may_send() {
            connected = true;
            break;
        }
        if socket.state() == tcp::State::Closed {
            break;
        }
    }

    if !connected {
        serial_println!("[net] TCP connect failed");
        stack.sockets.remove(handle);
        return None;
    }

    {
        let request = alloc::format!(
            "GET {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
            path, host
        );
        let socket = stack.sockets.get_mut::<tcp::Socket>(handle);
        socket.send_slice(request.as_bytes()).ok()?;
    }

    let mut response = alloc::vec![0u8; 8192];
    let mut total = 0;

    for _ in 0..30_000 {
        crate::task::keyboard::process_pending_scancodes();
        if crate::task::keyboard::check_ctrlc() {
            crate::task::keyboard::clear_ctrlc();
            crate::println!("^C");
            stack.sockets.remove(handle);
            return None;
        }

        stack.poll();
        stack.tick(1);
        for _ in 0..100_000 { core::hint::spin_loop(); }

        let socket = stack.sockets.get_mut::<tcp::Socket>(handle);
        if socket.can_recv() {
            let n = socket.recv_slice(&mut response[total..]).unwrap_or(0);
            total += n;
            if total >= response.len() { break; }
        }

        // Break when server has closed and we've read everything
        if !socket.may_recv() && !socket.can_recv() {
            break;
        }

        if !socket.is_active() { break; }
    }

    stack.sockets.remove(handle);

    if total == 0 { return None; }
    response.truncate(total);
    Some(response)
}

pub fn http_get_string(host: &str, path: &str, ip: Ipv4Address, port: u16) -> Option<String> {
    match http_get(host, path, ip, port) {
        Some(bits) => Some(String::from_utf8(bits).unwrap().to_string()),
        None => None,
    }
}