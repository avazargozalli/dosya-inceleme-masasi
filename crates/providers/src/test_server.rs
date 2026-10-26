//! Testler için **gerçek** TCP HTTP sunucusu — tek uygulama.
//!
//! Neden ortak: `api.rs` ve `local.rs` testleri aynı tuzağı ayrı ayrı yaşadı.
//! Tek `read` ile isteği okuyup hemen yanıtlayan sunucu, istemci gövdeyi ikinci
//! bir TCP parçasında gönderdiğinde soketi kapatıyor; kapanan soket istemcinin
//! kalan yazımına **RST** ile karşılık verip yanıtı düşürebiliyordu. Belirti
//! macOS CI'da `Transport`, ubuntu'da yeşil (defter: sağlayıcı test sunucusu).
//!
//! Kural: istek **tamamen** okunur (başlıklar + `Content-Length` kadar gövde),
//! yanıttan sonra yalnız yazma yönü kapatılır ve okuma yönü boşaltılır. Böylece
//! istemci yanıtı aldıktan sonra temiz bir EOF görür.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// İsteği **tamamen** okur ve okunan bayt sayısını döner (testler için).
///
/// Zaman aşımı 5 sn, üst sınır 64 KiB: yarım kalan bir bağlantı testi
/// kilitlemesin.
pub(crate) fn read_request(stream: &mut std::net::TcpStream) -> usize {
    use std::time::Duration;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut received: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 4096];
    let header_end = loop {
        match stream.read(&mut buffer) {
            Ok(0) => return received.len(),
            Ok(n) => {
                received.extend_from_slice(&buffer[..n]);
                if let Some(pos) = received.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
                if received.len() > 64 * 1024 {
                    return received.len();
                }
            }
            Err(_) => return received.len(),
        }
    };

    // `Content-Length` (varsa) kadar gövdeyi de oku; kalan bayt bırakma.
    let headers = String::from_utf8_lossy(&received[..header_end]).to_ascii_lowercase();
    let declared: usize = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut remaining = declared.saturating_sub(received.len() - header_end);
    let mut body_read = 0usize;
    while remaining > 0 {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                body_read += n;
                remaining = remaining.saturating_sub(n);
            }
            Err(_) => break,
        }
    }
    received.len() + body_read
}

/// Tek kullanımlık sunucu: `status` + JSON gövde döner, taban URL verir.
///
/// `Content-Length` gövdeden hesaplanır — elle yazılan uzunluk testi bozuyordu.
pub(crate) fn serve(status: &str, body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("dinle");
    let address = listener.local_addr().expect("adres");
    let status = status.to_string();
    let body = body.to_string();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            read_request(&mut stream);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
            // Yazma yönünü kapat, okuma yönünü boşalt: istemci yanıtı aldıktan
            // sonra kapanır, biz de EOF görürüz. Soketi hemen düşürmek RST
            // üretip yanıtı kaybettirebiliyordu.
            let _ = stream.shutdown(std::net::Shutdown::Write);
            let mut sink = [0u8; 1024];
            while let Ok(n) = stream.read(&mut sink) {
                if n == 0 {
                    break;
                }
            }
        }
    });
    format!("http://{address}")
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    use super::read_request;

    /// Kusurun kendisi: gövde **iki TCP parçası** hâlinde gelirse isteğin
    /// tamamı okunmalı. Tek `read`'lik sunucu ilk parçada durur ve ikinci
    /// yazım RST ile düşer; bu test onu deterministik yakalar.
    #[test]
    fn split_request_body_is_read_entirely() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("dinle");
        let address = listener.local_addr().expect("adres");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("kabul");
            let read = read_request(&mut stream);
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
            let _ = stream.flush();
            read
        });

        let mut client = TcpStream::connect(address).expect("bağlan");
        let body = "0123456789";
        let head = format!(
            "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        client.write_all(head.as_bytes()).expect("başlık");
        client.write_all(&body.as_bytes()[..4]).expect("gövde 1");
        client.flush().expect("akıt");
        thread::sleep(Duration::from_millis(50));
        client.write_all(&body.as_bytes()[4..]).expect("gövde 2");

        let mut response = String::new();
        client.read_to_string(&mut response).expect("yanıt");
        assert!(response.starts_with("HTTP/1.1 200"), "yanıt: {response}");
        assert_eq!(
            server.join().expect("sunucu"),
            head.len() + body.len(),
            "istek gövdesi tamamen okunmalı"
        );
    }
}
