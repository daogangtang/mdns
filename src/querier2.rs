use futures::try_ready;
use log::{
    trace,
    warn,
    error
};

use std::{env, io};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use std::collections::VecDeque;

use tokio::prelude::*;
use tokio::net::UdpSocket;
use tokio::timer::Interval;
use tokio::reactor::Handle;

use net2::unix::UnixUdpBuilderExt;

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

struct Querier {
    service_name: String,
    socket: UdpSocket,
    buf: Vec<u8>,
//    responses: VecDeque<Response>,
}

impl Querier {

    fn new(service_name: &str) -> Result<Self, io::Error> {

        /*
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
                    ip
                },
                _ => (),
            }
        }
        */

        // only form mvp
        let iface_addr = Ipv4Addr::new(192, 168, 8, 117); 
        let multicast_addr = Ipv4Addr::new(224, 0, 0, 251); 

        let socket = net2::UdpBuilder::new_v4()?
            .reuse_address(true)?
                        .reuse_port(true)?
                        .bind(("0.0.0.0", 5353))?;
        println!("Listening on: {}", socket.local_addr().unwrap());

        let socket = UdpSocket::from_std(socket, &Handle::default())?;

        socket.set_multicast_loop_v4(true)?;
        socket.set_multicast_ttl_v4(255)?;
        socket.join_multicast_v4(&multicast_addr, &iface_addr);

        Ok(Self {
            service_name: service_name.to_string(),
            socket: socket,
            buf: vec![0; 4096],
            //responses: VecDeque::new(),
        })
    }

    fn build_packet_and_addr(&self, service_name: &'static str) -> (Vec<u8>, SocketAddr) {
        let mut builder = DP_Builder::new_query(0, false);
        builder.add_question(
            &Name::from_str(service_name).unwrap(),
            QueryType::A,
            QueryClass::IN);

        let packet_data = builder.build().unwrap_or_else(|x| x);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 251)), 5353);
        
        (packet_data, addr)
    }

    //fn parse_response(buf: &[u8]) -> Result<Vec<Response>, io::Error> {
    //    
    //
    //}

}


impl Future for Querier {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        loop {

            // receive response from
            let (size, peer) = try_ready!(self.socket.poll_recv_from(&mut self.buf));

            println!("{:?}", &self.buf[..size]);
            // handle packet
            //if  = self.parse_response(&self.buf[..size]) {
            //    println!("{:?}, {}", response, addr);
            //}
        }
    }
}

fn main() {

    let service_name = "_cita.tcp.local";
    let querier = Querier::new(service_name).unwrap();

    // interval send dns packet
    // 
    let interval_task = Interval::new(Instant::now(), Duration::from_secs(5))
        //.take(10)
        .for_each(|instant| {
            println!("fire; instant={:?}", instant);
            // construct dns packet
            let (packet, addr) = querier.build_packet_and_addr(service_name);
            let sent_size = try_ready!(querier.socket.poll_send_to(&packet, &addr));
            println!("Sent {} bytes to {}", sent_size, &addr);

            Ok(())
        })
    .map_err(|e| panic!("interval errored; err={:?}", e));

    let combined_tasks = querier.join(interval_task);

    tokio::run(combined_tasks.map_err(|e| println!("server error = {:?}", e)));

}

