use std::future::Future;
use std::{
    convert::{TryFrom, TryInto},
    error::Error,
    io::prelude::*,
    net::{TcpListener, TcpStream},
};

pub mod prelude {
    //! Re-exports important traits and types. Meant to be glob imported when using `iroha_network`.

    #[doc(inline)]
    pub use crate::{AsyncStream, Request, Response, Stream};
}

pub const BUFFER_SIZE: usize = 2048;

pub struct Network {
    server_url: String,
}

impl Network {
    /// Creates a new client that will send request to the server on `server_url`
    /// # Arguments
    ///
    /// * `server_url` - is of format ip:port
    ///
    /// # Examples
    /// ```
    /// use iroha_network::Network;
    ///
    /// //If server runs on port 7878 on localhost
    /// let client = Network::new("127.0.0.1:7878");
    /// ```
    pub fn new(server_url: &str) -> Network {
        Network {
            server_url: server_url.to_string(),
        }
    }

    /// Establishes connection to server on `self.server_url`, sends `request` closes connection and returns `Response`.
    pub fn send_request(&self, request: Request) -> Result<Response, Box<dyn Error>> {
        let mut stream = TcpStream::connect(self.server_url.clone())?;
        Self::send_over_stream(&mut stream, request)
    }

    /// Establishes connection to server on `server_url`, sends `request` closes connection and returns `Response`.
    pub fn send_request_to(server_url: &str, request: Request) -> Result<Response, Box<dyn Error>> {
        let mut stream = TcpStream::connect(server_url)?;
        Self::send_over_stream(&mut stream, request)
    }

    /// Listens on the specified `server_url`.
    /// When there is an incoming connection, it passes it's `Stream` to `handler`.
    /// # Arguments
    ///
    /// * `server_url` - url of format ip:port (e.g. `127.0.0.1:7878`) on which this server will listen for incoming connections.
    /// * `handler` - callback function which is called when there is an incoming connection, it get's the stream for this connection
    pub fn listen<H>(server_url: &str, mut handler: H) -> Result<(), Box<dyn Error>>
    where
        H: FnMut(Box<dyn Stream>) -> Result<(), Box<dyn Error>>,
    {
        let listener = TcpListener::bind(server_url)?;
        while let Some(stream) = listener.incoming().next() {
            handler(Box::new(stream?))?;
        }
        Ok(())
    }

    /// Same as `listen` but asynchronous with the use of `async_std`
    ///
    /// Listens on the specified `server_url`.
    /// When there is an incoming connection, it passes it's `AsyncStream` to `handler`.
    /// # Arguments
    ///
    /// * `server_url` - url of format ip:port (e.g. `127.0.0.1:7878`) on which this server will listen for incoming connections.
    /// * `handler` - callback function which is called when there is an incoming connection, it get's the stream for this connection
    pub async fn listen_async<H, F>(server_url: &str, mut handler: H) -> Result<(), String>
    where
        H: FnMut(Box<dyn AsyncStream>) -> F,
        F: Future<Output = Result<(), String>>,
    {
        use async_std::{net::TcpListener, prelude::*};

        let listener = TcpListener::bind(server_url)
            .await
            .map_err(|e| e.to_string())?;
        let mut incoming = listener.incoming();

        while let Some(stream) = incoming.next().await {
            handler(Box::new(stream.map_err(|e| e.to_string())?)).await?;
        }
        Ok(())
    }

    /// Helper function to call inside `listen` `handler` function to parse and send response.
    /// The `handler` specified here will need to generate `Response` from `Request`.
    pub fn handle_message<H>(stream: &mut impl Stream, mut handler: H) -> Result<(), Box<dyn Error>>
    where
        H: FnMut(Request) -> Result<Response, Box<dyn Error>>,
    {
        let mut buffer = [0u8; BUFFER_SIZE];
        let read_size = stream.read(&mut buffer).expect("Request read failed.");
        let response = handler(
            buffer[..read_size]
                .to_vec()
                .try_into()
                .map_err(|_| "Failed to parse message.")?,
        )?;
        stream.write_all(&response)?;
        stream.flush()?;
        Ok(())
    }

