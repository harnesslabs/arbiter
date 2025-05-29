use arbiter_core::{
  agent::{Agent, AgentContainer, Context},
  handler::Handler,
  runtime::Runtime,
};

// Message types for our banking system
#[derive(Debug, Clone)]
struct Deposit(i32);

#[derive(Debug, Clone)]
struct Withdraw(i32);

#[derive(Debug, Clone)]
struct LogMessage(String);

#[derive(Debug, Clone)]
struct GetBalance;

// Bank agent - manages account balance
#[derive(Clone)]
struct BankAgent {
  account_id: String,
  balance:    i32,
}

impl Agent for BankAgent {}

impl Handler<Deposit> for BankAgent {
  type Reply = i32;

  // Returns new balance

  fn handle(&mut self, message: Deposit) -> Self::Reply {
    self.balance += message.0;
    println!("Account {}: Deposited {}. New balance: {}", self.account_id, message.0, self.balance);
    self.balance
  }
}

impl Handler<Withdraw> for BankAgent {
  type Reply = bool;

  // Returns success/failure

  fn handle(&mut self, message: Withdraw) -> Self::Reply {
    if self.balance >= message.0 {
      self.balance -= message.0;
      println!(
        "Account {}: Withdrew {}. New balance: {}",
        self.account_id, message.0, self.balance
      );
      true
    } else {
      println!("Account {}: Insufficient funds for withdrawal of {}", self.account_id, message.0);
      false
    }
  }
}

impl Handler<GetBalance> for BankAgent {
  type Reply = i32;

  fn handle(&mut self, _message: GetBalance) -> Self::Reply { self.balance }
}

// Logger agent - tracks all transactions for audit
#[derive(Clone)]
struct LoggerAgent {
  name:          String,
  message_count: i32,
  audit_trail:   Vec<String>,
}

impl Agent for LoggerAgent {}

impl Handler<LogMessage> for LoggerAgent {
  type Reply = ();

  fn handle(&mut self, message: LogMessage) -> Self::Reply {
    self.message_count += 1;
    let log_entry = format!("[{}] {}", self.message_count, message.0);
    println!("Logger '{}': {}", self.name, log_entry);
    self.audit_trail.push(log_entry);
  }
}

impl Handler<Deposit> for LoggerAgent {
  type Reply = ();

  fn handle(&mut self, message: Deposit) -> Self::Reply {
    self.message_count += 1;
    let log_entry = format!("[{}] AUDIT - Deposit of {} recorded", self.message_count, message.0);
    println!("Logger '{}': {}", self.name, log_entry);
    self.audit_trail.push(log_entry);
  }
}

impl Handler<Withdraw> for LoggerAgent {
  type Reply = ();

  fn handle(&mut self, message: Withdraw) -> Self::Reply {
    self.message_count += 1;
    let log_entry =
      format!("[{}] AUDIT - Withdrawal of {} attempted", self.message_count, message.0);
    println!("Logger '{}': {}", self.name, log_entry);
    self.audit_trail.push(log_entry);
  }
}

// Simple account agent using Context (shows different usage pattern)
#[derive(Clone)]
struct SimpleAccount {
  balance: i32,
}

impl Handler<Deposit> for SimpleAccount {
  type Reply = ();

  fn handle(&mut self, message: Deposit) -> Self::Reply {
    self.balance += message.0;
    println!("Simple account: Deposited {}. Balance: {}", message.0, self.balance);
  }
}

