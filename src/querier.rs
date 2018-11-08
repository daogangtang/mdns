use futures::try_ready;
use log::{
    trace,
    warn,
    error
};

use std::{env, io};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use std::collections::HashMap;

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

use std::str;

pub type AnswerBuilder = DP_Builder<Answers>;

const DEFAULT_TTL: u32 = 255;

struct Querier {
    service_name: String,
    socket: UdpSocket,
}

struct Respond {
    socket: UdpSocket,
    buf: Vec<u8>,
    ips: HashMap<IpAddr, usize>,
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

        // XXX: using Handle::default()?
        let socket = UdpSocket::from_std(socket, &Handle::default())?;

        socket.set_multicast_loop_v4(true)?;
        socket.set_multicast_ttl_v4(255)?;
        socket.join_multicast_v4(&multicast_addr, &iface_addr)?;

        Ok(Self {
            service_name: service_name.to_string(),
            socket: socket,
            //responses: VecDeque::new(),
        })
    }

    fn build_packet_and_multicast_addr(&self, service_name: String) -> (Vec<u8>, SocketAddr) {
        let builder = DP_Builder::new_query(0, false);
        let builder = builder.add_question(
            &Name::from_str(service_name).unwrap(),
            QueryType::A,
            QueryClass::IN);

        let packet_data = builder.build().unwrap_or_else(|x| x);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 251)), 5353);
        
        (packet_data, addr)
    }

    fn send(&mut self, packet: &[u8], addr: &SocketAddr) -> Result<Async<usize>, io::Error> {
        self.socket.poll_send_to(&packet, &addr)
    }

    fn query(&mut self) -> Result<(), io::Error>{

        // construct dns packet
        let (packet, addr) = self.build_packet_and_multicast_addr(self.service_name.clone());
        //let sent_size = try_ready!(querier.socket.poll_send_to(&packet, &addr));
        let sent_size = match self.send(&packet, &addr)? {
            Async::Ready(size) => size,
            Async::NotReady => return Ok(())
        };
        println!("Sent {} bytes to {}", sent_size, &addr);
        
        Ok(())
    }

}


impl Respond {
    
    pub fn new() -> Result<Self, io::Error> {
        // bind local address and port
        // reuse them
        let iface_addr = Ipv4Addr::new(192, 168, 8, 117); 
        let multicast_addr = Ipv4Addr::new(224, 0, 0, 251); 

        let socket = net2::UdpBuilder::new_v4()?
            .reuse_address(true)?
                        .reuse_port(true)?
                        .bind(("0.0.0.0", 5353))?;
        println!("Listening on: {}", socket.local_addr().unwrap());

        // XXX: using Handle::default()?
        let socket = UdpSocket::from_std(socket, &Handle::default())?;

        socket.set_multicast_loop_v4(true)?;
        socket.set_multicast_ttl_v4(255)?;
        socket.join_multicast_v4(&multicast_addr, &iface_addr)?;

        Ok(Self {
            socket: socket,
            buf: vec![0; 4096],
            ips: HashMap::new(),
        })


    }

    /*
    fn parse_response(&self, buf: &[u8]) -> Result<Packet, io::Error> {
        let raw_packet = match Packet::parse(&buf) {
            Ok(raw_packet) => raw_packet,
            Err(e) => {
                return Err(io::Error::new(io::ErrorKind::Other, "parsing packet error"))
            }
            
        };

        Ok(raw_packet)
    }
    */


}

impl Future for Respond {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        loop {

            // receive response from
            let (size, peer) = try_ready!(self.socket.poll_recv_from(&mut self.buf));

            let raw_packet: Option<Packet> = match Packet::parse(&self.buf[..size]) {
                Ok(raw_packet) => Some(raw_packet),
                Err(e) => {
                    //return Err(io::Error::new(io::ErrorKind::Other, "parsing packet error"))
                    None
                }
            };
            println!("{:?}, {}", raw_packet, peer);

            self.ips.entry(peer.ip()).or_insert(1);

            println!("{:?}", self.ips);
            // handle packet
            //if  = self.parse_response(&self.buf[..size]) {
            //    println!("{:?}, {}", response, addr);
            //}
        }
    }
}

fn main() {

    let service_name = "_cita.tcp.local";
    let mut querier = Querier::new(service_name).unwrap();

    // interval send dns packet
    // 
    let query_task = Interval::new(Instant::now(), Duration::from_secs(5))
        .for_each(move |instant| {
            println!("fire; instant={:?}", instant);
            querier.query().unwrap();

            Ok(())
        })
        .map_err(|e| panic!("interval errored; err={:?}", e));

    // respond polling tasks
    let respond_task = Respond::new().unwrap();

    // combine them with join
    let tasks = respond_task.join(query_task).and_then(|_|{ Ok(()) });

    // start event loop
    tokio::run(tasks.map_err(|e| println!("server error = {:?}", e)));
}

