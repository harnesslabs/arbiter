// use arbiter_core::{
//   agent::{Agent, State},
//   connection::{memory::InMemory, Connection},
//   prelude::*,
// };

// struct PingMessage;

// struct PongMessage;

// struct StopMessage;

// struct Ping {
//   pub max_count: usize,
//   pub count:     usize,
// }

// impl LifeCycle for Ping {
//   type StartMessage = PingMessage;
//   type StopMessage = StopMessage;

//   fn on_start(&mut self) -> Self::StartMessage {
//     println!("Ping on_start");
//     PingMessage
//   }

//   fn on_stop(&mut self) -> Self::StopMessage { StopMessage }
// }

// impl Handler<PongMessage> for Ping {
//   type Reply = PingMessage;

//   fn handle(&mut self, message: &PongMessage) -> HandleResult<Self::Reply> {
//     if self.count == self.max_count {
//       HandleResult::Stop
//     } else {
//       println!("Ping received PongMessage");
//       self.count += 1;
//       HandleResult::Message(PingMessage)
//     }
//   }
// }

// struct Pong;

// impl LifeCycle for Pong {
//   type StartMessage = ();
//   type StopMessage = ();

//   fn on_start(&mut self) -> Self::StartMessage {}

//   fn on_stop(&mut self) -> Self::StopMessage {}
// }

// impl Handler<PingMessage> for Pong {
//   type Reply = PongMessage;

//   fn handle(&mut self, _message: &PingMessage) -> HandleResult<Self::Reply> {
//     println!("Pong received PingMessage");
//     HandleResult::Message(PongMessage)
//   }
// }

// #[test]
// fn test_multi_agent() {
//   let ping = Agent::<Ping, InMemory>::new(Ping { max_count: 10, count: 0 });
//   let connection = ping.get_connection();
//   let pong = Agent::<Pong, InMemory>::new_with_connection(Pong, connection);
//   pong.address();

//   let pong = pong.process();
//   pong.start();

//   let ping = ping.process();
//   ping.start();

//   std::thread::sleep(std::time::Duration::from_millis(100));

//   ping.stop();
//   pong.stop();

//   let agent = ping.join();
//   assert_eq!(agent.inner().count, 10);
// }
