//! Сетевые проверки: DNS, TCP, TLS с SNI, HTTP, ICMP.

use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows_sys::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho};

#[derive(Debug, Clone, Default)]
pub struct Stage {
    pub ok: bool,
    pub ms: u64,
    pub error: String,
}

/// DNS с таймаутом.
pub fn resolve(host: &str, timeout_ms: u64) -> Result<Vec<IpAddr>, String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![ip]);
    }
    let host_owned = host.to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (host_owned.as_str(), 443u16)
            .to_socket_addrs()
            .map(|iter| iter.map(|a| a.ip()).collect::<Vec<IpAddr>>())
            .map_err(|e| describe_io(&e));
        let _ = tx.send(result);
    });
    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(Ok(mut ips)) => {
            let mut v4: Vec<IpAddr> = ips.iter().filter(|i| i.is_ipv4()).copied().collect();
            let v6: Vec<IpAddr> = ips.iter().filter(|i| i.is_ipv6()).copied().collect();
            ips.clear();
            v4.extend(v6);
            if v4.is_empty() {
                Err("нет A/AAAA-записей".to_string())
            } else {
                Ok(v4)
            }
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(format!("таймаут {} мс", timeout_ms)),
    }
}

fn connect_addrs(ips: &[IpAddr], port: u16, timeout_ms: u64) -> Result<(TcpStream, u64), String> {
    let mut last_error = String::from("нет адресов");
    for ip in ips {
        let addr = SocketAddr::new(*ip, port);
        let start = Instant::now();
        match TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms)) {
            Ok(s) => {
                let _ = s.set_nodelay(true);
                return Ok((s, start.elapsed().as_millis() as u64));
            }
            Err(e) => last_error = describe_io(&e),
        }
    }
    Err(last_error)
}

/// TCP-соединение с портом (разрешает имя, пробует все адреса).
pub fn tcp_connect(host: &str, port: u16, timeout_ms: u64) -> Stage {
    let start = Instant::now();
    let ips = match resolve(host, timeout_ms) {
        Ok(ips) => ips,
        Err(e) => {
            return Stage {
                ok: false,
                ms: start.elapsed().as_millis() as u64,
                error: e,
            }
        }
    };
    match connect_addrs(&ips, port, timeout_ms) {
        Ok((_stream, ms)) => Stage {
            ok: true,
            ms,
            error: String::new(),
        },
        Err(e) => Stage {
            ok: false,
            ms: start.elapsed().as_millis() as u64,
            error: e,
        },
    }
}

/// TCP + TLS-рукопожатие с указанным SNI (сертификат не проверяется).
pub fn tls_handshake(host: &str, port: u16, timeout_ms: u64) -> Stage {
    let start = Instant::now();
    let ips = match resolve(host, timeout_ms.min(5000)) {
        Ok(ips) => ips,
        Err(e) => {
            return Stage {
                ok: false,
                ms: start.elapsed().as_millis() as u64,
                error: e,
            }
        }
    };
    let timeout = Duration::from_millis(timeout_ms);
    let mut last_error = String::from("нет адресов");
    for ip in ips {
        let addr = SocketAddr::new(ip, port);
        let stream = match TcpStream::connect_timeout(&addr, timeout) {
            Ok(s) => s,
            Err(e) => {
                last_error = describe_io(&e);
                continue;
            }
        };
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
        let connector = match native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return Stage {
                    ok: false,
                    ms: start.elapsed().as_millis() as u64,
                    error: e.to_string(),
                }
            }
        };
        match connector.connect(host, stream) {
            Ok(_tls) => {
                return Stage {
                    ok: true,
                    ms: start.elapsed().as_millis() as u64,
                    error: String::new(),
                }
            }
            Err(e) => last_error = describe_tls(&e.to_string()),
        }
    }
    Stage {
        ok: false,
        ms: start.elapsed().as_millis() as u64,
        error: last_error,
    }
}

#[derive(Debug, Clone, Default)]
pub struct HttpProbe {
    pub status: u16,
    pub title: String,
    pub error: String,
}

/// GET главной страницы сайта через WinHTTP.
pub fn http_get(host: &str, timeout_ms: u32) -> HttpProbe {
    let url = format!("https://{}/", host);
    match super::http::request(
        &url,
        "GET",
        &[
            (
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
            ),
            ("Accept", "text/html,application/xhtml+xml"),
        ],
        None,
        timeout_ms,
    ) {
        Ok(resp) => {
            let text = resp.text();
            HttpProbe {
                status: resp.status,
                title: extract_title(&text),
                error: String::new(),
            }
        }
        Err(e) => HttpProbe {
            status: 0,
            title: String::new(),
            error: e,
        },
    }
}