    /// Helper function to call inside `listen_async` `handler` function to parse and send response.
    /// The `handler` specified here will need to generate `Response` from `Request`.
    pub async fn handle_message_async<H, F>(
        mut stream: Box<dyn AsyncStream>,
        mut handler: H,
    ) -> Result<(), String>
    where
        H: FnMut(Request) -> F,
        F: Future<Output = Result<Response, String>>,
    {
        use async_std::prelude::*;

        let mut buffer = [0u8; BUFFER_SIZE];
        let read_size = stream
            .read(&mut buffer)
            .await
            .expect("Request read failed.");
        let bytes: Vec<u8> = buffer[..read_size].to_vec();
        let request: Request = bytes
            .try_into()
            .map_err(|e: Box<dyn Error>| e.to_string())?;
        let response = handler(request).await?;
        stream
            .write_all(&response)
            .await
            .map_err(|e| e.to_string())?;
        stream.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Function for internal use to send messages over TcpStream.
    fn send_over_stream(
        stream: &mut TcpStream,
        request: Request,
    ) -> Result<Response, Box<dyn Error>> {
        let payload: Vec<u8> = request.into();
        stream.write_all(&payload)?;
        stream.flush()?;
        let mut buffer = [0u8; BUFFER_SIZE];
        let read_size = stream.read(&mut buffer)?;
        Ok(buffer[..read_size].to_vec())
    }
}

pub trait Stream: Read + Write + Send {}
impl<T> Stream for T where T: Read + Write + Send {}

pub trait AsyncStream: async_std::io::Read + async_std::io::Write + Send + Unpin {}
impl<T> AsyncStream for T where T: async_std::io::Read + async_std::io::Write + Send + Unpin {}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Request {
    uri_path: String,
    payload: Vec<u8>,
}

impl Request {
    /// Creates a new request
    ///
    /// # Arguments
    ///
    /// * uri_path - corresponds to [URI syntax](https://en.wikipedia.org/wiki/Uniform_Resource_Identifier)
    /// `path` part (e.g. "/commands")
    /// * payload - the message in bytes
    ///
    /// # Examples
    /// ```
    /// use iroha_network::prelude::*;
    ///
    /// let request = Request::new("/commands".to_string(), "some_message".to_string().into_bytes());
    /// ```
    pub fn new(uri_path: String, payload: Vec<u8>) -> Request {
        Request { uri_path, payload }
    }

    pub fn url(&self) -> &str {
        &self.uri_path[..]
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..]
    }
}

impl From<Request> for Vec<u8> {
    fn from(request: Request) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(request.uri_path.as_bytes());
        bytes.extend_from_slice(b"\r\n");
        bytes.extend(request.payload.into_iter());
        bytes
    }
}

impl TryFrom<Vec<u8>> for Request {
    type Error = Box<dyn Error>;

    fn try_from(bytes: Vec<u8>) -> Result<Request, Box<dyn Error>> {
        let mut split_iter = bytes.split(|byte| *byte == b"\n"[0]);
        let url = split_iter.next().ok_or("Failed to get url.")?;
        let payload: Vec<u8> = split_iter.flatten().cloned().collect();
        Ok(Request {
            uri_path: String::from_utf8(url[..(url.len() - 1)].to_vec())?,
            payload,
        })
    }
}

pub type Response = Vec<u8>;

#[cfg(test)]
mod tests {
    use crate::{prelude::*, Network};
    use chashmap::CHashMap;
    use std::{
        convert::TryFrom,
        error::Error,
        net::{IpAddr, SocketAddr},
        str::FromStr,
        time::Duration,
    };

    #[test]
    fn request_correctly_built() {
        let request = Request {
            uri_path: "/commands".to_string(),
            payload: b"some_command".to_vec(),
        };
        let bytes: Vec<u8> = request.into();
        assert_eq!(b"/commands\r\nsome_command".to_vec(), bytes)
    }

    #[test]
    fn request_correctly_parsed() {
        let request = Request {
            uri_path: "/commands".to_string(),
            payload: b"some_command".to_vec(),
        };
        assert_eq!(
            Request::try_from(b"/commands\r\nsome_command".to_vec()).unwrap(),
            request
        )
    }

