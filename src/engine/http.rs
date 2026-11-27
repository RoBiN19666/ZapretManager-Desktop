//! HTTP-клиент на WinHTTP: системный TLS, редиректы, прогресс загрузки.

use std::io::Write;
use std::path::Path;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption,
    WinHttpSetTimeouts,
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_OPTION_REDIRECT_POLICY,
    WINHTTP_OPTION_SECURE_PROTOCOLS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_RAW_HEADERS_CRLF,
    WINHTTP_QUERY_STATUS_CODE,
};

use super::sys::{error_text, wide};

pub const USER_AGENT: &str = "ZapretManager/2.0 (+https://github.com/RoBiN19666/ZapretManager-Desktop)";

/// Потолок для одного скачивания: защита от «бесконечного» ответа сервера.
pub const MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        super::textfile::decode_bytes(&self.body)
    }

    pub fn location(&self) -> Option<String> {
        self.header("Location").map(|s| s.to_string())
    }
}

struct Handles(Vec<*mut core::ffi::c_void>);

impl Drop for Handles {
    fn drop(&mut self) {
        for h in self.0.iter().rev() {
            if !h.is_null() {
                unsafe {
                    WinHttpCloseHandle(*h);
                }
            }
        }
    }
}

fn split_url(url: &str) -> Option<(String, String, u16, String, bool)> {
    let lower = url.to_ascii_lowercase();
    let (secure, rest) = if let Some(r) = lower.strip_prefix("https://") {
        (true, &url[url.len() - r.len()..])
    } else if let Some(r) = lower.strip_prefix("http://") {
        (false, &url[url.len() - r.len()..])
    } else {
        return None;
    };
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = if let Some(colon) = host_port.rfind(':') {
        match host_port[colon + 1..].parse::<u16>() {
            Ok(p) => (&host_port[..colon], p),
            Err(_) => (host_port, if secure { 443 } else { 80 }),
        }
    } else {
        (host_port, if secure { 443 } else { 80 })
    };
    if host.is_empty() {
        return None;
    }
    Some((
        host.to_string(),
        path.to_string(),
        port,
        String::new(),
        secure,
    ))
}

/// Один HTTP-запрос. `headers` — дополнительные строки "Name: value".
pub fn request(
    url: &str,
    method: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    timeout_ms: u32,
) -> Result<HttpResponse, String> {
    let (host, path, port, _, secure) = split_url(url).ok_or_else(|| format!("Некорректный адрес: {}", url))?;

    let agent = wide(USER_AGENT);
    unsafe {
        let session = WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err(format!("WinHTTP: не удалось создать сессию ({})", error_text(GetLastError())));
        }
        let mut handles = Handles(vec![session]);
        let t = timeout_ms.min(i32::MAX as u32) as i32;
        WinHttpSetTimeouts(session, t, t, t, t);

        let host_w = wide(&host);
        let connect = WinHttpConnect(session, host_w.as_ptr(), port, 0);
        if connect.is_null() {
            return Err(format!("WinHTTP: не удалось подключиться к {} ({})", host, error_text(GetLastError())));
        }
        handles.0.push(connect);

        let verb = wide(method);
        let path_w = wide(&path);
        let flags = if secure { WINHTTP_FLAG_SECURE } else { 0 };
        let request = WinHttpOpenRequest(
            connect,
            verb.as_ptr(),
            path_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            flags,
        );
        if request.is_null() {
            return Err(format!("WinHTTP: не удалось создать запрос ({})", error_text(GetLastError())));
        }
        handles.0.push(request);

        // Редиректы: следуем, но запрещаем понижение https → http
        // (WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP = 1).
        // Политика ALWAYS (2) разрешала скачать архив по открытому каналу.
        let redirect = 1u32;
        WinHttpSetOption(
            request,
            WINHTTP_OPTION_REDIRECT_POLICY,
            &redirect as *const u32 as *const core::ffi::c_void,
            4,
        );
        let protocols: u32 = 0x0800 | 0x2000; // TLS 1.2 | TLS 1.3
        WinHttpSetOption(
            request,
            WINHTTP_OPTION_SECURE_PROTOCOLS,
            &protocols as *const u32 as *const core::ffi::c_void,
            4,
        );

        let mut header_text = String::new();
        header_text.push_str("Accept-Encoding: identity\r\n");
        header_text.push_str("Cache-Control: no-cache\r\n");
        for (k, v) in headers {
            header_text.push_str(k);
            header_text.push_str(": ");
            header_text.push_str(v);
            header_text.push_str("\r\n");
        }
        let headers_w = wide(&header_text);
        let (body_ptr, body_len) = match body {
            Some(b) => (b.as_ptr() as *const core::ffi::c_void, b.len() as u32),
            None => (std::ptr::null(), 0),
        };
        let ok = WinHttpSendRequest(
            request,
            headers_w.as_ptr(),
            u32::MAX,
            body_ptr,
            body_len,
            body_len,
            0,
        );
        if ok == 0 {
            return Err(format!("WinHTTP: запрос не отправлен ({})", error_text(GetLastError())));
        }
        if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
            return Err(format!("WinHTTP: нет ответа ({})", error_text(GetLastError())));
        }

        let mut status: u32 = 0;
        let mut size: u32 = 4;
        if WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut status as *mut u32 as *mut core::ffi::c_void,
            &mut size,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(format!("WinHTTP: не удалось прочитать код ответа ({})", error_text(GetLastError())));
        }

        // Заголовки
        let mut header_size: u32 = 0;
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_RAW_HEADERS_CRLF,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut header_size,
            std::ptr::null_mut(),
        );
        let mut header_buf = vec![0u8; header_size as usize + 4];
        let mut headers = Vec::new();
        if header_size > 0
            && WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_RAW_HEADERS_CRLF,
                std::ptr::null(),
                header_buf.as_mut_ptr() as *mut core::ffi::c_void,
                &mut header_size,
                std::ptr::null_mut(),
            ) != 0
        {
            let text = String::from_utf16_lossy(
                &header_buf[..header_size as usize]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            );
            for line in text.split("\r\n").skip(1) {
                if let Some(colon) = line.find(':') {
                    headers.push((
                        line[..colon].trim().to_string(),
                        line[colon + 1..].trim().to_string(),
                    ));
                }
            }
        }

        let mut body_bytes = Vec::new();
        loop {
            let mut available: u32 = 0;
            if WinHttpQueryDataAvailable(request, &mut available) == 0 || available == 0 {
                break;
            }
            let mut buf = vec![0u8; available as usize];
            let mut read: u32 = 0;
            if WinHttpReadData(
                request,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                available,
                &mut read,
            ) == 0
                || read == 0
            {
                break;
            }
            body_bytes.extend_from_slice(&buf[..read as usize]);
            if body_bytes.len() > 32 * 1024 * 1024 {
                break;
            }
        }

        Ok(HttpResponse {
            status: status as u16,
            headers,
            body: body_bytes,
        })
    }
}