#[test]
fn test_agent_containers_direct() {
  println!("=== Testing AgentContainer Direct Usage ===");

  // Create bank agent
  let bank_agent = BankAgent { account_id: "ACC-001".to_string(), balance: 1000 };

  // Create logger agent
  let logger_agent = LoggerAgent {
    name:          "MainLogger".to_string(),
    message_count: 0,
    audit_trail:   Vec::new(),
  };

  // Wrap in containers and register handlers
  let mut bank_container = AgentContainer::new(bank_agent);
  bank_container.register_handler::<Deposit>();
  bank_container.register_handler::<Withdraw>();
  bank_container.register_handler::<GetBalance>();

  let mut logger_container = AgentContainer::new(logger_agent);
  logger_container.register_handler::<LogMessage>();
  logger_container.register_handler::<Deposit>();
  logger_container.register_handler::<Withdraw>();

  // Test banking operations
  println!("\n--- Banking Operations ---");

  // Deposit money
  let deposit = Deposit(500);
  let deposit_result = bank_container.handle_message(deposit.clone());
  assert!(deposit_result.is_some());
  assert_eq!(bank_container.agent().balance, 1500);

  // Log the deposit in audit trail
  logger_container.handle_message(deposit);

  // Try to withdraw money
  let withdraw = Withdraw(200);
  let withdraw_result = bank_container.handle_message(withdraw.clone());
  assert!(withdraw_result.is_some());
  assert_eq!(bank_container.agent().balance, 1300);

  // Log the withdrawal
  logger_container.handle_message(withdraw);

  // Check balance
  let balance_result = bank_container.handle_message(GetBalance);
  assert!(balance_result.is_some());

  // Test message selectivity
  println!("\n--- Testing Message Selectivity ---");

  // Bank agent should ignore log messages
  let log_msg = LogMessage("Banking transaction completed".to_string());
  let bank_log_result = bank_container.handle_message(log_msg.clone());
  assert!(bank_log_result.is_none());

  // Logger agent should handle log messages
  let logger_result = logger_container.handle_message(log_msg);
  assert!(logger_result.is_some());

  // Verify final states
  assert_eq!(bank_container.agent().balance, 1300);
  assert_eq!(logger_container.agent().message_count, 3); // 2 audit + 1 log message
  assert_eq!(logger_container.agent().audit_trail.len(), 3);
}

#[test]
fn test_runtime_integration() {
  println!("\n=== Testing Runtime Integration ===");

  let mut runtime = Runtime::new();

  // Register agents in runtime (they get wrapped in containers automatically)
  runtime.register_agent("bank".to_string(), BankAgent {
    account_id: "ACC-002".to_string(),
    balance:    2000,
  });

  runtime.register_agent("audit_logger".to_string(), LoggerAgent {
    name:          "AuditLogger".to_string(),
    message_count: 0,
    audit_trail:   Vec::new(),
  });

  // Register standalone handlers in runtime
  runtime.register_handler::<BankAgent, Deposit>(BankAgent {
    account_id: "StandaloneHandler".to_string(),
    balance:    0,
  });

  runtime.register_handler::<LoggerAgent, LogMessage>(LoggerAgent {
    name:          "RuntimeLogger".to_string(),
    message_count: 0,
    audit_trail:   Vec::new(),
  });

  // Test runtime message broadcasting
  println!("\n--- Runtime Message Broadcasting ---");

  // Send messages through runtime - they go to all matching handlers
  let deposit_msg = Deposit(1000);
  let deposit_sent = runtime.send_message(deposit_msg);
  assert!(deposit_sent);

  let withdraw_msg = Withdraw(300);
  let withdraw_sent = runtime.send_message(withdraw_msg);
  assert!(withdraw_sent);

  let log_msg = LogMessage("System initialized".to_string());
  let log_sent = runtime.send_message(log_msg);
  assert!(log_sent);

  // Verify runtime has registered agents
  assert_eq!(runtime.list_agents().len(), 2);
  assert!(runtime.get_agent_info("bank").is_some());
  assert!(runtime.get_agent_info("audit_logger").is_some());
}

#[test]
fn test_context_usage() {
  println!("\n=== Testing Context Usage ===");

  // Test the Context pattern with simple account
  let simple_account = SimpleAccount { balance: 100 };
  let context = Context::new(simple_account);

  // Use context to handle messages directly
  context.handle_with(Deposit(50));
  context.handle_with(Deposit(25));

  // Check final state
  context.with(|account| {
    assert_eq!(account.balance, 175);
    println!("Final simple account balance: {}", account.balance);
  });
}

#[test]
fn test_mixed_usage_patterns() {
  println!("\n=== Testing Mixed Usage Patterns ===");

  // Demonstrate different ways to use the framework

  // 1. Direct AgentContainer usage
  let bank = BankAgent { account_id: "MIXED-001".to_string(), balance: 500 };
  let mut bank_container = AgentContainer::new(bank);
  bank_container.register_handler::<Deposit>();
  bank_container.handle_message(Deposit(100));

  // 2. Context usage
  let simple = SimpleAccount { balance: 200 };
  let simple_context = Context::new(simple);
  simple_context.handle_with(Deposit(50));

  // 3. Runtime usage
  let mut runtime = Runtime::new();
  runtime.register_handler::<LoggerAgent, LogMessage>(LoggerAgent {
    name:          "MixedLogger".to_string(),
    message_count: 0,
    audit_trail:   Vec::new(),
  });
  runtime.send_message(LogMessage("Mixed test completed".to_string()));

  // All patterns work together!
  println!("All usage patterns work independently and can be mixed!");
}