    #[test]
    fn single_threaded() {
        std::thread::spawn(|| {
            let _result = Network::listen("127.0.0.1:7878", |mut stream| {
                Network::handle_message(&mut stream, |_request| Ok("pong".as_bytes().to_vec()))
            });
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(
            Network::send_request_to("127.0.0.1:7878", Request::new("/ping".to_string(), vec![]))
                .unwrap(),
            "pong".as_bytes()
        )
    }

    #[test]
    fn single_threaded_async() {
        use async_std::task;

        async fn handle_request(request: Request) -> Result<Response, String> {
            Ok("pong".as_bytes().to_vec())
        };

        async fn handle_connection(stream: Box<dyn AsyncStream>) -> Result<(), String> {
            Network::handle_message_async(stream, handle_request).await
        };

        task::spawn(async { Network::listen_async("127.0.0.1:7878", handle_connection).await });
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(
            Network::send_request_to("127.0.0.1:7878", Request::new("/ping".to_string(), vec![]))
                .unwrap(),
            "pong".as_bytes()
        )
    }

    #[test]
    fn network_client() {
        std::thread::spawn(|| {
            let _result = Network::listen("127.0.0.1:7878", |mut stream| {
                Network::handle_message(&mut stream, |_request| Ok("pong".as_bytes().to_vec()))
            });
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        let client = Network::new("127.0.0.1:7878");
        assert_eq!(
            client
                .send_request(Request::new("/ping".to_string(), vec![]))
                .unwrap(),
            "pong".as_bytes()
        );
        assert_eq!(
            client
                .send_request(Request::new("/ping".to_string(), vec![]))
                .unwrap(),
            "pong".as_bytes()
        )
    }

    #[test]
    fn multi_threaded() {
        std::thread::spawn(|| {
            let _result = Network::listen("127.0.0.1:7878", |mut stream| {
                std::thread::spawn(move || {
                    let _result = Network::handle_message(&mut stream, |_request| {
                        Ok("pong".as_bytes().to_vec())
                    });
                });
                Ok(())
            });
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::thread::spawn(|| {
            assert_eq!(
                Network::send_request_to(
                    "127.0.0.1:7878",
                    Request::new("/ping".to_string(), vec![])
                )
                .unwrap(),
                "pong".as_bytes()
            )
        });
        assert_eq!(
            Network::send_request_to("127.0.0.1:7878", Request::new("/ping".to_string(), vec![]))
                .unwrap(),
            "pong".as_bytes()
        )
    }

    struct Peer {
        peers: CHashMap<SocketAddr, ()>,
    }

    impl Peer {
        pub fn new() -> Peer {
            Peer {
                peers: CHashMap::new(),
            }
        }

        pub fn handle_message(&self, request: Request) -> Result<Response, Box<dyn Error>> {
            match request.url() {
                "/number_of_peers" => Ok(vec![self.peers.len() as u8]),
                "/add_peer" => {
                    self.peers.insert(
                        SocketAddr::from_str(
                            String::from_utf8(request.payload.clone()).unwrap().as_ref(),
                        )
                        .unwrap(),
                        (),
                    );
                    Ok(vec![])
                }
                "/add_me" => {
                    let address = SocketAddr::from_str(
                        String::from_utf8(request.payload.clone()).unwrap().as_ref(),
                    )
                    .unwrap();
                    for (peer, _) in self.peers.clone() {
                        Network::send_request_to(
                            peer.to_string().as_ref(),
                            Request::new("/add_peer".to_string(), request.payload.clone()),
                        );
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(50));
                            Network::send_request_to(
                                address.to_string().as_ref(),
                                Request::new(
                                    "/add_peer".to_string(),
                                    peer.to_string().into_bytes(),
                                ),
                            );
                        });
                    }
                    self.peers.insert(address, ());
                    Ok(vec![])
                }
                _ => unimplemented!(),
            }
        }

        pub fn start(&self, port: u16) -> Result<(), Box<dyn Error>> {
            Network::listen(
                SocketAddr::new(IpAddr::from_str("127.0.0.1")?, port)
                    .to_string()
                    .as_ref(),
                |mut stream| {
                    Network::handle_message(&mut stream, |request| self.handle_message(request))
                },
            )
        }

        pub fn start_and_connect(&self, port: u16, peer: &str) -> Result<(), Box<dyn Error>> {
            self.peers.insert(SocketAddr::from_str(peer)?, ());
            Network::send_request_to(
                peer,
                Request::new(
                    "/add_me".to_string(),
                    SocketAddr::new(IpAddr::from_str("127.0.0.1")?, port)
                        .to_string()
                        .into_bytes(),
                ),
            )?;
            self.start(port)
        }
    }

    #[test]
    fn peer_to_peer() {
        std::thread::spawn(|| {
            Peer::new().start(7879);
        });
        std::thread::sleep(Duration::from_millis(50));
        std::thread::spawn(|| {
            Peer::new().start_and_connect(7880, "127.0.0.1:7879");
        });
        std::thread::sleep(Duration::from_millis(50));
        std::thread::spawn(|| {
            Peer::new().start_and_connect(7881, "127.0.0.1:7879");
        });
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            Network::send_request_to(
                "127.0.0.1:7881",
                Request::new("/number_of_peers".to_string(), vec![])
            )
            .unwrap(),
            vec![2]
        )
    }
}
