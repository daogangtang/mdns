use futures::try_ready;
use log::{
    trace,
    warn,
    error
};

use std::{env, io};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::prelude::*;
use tokio::net::UdpSocket;

use get_if_addrs::get_if_addrs;
use dns_parser::{
    Packet,
    Builder as DP_Builder,
    Question,
    Answers,
    Name, 
    QueryClass, 
    QueryType, 
    RRData
};

pub type AnswerBuilder = DP_Builder<Answers>;

const DEFAULT_TTL: u32 = 255;

struct Responder {
    socket: UdpSocket,
    buf: Vec<u8>,
    received_from: Option<(usize, SocketAddr)>,
}

impl Responder {
    fn handle_packet(&self, buf: &[u8], addr: SocketAddr) -> Option<(Vec<u8>, SocketAddr)> {
        let packet = match Packet::parse(buf) {
            Ok(packet) => packet,
            Err(error) => {
                warn!("couldn't parse packet from {:?}: {}", addr, error);
                return None;
            }
        };

        if !packet.header.query {
            trace!("received packet from {:?} with no query", addr);
            return None;
        }

        if packet.header.truncated {
            warn!("dropping truncated packet from {:?}", addr);
            return None;
        }

        let mut multicast_builder = DP_Builder::new_response(packet.header.id, false)
            .move_to::<Answers>();
        multicast_builder.set_max_size(None);

        for question in packet.questions {
            if question.qclass == QueryClass::IN || question.qclass == QueryClass::Any {
                multicast_builder = self.handle_question(&question, multicast_builder);
            }
        }

        if !multicast_builder.is_empty() {
            let response = multicast_builder.build().unwrap_or_else(|x| x);
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 251)), 5353);

            // send it to multicase address
            // TODO: according to rfc, we should wait random 20ms~120ms here before sending
            //let sent_size = try_ready!(self.socket.poll_send_to(&response, &addr));
            //println!("Sent {} bytes to {}", sent_size, &addr);

            return Some((response, addr))
        }

        None
    }

    fn handle_question (&self, question: &Question, mut builder: AnswerBuilder)
        -> AnswerBuilder {
       
        match question.qtype {
            QueryType::A => {
                //builder = self.add_ip_rr(get_hostname(), builder, DEFAULT_TTL);
                builder = self.add_ip_rr(&Name::from_str("node xxx").unwrap(), builder, DEFAULT_TTL);
            },
            _ => (),
        }

        builder
    }

    
    fn add_ip_rr(&self, hostname: &Name, mut builder: AnswerBuilder, ttl: u32) -> AnswerBuilder {
        let interfaces = match get_if_addrs() {
            Ok(ifcs) => ifcs,
            Err(err) => {
                error!("could not get list of interfaces: {}", err);
                return builder;
            }
        };

        for iface in interfaces {
            if iface.is_loopback() {
                continue;
            }

            match iface.ip() {
                IpAddr::V4(ip) => {
                    builder = builder.add_answer(hostname, QueryClass::IN, ttl, &RRData::A(ip));
                },
                _ => (),
            }
        }

        builder
    }
        

}


impl Future for Responder {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {

        loop {
            self.received_from = Some(try_ready!(self.socket.poll_recv_from(&mut self.buf)));

            if let Some((size, peer)) = self.received_from {
                println!("{:?}", &self.buf[..size]);
                // handle packet
                if let Some((response, addr)) = self.handle_packet(&self.buf[..size], peer) {
                    println!("{:?}, {}", response, addr);
                    
                    // send it to multicase address
                    // TODO: according to rfc, we should wait random 20ms~120ms here before sending
                    let sent_size = try_ready!(self.socket.poll_send_to(&response, &addr));
                    println!("Sent {} bytes to {}", sent_size, &addr);

                }
            }
        }
    }
}


fn main() {
    // create local binding 
    // XXX: should reuse local address and port
    //      and add to multicase group
    let addr = env::args().nth(1).unwrap_or("0.0.0.0:5353".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let socket = UdpSocket::bind(&addr).unwrap();
    println!("Listening on: {}", socket.local_addr().unwrap());

    let responder = Responder {
        socket: socket,
        buf: vec![0; 4096],
        received_from: None,
    };

    tokio::run(responder.map_err(|e| println!("server error = {:?}", e)));
}

