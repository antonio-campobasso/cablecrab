use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use pcap::{Device, Capture, ConnectionStatus, Packet, PacketHeader};
use pktparse::{ethernet, ipv4, tcp, udp, icmp, arp};
use std::net::Ipv4Addr;
use std::string::ToString;
use pktparse::ip::IPProtocol;
use pktparse::ipv4::IPv4Header;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}


//TODO racchiudere le due struct ConnInfo e ConnData in un'altra struct???
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct ConnInfo {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: String,
    pub app_descr: String,
}

impl ConnInfo {
    pub fn new(src: Ipv4Addr, dst: Ipv4Addr, src_port: u16, dst_port: u16, protocol: String, app_descr: String) -> Self {
        ConnInfo {
            src,
            dst,
            src_port,
            dst_port,
            protocol,
            app_descr,
        }
    }
}

impl Display for ConnInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}|{}|{}|{}|{}|{})", self.src, self.dst, self.protocol, self.src_port, self.dst_port, self.app_descr)
    }
}

pub struct ConnData {
    pub ts_first: libc::timeval,
    pub ts_last: libc::timeval,
    pub total_bytes: usize,

}

impl ConnData {
    pub fn new(ts_first: libc::timeval, ts_last: libc::timeval, total_bytes: usize) -> Self {
        ConnData {
            ts_first,
            ts_last,
            total_bytes,
        }
    }
}

impl Display for ConnData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "(tot_bytes:{} ts_first:{}.{:06} ts_last:{}.{:06})", self.total_bytes, self.ts_first.tv_sec, self.ts_first.tv_usec, self.ts_last.tv_sec, self.ts_last.tv_usec)
    }
}


pub trait ToStr {
    fn tostring(&self) -> String;
}

impl ToStr for IPProtocol {
    fn tostring(&self) -> String {
        match self {
            IPProtocol::ICMP => { "ICMP".to_string() }
            IPProtocol::UDP => { "UDP".to_string() }
            IPProtocol::TCP => { "TCP".to_string() }
            _ => { "".to_string() }
        }
    }
}


// list devices
pub fn list_all_devices() -> Vec<Device> {
    let devices = Device::list().unwrap();

    for d in &devices {
        if d.flags.connection_status.eq(&ConnectionStatus::Connected) && d.addresses.len() > 1 {
            println!("{:?}: {:?} - IP Net interface: {:?}", d.name, d.flags.connection_status, d.addresses[1].addr);
        } else if d.flags.connection_status.eq(&ConnectionStatus::Connected) && d.addresses.len() < 2 {
            continue; //Per il Mac bypass llw0 and awdl interface because not chooseable
        } else {
            println!("{:?}: {:?}", d.name, d.flags.connection_status);
        }
    }
    devices
}

//service function to print the hashmap
pub fn print_hashmap(hm: &HashMap<ConnInfo, ConnData>) -> () {
    let mut i = 1;

    for (key, value) in hm {
        println!("{}|{}:{}", i, key, value);
        i += 1;
    }
}

pub fn reporting(report: &mut HashMap<ConnInfo,ConnData>, datagram: IPv4Header, src: u16, dst: u16, packet_header: &PacketHeader, length: usize) -> () {
    let ci = ConnInfo::new(datagram.source_addr, datagram.dest_addr, src, dst, datagram.protocol.tostring(), "".to_string());
    let cd = ConnData::new(packet_header.ts, packet_header.ts, length + 38);
    report.entry(ci)
        .and_modify(|cd| {
            cd.total_bytes += length + 38;
            cd.ts_last = packet_header.ts
        })
        .or_insert(cd);
}

pub fn app_recognition_udp(src: u16, dst: u16) -> () {
    if dst == 53 || src == 53 {
        // println!("DNS message.");
    } else if dst == 161 || src == 161 {
        //println!("SNMP message.");
    }
}

pub fn app_recognition_tcp(src: u16, dst: u16) -> () {
    if dst == 80 || src == 80 {
        //println!("HTTP message.");
    } else if dst == 443 || src == 443 {
        //println!("HTTPS message.");
    } else if dst == 22 || src == 22 {
        //println!("SSH message.");
    }
}

pub fn parsing(report: &mut HashMap<ConnInfo,ConnData>, packet: Packet) -> () {
    if let Ok((payload_e, frame)) = ethernet::parse_ethernet_frame(packet.data) {
        //println!("{}", payload_e.len()); verifica di bytes effettivi trasmessi --> controllo payload del frame e aggiungo 38 (heaeder eth)
        match frame.ethertype {
            ethernet::EtherType::IPv4 => {
                if let Ok((payload_i, datagram)) = ipv4::parse_ipv4_header(payload_e) {
                    match datagram.protocol {
                        IPProtocol::TCP => {
                            if let Ok((_payload_t, segment)) = tcp::parse_tcp_header(payload_i) {
                                reporting(report,datagram, segment.source_port, segment.dest_port, packet.header, payload_e.len());
                                app_recognition_tcp(segment.source_port, segment.dest_port);
                                //println!("{:?}", segment);

                            } else {
                                println!("Error parsing TCP segment.");
                            }
                        }
                        IPProtocol::UDP => {
                            if let Ok((_payload_u, udp_datagram)) = udp::parse_udp_header(payload_i) {
                                reporting(report, datagram, udp_datagram.source_port, udp_datagram.dest_port, packet.header, payload_e.len());
                                app_recognition_udp(udp_datagram.source_port, udp_datagram.dest_port);
                                //println!("{:?}", udp_datagram);
                            } else {
                                println!("Error parsing UDP datagram.");
                            }
                        }
                        IPProtocol::ICMP => { //TODO
                            if let Ok((_payload, _packet)) = icmp::parse_icmp_header(payload_i) {
                                // println!("{:?}", _packet);
                            } else {
                                println!("Error parsing ICMP packet.");
                            }
                        }
                        _ => { println!("L4 protocol not supported") }
                    }
                } else {
                    println!("Error parsing IP datagram.");
                }
            }
            //TODO ethernet::EtherType::IPv6 => {
            // }
            ethernet::EtherType::ARP => { //TODO da capire se inserire o meno nel report
                if let Ok((_payload, _packet)) = arp::parse_arp_pkt(payload_e) {
                    //println!("{:x?}", _packet);
                    print_hashmap(report);
                } else {
                    println!("Error parsing ARP packet.");
                }
            }
            _ => {
                println!("L3 protocol not supported");
            }
        }
    } else {
        println!("Error parsing Ethernet frame.");
    }
}

pub fn start_capture(interface_name: &str, bpf_program: &str) -> () {
    let mut cap = Capture::from_device(interface_name).unwrap()// TODO assume the device exists and we are authorized to open it
        .promisc(true)
        //.snaplen(65535)
        //.buffer_size(65)//serve per vedere subito output quando inviamo pochi dati, altrimenti non vedevo efficacia filtri
        .open().unwrap();//TODO check error in opening and starting a capture

    println!("Sniffing process in promiscuous mode is active on interface: {}", interface_name);
    cap.filter(bpf_program, true).unwrap();

    //TODO lasciare la dichiarazione dell'HashMap report dentro start_capture?
    let mut report: HashMap<ConnInfo, ConnData> = HashMap::new();

    while let Ok(packet) = cap.next_packet() { //TODO fare controllo sul next packet
        parsing(&mut report, packet);

        //fare il ciclo while all'interno di un'altra funzione????
    }
    print_hashmap(&report);
}
