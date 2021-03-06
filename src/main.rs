use std::fs::File;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::ops::Add;
use clap::{App, Arg};

const CONNECT_MESSAGE: &'static str = "Connect";
const ACCEPT_RESPONSE: &'static str = "Accept";
const REQUEST_PREFIX: &'static str = "GET:";
const BYE_MESSAGE: &'static str = "BYE";
const BYE_RESPONSE: &'static str = "BYE";
const MAX_BATCH_SIZE: usize = 500;

fn main() {
    let app = App::new("TCP Client for the proxy server")
        .author("Ruben Kostandyan @KoStard")
        .about("Connects to the proxy server, sends the given \
                URL to it and receives the response back, \
                printing to the standard output")
        .arg(Arg::with_name("proxy-server")
            .short("p")
            .long("proxy-server")
            .help("The proxy server address")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("url")
            .long("url")
            .help("The target URL you are trying to read from with the proxy")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("target-file")
            .long("target-file")
            .short("f")
            .help("The target file to write the proxy server response into")
            .takes_value(true)
            .required(true))
        .get_matches();
    let proxy_server_address_raw = app.value_of("proxy-server").expect("Proxy server not provided");
    let url = app.value_of("url").expect("Destination URL not specified");
    let target_file_path = app.value_of("target-file").expect("Target file not specified");

    let proxy_server_address: SocketAddr = proxy_server_address_raw
        .parse()
        .expect("Couldn't parse the proxy address");
    let mut socket = TcpStream::connect(proxy_server_address)
        .expect("Failed to bind to the UDP socket");

    println!("Sending connect");
    send_message(CONNECT_MESSAGE.to_owned(), &mut socket);
    println!("Waiting for acceptance");
    assert_eq!(response_to_string(load_tcp_message(&mut socket)), ACCEPT_RESPONSE);

    let mut file = File::create(target_file_path).expect("Couldn't create the file");
    println!("Sending the URL");
    send_message(generate_request_from_url(url), &mut socket);
    println!("Waiting for response");
    let main_response = load_tcp_message(&mut socket);
    file.write_all(main_response.as_slice()).expect("Couldn't write into the file");

    println!("Sending bye message");
    send_message(BYE_MESSAGE.to_owned(), &mut socket);
    println!("Waiting for bye response");
    assert_eq!(response_to_string(load_tcp_message(&mut socket)), BYE_RESPONSE);
}

fn generate_request_from_url(url: &str) -> String {
    String::from(REQUEST_PREFIX)
        .add(url)
}

fn response_to_string(content: Vec<u8>) -> String {
    String::from_utf8_lossy(content.as_slice()).to_string()
}

fn send_message(message: String, socket: &mut TcpStream) {
    // Maybe we can retry in case of failures
    let mut index = 0;
    let buf = add_headers(message.as_bytes());
    while index < buf.len() {
        let count = socket.write(&buf[index..])
            .expect("Failed sending a message to the proxy");
        index += count;
    }
}

fn add_headers(message: &[u8]) -> Vec<u8> {
    let length = message.len();
    if length > u32::MAX as usize {
        panic!("Maximum allowed length is {}", u32::MAX);
    }
    let length_bytes = (length as u32).to_be_bytes();
    let mut new_message = Vec::new();
    new_message.extend(length_bytes);
    new_message.extend(message);
    return new_message;
}

fn parse_headers(message: Vec<u8>) -> (u32, Vec<u8>) {
    (u32::from_be_bytes([message[0], message[1], message[2], message[3]]),
     message[4..].to_vec())
}

/// Using custom protocol here
/// First 4 bytes should be responsible for showing the length of the request
fn load_tcp_message(stream: &mut TcpStream) -> Vec<u8> {
    println!("Reading TCP message from {:?}", stream);
    let mut overall_message = Vec::new();
    let (overall_length, current_body) = tcp_read_with_headers(stream);
    overall_message.extend(current_body);
    while overall_message.len() < overall_length as usize {
        println!("One Read {} {}", overall_message.len(), overall_length);
        overall_message.extend(one_tcp_read(stream));
    }
    if overall_message.len() > overall_length as usize {
        overall_message[..overall_length as usize].to_vec()
    } else {
        overall_message
    }
}

fn tcp_read_with_headers(stream: &mut TcpStream) -> (u32, Vec<u8>) {
    let mut initial_message = Vec::new();
    while initial_message.len() < 4 {
        initial_message.extend(one_tcp_read(stream));
    }
    parse_headers(initial_message)
}

fn one_tcp_read(stream: &mut TcpStream) -> Vec<u8> {
    // TODO check if will block if not enough message was sent
    let mut buffer = [0; MAX_BATCH_SIZE];
    let count = stream.read(&mut buffer).expect("Failed reading from the stream");
    if count == 0 {
        panic!("Issue with the TCP read, got 0 bytes");
    }
    buffer[..count].to_vec()
}
