use alloc::string::String;
use smoltcp::wire::Ipv4Address;
use crate::{print, println};
use crate::shell::commands::Command;

pub struct NetCommand;
impl Command for NetCommand {
    fn name(&self) -> &'static str { "net" }
    fn description(&self) -> &'static str { "Network info: net <status|mac|ip>" }
    fn execute(&self, args: &[String]) {
        let flags = crate::shell::flags::Flags::parse(args);
        let subcmd = flags.get(0).unwrap_or("status");

        match subcmd {
            "status" => {
                let guard = crate::net::NET.lock();
                match guard.as_ref() {
                    Some(stack) => {
                        let e1000 = crate::device::e1000::E1000_DEV.lock();
                        let e = e1000.as_ref().unwrap();
                        let status = e.read_reg_pub(0x0008);
                        let link_up = status & (1 << 1) != 0;
                        let speed = match (status >> 6) & 0x3 {
                            0 => "10 Mbps",
                            1 => "100 Mbps",
                            2 => "1000 Mbps",
                            _ => "unknown",
                        };
                        println!("Link:  {}", if link_up { "UP" } else { "DOWN" });
                        println!("Speed: {}", speed);
                        println!("MAC:   {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                                 e.mac[0], e.mac[1], e.mac[2],
                                 e.mac[3], e.mac[4], e.mac[5]);
                        match stack.ip {
                            Some(ip) => println!("IP:    {}/{}", ip, stack.prefix_len),
                            None => println!("IP:    not configured"),
                        }
                    }
                    None => println!("network not initialized"),
                }
            }
            "mac" => {
                let guard = crate::device::e1000::E1000_DEV.lock();
                match guard.as_ref() {
                    Some(e1000) => println!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                                            e1000.mac[0], e1000.mac[1], e1000.mac[2],
                                            e1000.mac[3], e1000.mac[4], e1000.mac[5]),
                    None => println!("e1000 not initialized"),
                }
            }
            "ip" => {
                match crate::net::get_ip() {
                    Some(ip) => println!("{}", ip),
                    None => println!("not configured"),
                }
            }

            _ => println!("Usage: net <status|mac|ip>"),
        }
    }
}

pub struct PingCommand;
impl Command for PingCommand {
    fn name(&self) -> &'static str { "ping" }
    fn description(&self) -> &'static str { "Ping a host: ping <ip>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: ping <host>"); return; }

        let target = match args[0].parse::<Ipv4Address>() {
            Ok(ip) => ip,
            Err(_) => {
                // Try DNS resolution
                print!("Resolving {}... ", args[0]);
                match crate::net::resolve(&args[0]) {
                    Some(ip) => { println!("{}", ip); ip }
                    None => { println!("failed"); return; }
                }
            }
        };

        println!("Pinging {}...", target);
        let result = crate::net::NET.lock().as_mut().and_then(|s| s.ping(target));
        match result {
            Some(rtt) => println!("Reply from {}: time={}ms", target, rtt),
            None => println!("Request timed out"),
        }
    }
}

pub struct FetchCommand;
impl Command for FetchCommand {
    fn name(&self) -> &'static str { "fetch" }
    fn description(&self) -> &'static str { "HTTP GET: fetch <url>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: fetch <url>"); return; }

        if args[0].starts_with("https://") {
            println!("HTTPS not supported yet, try http://");
            return;
        }

        let url = args[0].trim_start_matches("http://");

        // Split host:port from path
        let (hostport, path) = match url.find('/') {
            Some(pos) => (&url[..pos], &url[pos..]),
            None => (url, "/"),
        };

        // Split host and port
        let (host, port) = match hostport.rfind(':') {
            Some(pos) => (&hostport[..pos], hostport[pos+1..].parse::<u16>().unwrap_or(80)),
            None => (hostport, 80u16),
        };

        // Resolve host
        let ip = match host.parse::<smoltcp::wire::Ipv4Address>() {
            Ok(ip) => ip,
            Err(_) => {
                print!("Resolving {}... ", host);
                match crate::net::resolve(host) {
                    Some(ip) => { println!("{}", ip); ip }
                    None => { println!("failed"); return; }
                }
            }
        };

        println!("Connecting to {}:{}...", ip, port);

        match crate::net::http_get(host, path, ip, port) {
            Some(response) => {
                if let Some(body_start) = response.find("\r\n\r\n") {
                    println!("{}", &response[body_start + 4..]);
                } else {
                    println!("{}", response);
                }
            }
            None => println!("fetch failed"),
        }
    }
}