pub fn get(url: &str, timeout_ms: u32) -> Result<HttpResponse, String> {
    request(url, "GET", &[], None, timeout_ms)
}

pub fn get_string(url: &str, timeout_ms: u32) -> Result<String, String> {
    Ok(get(url, timeout_ms)?.text())
}

pub fn get_json(url: &str, timeout_ms: u32) -> Result<serde_json::Value, String> {
    let resp = get(url, timeout_ms)?;
    serde_json::from_slice(&resp.body).map_err(|e| format!("Не удалось разобрать ответ GitHub: {}", e))
}

/// Скачать файл с отчётом о прогрессе (байты скачано, всего; 0 — неизвестно).
pub fn download(
    url: &str,
    target: &Path,
    timeout_ms: u32,
    progress: impl FnMut(u64, u64),
) -> Result<u64, String> {
    download_with_headers(url, target, timeout_ms, &[], progress)
}

/// Скачивание с дополнительными заголовками (нужно для приватных релизов GitHub).
pub fn download_with_headers(
    url: &str,
    target: &Path,
    timeout_ms: u32,
    headers: &[(&str, &str)],
    mut progress: impl FnMut(u64, u64),
) -> Result<u64, String> {
    let (host, path, port, _, secure) = split_url(url).ok_or_else(|| format!("Некорректный адрес: {}", url))?;
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // Файл создаём только после успешного ответа: иначе на любой 404/500
    // остаётся пустой или частичный файл (а старый уже усечён).

    let agent = wide(USER_AGENT);
    unsafe {
        let session = WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err(format!("WinHTTP: не удалось создать сессию ({})", error_text(GetLastError())));
        }
        let mut handles = Handles(vec![session]);
        let t = timeout_ms.min(i32::MAX as u32) as i32;
        WinHttpSetTimeouts(session, t.min(60000), t, t, t);
        let host_w = wide(&host);
        let connect = WinHttpConnect(session, host_w.as_ptr(), port, 0);
        if connect.is_null() {
            return Err(format!("WinHTTP: не удалось подключиться к {}", host));
        }
        handles.0.push(connect);
        let verb = wide("GET");
        let path_w = wide(&path);
        let request = WinHttpOpenRequest(
            connect,
            verb.as_ptr(),
            path_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            if secure { WINHTTP_FLAG_SECURE } else { 0 },
        );
        if request.is_null() {
            return Err("WinHTTP: не удалось создать запрос".to_string());
        }
        handles.0.push(request);
        // ALWAYS (2) разрешала и понижение до http://; нам нужен только
        // кросс-хостовый https-редирект GitHub → objects.githubusercontent.com.
        let redirect = 1u32;
        WinHttpSetOption(
            request,
            WINHTTP_OPTION_REDIRECT_POLICY,
            &redirect as *const u32 as *const core::ffi::c_void,
            4,
        );
        let protocols: u32 = 0x0800 | 0x2000;
        WinHttpSetOption(
            request,
            WINHTTP_OPTION_SECURE_PROTOCOLS,
            &protocols as *const u32 as *const core::ffi::c_void,
            4,
        );
        let mut header_text = String::from("Accept-Encoding: identity\r\n");
        for (k, v) in headers {
            header_text.push_str(k);
            header_text.push_str(": ");
            header_text.push_str(v);
            header_text.push_str("\r\n");
        }
        let headers_w = wide(&header_text);
        if WinHttpSendRequest(request, headers_w.as_ptr(), u32::MAX, std::ptr::null(), 0, 0, 0) == 0 {
            return Err(format!("WinHTTP: запрос не отправлен ({})", error_text(GetLastError())));
        }
        if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
            return Err(format!("WinHTTP: нет ответа ({})", error_text(GetLastError())));
        }
        let mut status: u32 = 0;
        let mut size = 4u32;
        let status_ok = WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut status as *mut u32 as *mut core::ffi::c_void,
            &mut size,
            std::ptr::null_mut(),
        );
        // status == 0 здесь означал бы «код не прочитан», а не успех
        // (раньше сравнение с ERROR_SUCCESS пропускало такой ответ).
        if status_ok == 0 || !(200..300).contains(&status) {
            return Err(format!("Сервер вернул HTTP {}", status));
        }
        let mut file = std::fs::File::create(target).map_err(|e| e.to_string())?;

        // Content-Length
        let mut total: u64 = 0;
        let mut header_size = 0u32;
        if WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_RAW_HEADERS_CRLF,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut header_size,
            std::ptr::null_mut(),
        ) != 0
            && header_size > 0
        {
            let mut buf = vec![0u8; header_size as usize + 4];
            if WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_RAW_HEADERS_CRLF,
                std::ptr::null(),
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                &mut header_size,
                std::ptr::null_mut(),
            ) != 0
            {
                let text = String::from_utf16_lossy(
                    &buf[..header_size as usize]
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect::<Vec<_>>(),
                );
                for line in text.split("\r\n") {
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        total = value.trim().parse().unwrap_or(0);
                    }
                }
            }
        }

        if total > MAX_DOWNLOAD_BYTES {
            return Err(format!(
                "сервер обещает файл больше {} — загрузка отменена",
                crate::util::format_size(MAX_DOWNLOAD_BYTES)
            ));
        }

        let mut written: u64 = 0;
        let mut last_percent = u64::MAX;
        loop {
            let mut available: u32 = 0;
            if WinHttpQueryDataAvailable(request, &mut available) == 0 || available == 0 {
                break;
            }
            let mut buf = vec![0u8; available as usize];
            let mut read: u32 = 0;
            if WinHttpReadData(
                request,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                available,
                &mut read,
            ) == 0
                || read == 0
            {
                break;
            }
            file.write_all(&buf[..read as usize]).map_err(|e| e.to_string())?;
            written += read as u64;
            if written > MAX_DOWNLOAD_BYTES {
                return Err(format!(
                    "загрузка превысила {} — прервано",
                    crate::util::format_size(MAX_DOWNLOAD_BYTES)
                ));
            }
            let percent = if total > 0 { written * 100 / total } else { 0 };
            if percent != last_percent {
                last_percent = percent;
                progress(written, total);
            }
        }
        file.flush().ok();
        Ok(written)
    }
}

/// Проверить, что адрес архива ведёт на GitHub по HTTPS.
pub fn trusted_github_url(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("https://") else {
        return Err(format!("Недоверенный адрес архива: {}", url));
    };
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    // `user@host` в URL — приём обхода проверки хоста; такие адреса не нужны.
    if host_port.contains('@') {
        return Err(format!("Недоверенный адрес архива: {}", url));
    }
    let host = host_port.split(':').next().unwrap_or("");
    let ok = host == "github.com"
        || host.ends_with(".github.com")
        || host == "objects.githubusercontent.com"
        || host == "codeload.github.com";
    if !ok {
        return Err(format!(
            "Архив должен скачиваться с GitHub, а не с {}",
            host
        ));
    }
    Ok(())
}