pub fn extract_title(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(start) = lower.find("<title") else {
        return String::new();
    };
    let Some(open_end) = lower[start..].find('>') else {
        return String::new();
    };
    let content_start = start + open_end + 1;
    let Some(end) = lower[content_start..].find("</title>") else {
        return String::new();
    };
    let raw = &html[content_start..content_start + end];
    crate::util::normalize_ws(raw)
}

/// ICMP ping. Возвращает (успех, мс, ошибка).
pub fn ping(host: &str, timeout_ms: u32) -> (bool, u64, String) {
    let ip = match host.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => match resolve(host, timeout_ms as u64) {
            Ok(ips) => match ips.first() {
                Some(ip) => *ip,
                None => return (false, 0, "нет адреса".to_string()),
            },
            Err(e) => return (false, 0, e),
        },
    };
    let v4 = match ip {
        IpAddr::V4(v4) => v4,
        IpAddr::V6(_) => return (false, 0, "IPv6 не поддерживается".to_string()),
    };
    unsafe {
        let handle = IcmpCreateFile();
        if handle.is_null() {
            return (false, 0, "ICMP недоступен".to_string());
        }
        let addr = u32::from_le_bytes(v4.octets());
        let payload = [0u8; 32];
        let mut reply = vec![0u8; 32 + std::mem::size_of::<IcmpReply>() + 64];
        let count = IcmpSendEcho(
            handle,
            addr,
            payload.as_ptr() as *const core::ffi::c_void,
            payload.len() as u16,
            std::ptr::null(),
            reply.as_mut_ptr() as *mut core::ffi::c_void,
            reply.len() as u32,
            timeout_ms,
        );
        IcmpCloseHandle(handle);
        if count == 0 {
            return (false, 0, "нет ответа".to_string());
        }
        let parsed: &IcmpReply = &*(reply.as_ptr() as *const IcmpReply);
        if parsed.status == 0 {
            (true, parsed.round_trip_time as u64, String::new())
        } else {
            (false, 0, format!("ICMP статус {}", parsed.status))
        }
    }
}

#[repr(C)]
struct IcmpReply {
    address: u32,
    status: u32,
    round_trip_time: u32,
    data_size: u16,
    reserved: u16,
}

fn describe_io(e: &std::io::Error) -> String {
    match e.kind() {
        ErrorKind::TimedOut => "таймаут".to_string(),
        ErrorKind::ConnectionReset => "соединение сброшено (reset)".to_string(),
        ErrorKind::ConnectionRefused => "в соединении отказано".to_string(),
        ErrorKind::ConnectionAborted => "соединение прервано".to_string(),
        ErrorKind::UnexpectedEof => "соединение закрыто (eof)".to_string(),
        ErrorKind::HostUnreachable => "хост недоступен".to_string(),
        ErrorKind::NetworkUnreachable => "сеть недоступна".to_string(),
        _ => {
            let text = e.to_string();
            let lower = text.to_lowercase();
            if lower.contains("timed out") {
                "таймаут".to_string()
            } else if lower.contains("forcibly") || lower.contains("reset") {
                "соединение сброшено (reset)".to_string()
            } else {
                text
            }
        }
    }
}

fn describe_tls(text: &str) -> String {
    let text = text.to_string();
    let lower = text.to_lowercase();
    if lower.contains("timed out") {
        "таймаут".to_string()
    } else if lower.contains("reset") || lower.contains("forcibly") || lower.contains("eof") {
        "соединение сброшено (reset)".to_string()
    } else {
        text
    }
}

/// Быстрая проверка «похоже на блокировку ТСПУ»: reset/таймаут на TCP или TLS.
pub fn looks_blocked(host: &str) -> (bool, String) {
    let tcp = tcp_connect(host, 443, 2500);
    if !tcp.ok {
        let e = tcp.error.to_lowercase();
        if e.contains("таймаут") || e.contains("reset") || e.contains("сброс") {
            return (true, format!("TCP: {}", tcp.error));
        }
        return (false, String::new());
    }
    let tls = tls_handshake(host, 443, 4000);
    if !tls.ok {
        let e = tls.error.to_lowercase();
        if e.contains("reset")
            || e.contains("сброс")
            || e.contains("таймаут")
            || e.contains("eof")
            || e.contains("forcibly")
            || e.contains("закрыт")
        {
            return (true, format!("TLS: {}", tls.error));
        }
    }
    (false, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title() {
        assert_eq!(
            extract_title("<html><head><TITLE>Привет  мир</TITLE></head>"),
            "Привет мир"
        );
    }
}
