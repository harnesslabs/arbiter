use std::process;

use arbiter::{
  actor::LifeCycle,
  handler::{Envelope, Handler},
  network::{
    Socket,
    tcp::{TcpEnvelope, TcpStream},
  },
  runtime::Runtime,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{Level, info};

#[derive(Parser, Debug)]
#[command(name = "Arbiter Chat CLI")]
#[command(about = "A decentralized chat application demonstrating Arbiter TCP actors", long_about = None)]
struct Args {
  /// Name to show in the chat
  #[arg(short, long)]
  name: String,

  /// Local port to listen on. If omitted, the OS assigns a random port.
  #[arg(short, long)]
  listen: Option<u16>,

  /// Address of another chat node to connect to
  #[arg(short, long)]
  connect: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ChatMessage {
  sender:  String,
  content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ChatStart;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ChatStop;

struct ChatActor {
  name: String,
}

impl LifeCycle for ChatActor {
  type Snapshot = ();
  type StartMessage = ChatStart;
  type StopMessage = ChatStop;

  fn on_start(&mut self) -> Self::StartMessage {
    info!("ChatActor started for user {}", self.name);
    ChatStart
  }

  fn on_stop(&mut self) -> Self::StopMessage { ChatStop }

  fn snapshot(&self) -> Self::Snapshot {}
}

impl Handler<ChatMessage> for ChatActor {
  type Reply = ();

  fn handle(&mut self, msg: &ChatMessage) -> Option<Self::Reply> {
    // Prevent printing our own messages twice (we print them when sending)
    if msg.sender != self.name {
      println!("[{}] {}", msg.sender, msg.content);
    }
    None
  }
}

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt().with_max_level(Level::INFO).init();

  let args = Args::parse();

  // Start runtime
  // Note: The generic TcpStream in arbiter hardcodes `127.0.0.1:0`.
  // For a production CLI we would pass args.listen down to N::new(),
  // but the default randomly assigned port works great for local testing!
  let mut runtime = Runtime::<TcpStream>::new();

  // Attempt connections
  if let Some(ref peer) = args.connect {
    info!("Connecting to peer at {}", peer);
    // We use block_on/await to ensure the connection establishes
    if let Err(e) = runtime.network().connect_to(peer).await {
      tracing::error!("Failed to connect to {}: {}", peer, e);
      process::exit(1);
    }
    // Give connection a moment to establish
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Successfully connected to {}", peer);
  }

  println!("========================================");
  println!(" Welcome to Arbiter Chat, {}!", args.name);
  println!(" Your node is listening on: {}", runtime.network().local_addr());
  println!(" Type your messages below. /quit to exit.");
  println!("========================================");

  // Spawn chat actor
  let chat_actor = runtime
    .spawn(ChatActor { name: args.name.clone() })
    .with_handler::<ChatMessage>()
    .with_name("chat_client");

  let mut process = runtime.process(chat_actor);
  process.start().await.unwrap();

  // Spawn a background task to read from stdin and broadcast
  let socket = runtime.socket();
  let name = args.name.clone();

  tokio::spawn(async move {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
      line.clear();
      match reader.read_line(&mut line).await {
        Ok(0) => break, // EOF
        Ok(_) => {
          let text = line.trim();
          if text.is_empty() {
            continue;
          }
          if text == "/quit" {
            println!("Exiting chat...");
            process::exit(0);
          }

          let msg = ChatMessage { sender: name.clone(), content: text.to_string() };

          // We broadcast this message through the network
          socket.send(TcpEnvelope::wrap(msg)).await;
        },
        Err(e) => {
          tracing::error!("Error reading stdin: {}", e);
          break;
        },
      }
    }
  });

  // Run until ctrl-c
  tokio::signal::ctrl_c().await.unwrap();
  println!("Exiting chat...");
}
