//! SSDP: so findet Home Assistant den Rahmen von selbst (E-47).
//!
//! Zwei Richtungen, ein UDP-Socket auf Port 1900 in der Multicast-Gruppe
//! 239.255.255.250:
//!
//! - **Antworten.** Ein Steuerpunkt fragt per `M-SEARCH` nach einem Zieltyp
//!   (`ST`). Passt er zu uns, antworten wir ihm direkt mit unserer `LOCATION`.
//! - **Ankuendigen.** Alle paar Minuten schicken wir `ssdp:alive` in die Gruppe,
//!   beim Beenden `ssdp:byebye`. So erscheint der Rahmen auch, wenn niemand
//!   fragt — und verschwindet sauber statt als Leiche in der Geraeteliste.
//!
//! Jede Nachricht traegt `BOOTID.UPNP.ORG` (E-57): eine Kennung des
//! Prozessstarts. Daran erkennt Home Assistant einen Neustart und abonniert
//! seine Ereignisse neu — die alten Abonnements leben nur im Speicher.
//!
//! Die `LOCATION` braucht unsere eigene Adresse. Die kennt niemand besser als
//! der Kernel: ein UDP-Socket, „verbunden" mit dem Anfragenden, verraet mit
//! `local_addr` die Schnittstelle, ueber die er erreichbar ist. Das funktioniert
//! ohne Schnittstellenliste und auf Android ohne weitere Rechte.
//!
//! Was Android trotzdem braucht: einen `MulticastLock`, sonst verwirft das
//! WLAN eintreffende Multicast-Pakete — und `M-SEARCH` ist Multicast. Den haelt
//! `mod.rs` ueber die Java-Bruecke.

use super::description::{Service, DEVICE_TYPE};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::sync::watch;

pub const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
pub const PORT: u16 = 1900;
/// Wie lange ein Steuerpunkt unsere Ankuendigung fuer gueltig halten darf.
pub const MAX_AGE_SECS: u32 = 1800;
/// Ankuendigen deutlich vor Ablauf — der Standard rät zur halben Frist.
pub const ALIVE_INTERVAL: Duration = Duration::from_secs(MAX_AGE_SECS as u64 / 2);

pub fn multicast_target() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(MULTICAST_ADDR, PORT))
}

/// Alle Zieltypen, unter denen der Rahmen zu finden ist.
pub fn targets(udn: &str) -> Vec<String> {
    let mut t = vec!["upnp:rootdevice".to_string(), udn.to_string(), DEVICE_TYPE.to_string()];
    t.extend(Service::ALL.iter().map(|s| s.service_type().to_string()));
    t
}

/// Der `USN` zu einem Zieltyp: die UDN, bei allem ausser ihr selbst mit dem
/// Typ dahinter.
pub fn usn(udn: &str, st: &str) -> String {
    if st == udn {
        udn.to_string()
    } else {
        format!("{udn}::{st}")
    }
}

/// Der `ST`-Wert einer `M-SEARCH`-Anfrage, falls es eine ist.
pub fn parse_msearch(text: &str) -> Option<String> {
    let mut lines = text.lines();
    let first = lines.next()?.trim();
    if !first.starts_with("M-SEARCH") {
        return None;
    }
    let mut st = None;
    let mut discover = false;
    for line in lines {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim().to_ascii_uppercase().as_str() {
            "ST" => st = Some(value.trim().to_string()),
            "MAN" => discover = value.contains("ssdp:discover"),
            _ => {}
        }
    }
    if discover { st } else { None }
}

/// Auf welche unserer Zieltypen eine Suche nach `st` passt.
///
/// `ssdp:all` heisst alle; sonst genau der eine, wenn er unserer ist.
pub fn matching_targets(st: &str, udn: &str) -> Vec<String> {
    let all = targets(udn);
    if st == "ssdp:all" {
        all
    } else if all.iter().any(|t| t == st) {
        vec![st.to_string()]
    } else {
        Vec::new()
    }
}

