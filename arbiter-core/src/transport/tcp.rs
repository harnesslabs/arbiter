// #![cfg(feature = "tcp")]

// use std::{collections::VecDeque, net::SocketAddr};

// use crate::transport::{AsyncRuntime, Runtime, Transport};

// pub struct TcpTransport {
//   local_identity: SocketAddr,
//   message_queue:  VecDeque<Vec<u8>>,
// }

// impl Transport<AsyncRuntime> for TcpTransport {
//   type Address = SocketAddr;
//   type Error = String;
//   type Payload = Vec<u8>;

//   fn new() -> Self { Self { local_identity: todo!(), message_queue: VecDeque::new() } }

//   fn send(
//     &mut self,
//     payload: Self::Payload,
//   ) -> <AsyncRuntime as super::Runtime>::Output<Result<(), Self::Error>> {
//     self.message_queue.push_back(payload);
//     AsyncRuntime::wrap(Ok(()))
//   }

//   fn poll(&mut self) -> <AsyncRuntime as super::Runtime>::Output<Vec<Self::Payload>> { todo!() }

//   fn local_address(&self) -> Self::Address { todo!() }
// }