/// Kennung dieses Prozessstarts (`BOOTID.UPNP.ORG`, UPnP 1.1, E-57).
///
/// Ein Steuerpunkt erkennt am geaenderten Wert, dass das Geraet neu gestartet
/// ist — und damit, dass seine Ereignis-Abonnements dort nicht mehr existieren.
/// Home Assistant verbindet sich dann neu und abonniert frisch. Ohne diese
/// Kennung hielt es nach jedem Neustart des Rahmens bis zu 30 Minuten an
/// einem toten Abonnement fest, fragte nicht ab und zeigte nichts Neues.
///
/// Die Sekunden seit 1970 beim ersten Aufruf: pro Start ein neuer, immer
/// groesserer Wert, wie der Standard es verlangt (31 Bit reichen bis 2038).
pub fn boot_id() -> u32 {
    use std::sync::OnceLock;
    static BOOT_ID: OnceLock<u32> = OnceLock::new();
    *BOOT_ID.get_or_init(|| {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(1);
        (secs.min(i32::MAX as u64) as u32).max(1)
    })
}

/// Kennung der Geraetebeschreibung (`CONFIGID.UPNP.ORG`). Aendert sich nur,
/// wenn Beschreibung oder Dienste sich aendern — bislang nie.
pub const CONFIG_ID: u32 = 1;

/// Die beiden UPnP-1.1-Kopfzeilen, die in jeder SSDP-Nachricht stehen.
fn upnp11_headers() -> String {
    format!(
        "BOOTID.UPNP.ORG: {boot}\r\nCONFIGID.UPNP.ORG: {cfg}\r\n",
        boot = boot_id(),
        cfg = CONFIG_ID
    )
}

/// Antwort auf eine passende Suche.
pub fn response(st: &str, udn: &str, location: &str, server: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age={max_age}\r\nEXT:\r\nLOCATION: {location}\r\nSERVER: {server}\r\nST: {st}\r\nUSN: {usn}\r\n{upnp11}\r\n",
        max_age = MAX_AGE_SECS,
        usn = usn(udn, st),
        upnp11 = upnp11_headers(),
    )
}

/// Ankuendigung in die Gruppe.
pub fn alive(nt: &str, udn: &str, location: &str, server: &str) -> String {
    format!(
        "NOTIFY * HTTP/1.1\r\nHOST: {host}\r\nCACHE-CONTROL: max-age={max_age}\r\nLOCATION: {location}\r\nNT: {nt}\r\nNTS: ssdp:alive\r\nSERVER: {server}\r\nUSN: {usn}\r\n{upnp11}\r\n",
        host = multicast_target(),
        max_age = MAX_AGE_SECS,
        usn = usn(udn, nt),
        upnp11 = upnp11_headers(),
    )
}

/// Abmeldung beim Beenden.
pub fn byebye(nt: &str, udn: &str) -> String {
    format!(
        "NOTIFY * HTTP/1.1\r\nHOST: {host}\r\nNT: {nt}\r\nNTS: ssdp:byebye\r\nUSN: {usn}\r\n{upnp11}\r\n",
        host = multicast_target(),
        usn = usn(udn, nt),
        upnp11 = upnp11_headers(),
    )
}

/// Unsere Adresse aus Sicht von `peer` — die Schnittstelle, ueber die der
/// Kernel ihn erreichen wuerde.
pub fn local_ip_toward(peer: IpAddr) -> Option<IpAddr> {
    let probe = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    probe.connect(SocketAddr::new(peer, PORT)).ok()?;
    let ip = probe.local_addr().ok()?.ip();
    if ip.is_unspecified() { None } else { Some(ip) }
}

pub fn location(ip: IpAddr, http_port: u16) -> String {
    format!("http://{ip}:{http_port}/upnp/device.xml")
}

fn open_socket() -> std::io::Result<tokio::net::UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    // Port 1900 teilen sich alle UPnP-Programme auf dem Geraet; ohne
    // Wiederverwendung scheiterte das Binden, sobald ein zweites laeuft.
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddr::from(([0, 0, 0, 0], PORT)).into())?;
    socket.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)?;
    socket.set_multicast_ttl_v4(2)?;
    tokio::net::UdpSocket::from_std(socket.into())
}

/// Der SSDP-Dienst: antwortet auf Suchen und kuendigt sich an, bis `stop`
/// kippt — dann meldet er sich ab.
pub async fn run(udn: String, http_port: u16, server: String, mut stop: watch::Receiver<bool>) {
    let socket = match open_socket() {
        Ok(s) => s,
        Err(e) => {
            log::error!("UPnP: SSDP-Socket auf Port {PORT} nicht zu oeffnen: {e}");
            return;
        }
    };
    log::info!("UPnP: SSDP lauscht auf {}", multicast_target());

    let mut buf = vec![0u8; 2048];
    let mut ticker = tokio::time::interval(ALIVE_INTERVAL);
    let group = multicast_target();

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Der erste Tick feuert sofort: die Ankuendigung beim Start.
                if let Some(ip) = local_ip_toward(IpAddr::V4(MULTICAST_ADDR)) {
                    let loc = location(ip, http_port);
                    for nt in targets(&udn) {
                        let _ = socket.send_to(alive(&nt, &udn, &loc, &server).as_bytes(), group).await;
                    }
                }
            }
            received = socket.recv_from(&mut buf) => {
                let Ok((n, peer)) = received else { continue };
                let text = String::from_utf8_lossy(&buf[..n]);
                let Some(st) = parse_msearch(&text) else { continue };
                let hits = matching_targets(&st, &udn);
                if hits.is_empty() {
                    continue;
                }
                let Some(ip) = local_ip_toward(peer.ip()) else { continue };
                let loc = location(ip, http_port);
                for target in hits {
                    let _ = socket.send_to(response(&target, &udn, &loc, &server).as_bytes(), peer).await;
                }
            }
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    for nt in targets(&udn) {
                        let _ = socket.send_to(byebye(&nt, &udn).as_bytes(), group).await;
                    }
                    log::info!("UPnP: SSDP abgemeldet");
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UDN: &str = "uuid:12345678-1234-4123-8123-123456789abc";

    #[test]
    fn erkennt_eine_suche_und_liest_das_ziel() {
        let msg = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 3\r\nST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n";
        assert_eq!(parse_msearch(msg).as_deref(), Some(DEVICE_TYPE));
        // Kleinschreibung der Kopfzeilen ist erlaubt.
        let klein = msg.replace("ST:", "st:").replace("MAN:", "man:");
        assert_eq!(parse_msearch(&klein).as_deref(), Some(DEVICE_TYPE));
    }

    #[test]
    fn ignoriert_alles_was_keine_suche_ist() {
        assert_eq!(parse_msearch("NOTIFY * HTTP/1.1\r\nNTS: ssdp:alive\r\n\r\n"), None);
        assert_eq!(parse_msearch("HTTP/1.1 200 OK\r\nST: x\r\n\r\n"), None);
        // Ohne "ssdp:discover" ist es keine Suche.
        assert_eq!(parse_msearch("M-SEARCH * HTTP/1.1\r\nST: ssdp:all\r\n\r\n"), None);
    }

    #[test]
    fn beantwortet_genau_die_passenden_ziele() {
        assert_eq!(matching_targets("ssdp:all", UDN).len(), 6);
        assert_eq!(matching_targets("upnp:rootdevice", UDN), vec!["upnp:rootdevice"]);
        assert_eq!(matching_targets(DEVICE_TYPE, UDN), vec![DEVICE_TYPE]);
        assert_eq!(matching_targets(UDN, UDN), vec![UDN]);
        // Home Assistant sucht genau danach (manifest.json der dlna_dmr).
        assert!(!matching_targets("urn:schemas-upnp-org:device:MediaRenderer:1", UDN).is_empty());
        assert!(matching_targets("urn:schemas-upnp-org:device:MediaServer:1", UDN).is_empty());
        assert!(matching_targets("roku:ecp", UDN).is_empty());
    }

    #[test]
    fn usn_haengt_den_typ_an_ausser_bei_der_udn_selbst() {
        assert_eq!(usn(UDN, UDN), UDN);
        assert_eq!(
            usn(UDN, "upnp:rootdevice"),
            format!("{UDN}::upnp:rootdevice")
        );
    }

    #[test]
    fn antwort_und_ankuendigung_tragen_die_pflichtfelder() {
        let r = response(DEVICE_TYPE, UDN, "http://10.0.0.5:8128/upnp/device.xml", "Test/1.0");
        assert!(r.starts_with("HTTP/1.1 200 OK\r\n"));
        for key in ["CACHE-CONTROL: max-age=1800", "EXT:", "LOCATION: http://10.0.0.5:8128/upnp/device.xml", "SERVER: Test/1.0", &format!("ST: {DEVICE_TYPE}"), &format!("USN: {UDN}::{DEVICE_TYPE}")] {
            assert!(r.contains(key), "{key}");
        }
        assert!(r.ends_with("\r\n\r\n"));

        let a = alive("upnp:rootdevice", UDN, "http://10.0.0.5:8128/upnp/device.xml", "Test/1.0");
        assert!(a.starts_with("NOTIFY * HTTP/1.1\r\n"));
        assert!(a.contains("HOST: 239.255.255.250:1900"));
        assert!(a.contains("NTS: ssdp:alive"));
        assert!(a.contains("NT: upnp:rootdevice"));

        let b = byebye("upnp:rootdevice", UDN);
        assert!(b.contains("NTS: ssdp:byebye"));
        assert!(!b.contains("LOCATION"));
    }

    #[test]
    fn jede_nachricht_traegt_bootid_und_configid_e_57() {
        // UPnP 1.1: an der BOOTID erkennt Home Assistant den Neustart und
        // abonniert neu. Fehlte sie, hing HA bis zu 30 Minuten an einem
        // toten Abonnement.
        let boot = format!("BOOTID.UPNP.ORG: {}\r\n", boot_id());
        let cfg = format!("CONFIGID.UPNP.ORG: {CONFIG_ID}\r\n");
        for msg in [
            response(DEVICE_TYPE, UDN, "http://10.0.0.5:8128/upnp/device.xml", "Test/1.0"),
            alive("upnp:rootdevice", UDN, "http://10.0.0.5:8128/upnp/device.xml", "Test/1.0"),
            byebye("upnp:rootdevice", UDN),
        ] {
            assert!(msg.contains(&boot), "BOOTID fehlt in:\n{msg}");
            assert!(msg.contains(&cfg), "CONFIGID fehlt in:\n{msg}");
            // Kopfzeilen enden mit der Leerzeile, nichts haengt dahinter.
            assert!(msg.ends_with("\r\n\r\n"), "Ende falsch in:\n{msg}");
            assert!(!msg.contains("\r\n\r\n\r\n"), "doppelte Leerzeile in:\n{msg}");
        }
    }

    #[test]
    fn bootid_ist_je_prozess_fest_und_plausibel_e_57() {
        let a = boot_id();
        let b = boot_id();
        assert_eq!(a, b, "innerhalb eines Starts darf sich die BOOTID nicht aendern");
        // Sekunden seit 1970, also irgendwann nach 2020 und innerhalb 31 Bit.
        assert!(a > 1_577_836_800, "war: {a}");
        assert!(a <= i32::MAX as u32);
    }

    #[test]
    fn location_zeigt_auf_die_geraetebeschreibung() {
        assert_eq!(
            location("192.168.1.20".parse().unwrap(), 8128),
            "http://192.168.1.20:8128/upnp/device.xml"
        );
    }

    #[test]
    fn eigene_adresse_ist_keine_platzhalteradresse() {
        // Gegen einen Rechner im eigenen Netz muss eine konkrete Adresse
        // herauskommen; ohne Netz darf es None sein, nie 0.0.0.0.
        if let Some(ip) = local_ip_toward("192.0.2.1".parse().unwrap()) {
            assert!(!ip.is_unspecified());
        }
    }
}
