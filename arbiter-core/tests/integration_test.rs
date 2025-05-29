use arbiter_core::{
  agent::{Agent, AgentContainer, Context},
  handler::Handler,
  runtime::Runtime,
};

// Message types for our pipeline system
#[derive(Debug, Clone)]
struct StartProcessing(i32);

#[derive(Debug, Clone)]
struct ProcessedData(i32);

#[derive(Debug, Clone)]
struct ValidatedData(i32);

#[derive(Debug, Clone)]
struct FinalResult(i32);

// Processor agent - takes StartProcessing and outputs ProcessedData
#[derive(Clone)]
struct ProcessorAgent {
  name: String,
}

impl Agent for ProcessorAgent {}

impl Handler<StartProcessing> for ProcessorAgent {
  type Reply = ProcessedData;

  // This will automatically be routed to next handler!

  fn handle(&mut self, message: StartProcessing) -> Self::Reply {
    let processed_value = message.0 * 2; // Double the input
    println!("{}: Processing {} -> {}", self.name, message.0, processed_value);
    ProcessedData(processed_value)
  }
}

// Validator agent - takes ProcessedData and outputs ValidatedData
#[derive(Clone)]
struct ValidatorAgent {
  name: String,
}

impl Agent for ValidatorAgent {}

impl Handler<ProcessedData> for ValidatorAgent {
  type Reply = ValidatedData;

  // This will automatically be routed to next handler!

  fn handle(&mut self, message: ProcessedData) -> Self::Reply {
    let validated_value = message.0 + 10; // Add validation bonus
    println!("{}: Validating {} -> {}", self.name, message.0, validated_value);
    ValidatedData(validated_value)
  }
}

// Finalizer agent - takes ValidatedData and outputs FinalResult
#[derive(Clone)]
struct FinalizerAgent {
  name: String,
}

impl Agent for FinalizerAgent {}

impl Handler<ValidatedData> for FinalizerAgent {
  type Reply = FinalResult;

  // This will automatically be routed to next handler!

  fn handle(&mut self, message: ValidatedData) -> Self::Reply {
    let final_value = message.0 * 3; // Triple for final result
    println!("{}: Finalizing {} -> {}", self.name, message.0, final_value);
    FinalResult(final_value)
  }
}

// Logger agent - takes FinalResult and logs it (returns () to end the chain)
#[derive(Clone)]
struct LoggerAgent {
  name:    String,
  results: Vec<i32>,
}

impl Agent for LoggerAgent {}

impl Handler<FinalResult> for LoggerAgent {
  type Reply = ();

  // () replies don't get routed further - ends the chain

  fn handle(&mut self, message: FinalResult) -> Self::Reply {
    println!("{}: Logging final result: {}", self.name, message.0);
    self.results.push(message.0);
  }
}

#[test]
fn test_automatic_message_routing() {
  println!("=== Testing Automatic Message Routing ===");

  let mut runtime = Runtime::new();

  // Register handlers in the processing pipeline
  runtime.register_handler(ProcessorAgent { name: "DataProcessor".to_string() });

  runtime.register_handler(ValidatorAgent { name: "DataValidator".to_string() });

  runtime.register_handler(FinalizerAgent { name: "DataFinalizer".to_string() });

  runtime
    .register_handler(LoggerAgent { name: "ResultLogger".to_string(), results: Vec::new() });

  // Send initial message and let the runtime automatically route through the pipeline
  println!("\n--- Processing Pipeline ---");
  println!("Input: 5");
  println!("Expected path: 5 -> 10 -> 20 -> 60");

  let iterations = runtime.send_and_run(StartProcessing(5));

  println!("\nPipeline completed in {} iterations", iterations);
  assert_eq!(iterations, 4); // Should process 4 messages: Start -> Processed -> Validated -> Final
  assert!(runtime.is_idle()); // Queue should be empty after processing
}

#[test]
fn test_multiple_parallel_pipelines() {
  println!("\n=== Testing Multiple Parallel Pipelines ===");

  let mut runtime = Runtime::new();

  // Register the same handlers as before
  runtime.register_handler(ProcessorAgent { name: "Processor1".to_string() });

  runtime.register_handler(ValidatorAgent { name: "Validator1".to_string() });

  runtime.register_handler(FinalizerAgent { name: "Finalizer1".to_string() });

  runtime.register_handler(LoggerAgent { name: "Logger1".to_string(), results: Vec::new() });

  // Send multiple messages - they'll all flow through the pipeline
  println!("\n--- Multiple Inputs ---");
  runtime.send_message(StartProcessing(1));
  runtime.send_message(StartProcessing(2));
  runtime.send_message(StartProcessing(3));

  println!("Queue length before processing: {}", runtime.queue_length());

  let iterations = runtime.run();

  println!("All pipelines completed in {} iterations", iterations);
  assert_eq!(iterations, 12); // 3 messages * 4 steps each = 12 iterations
  assert!(runtime.is_idle());
}

#[test]
fn test_agent_container_with_routing() {
  println!("\n=== Testing AgentContainer Direct Usage ===");

  // Test that AgentContainer works properly now
  let processor = ProcessorAgent { name: "DirectProcessor".to_string() };

  let mut container = AgentContainer::new(processor);
  container.register_handler::<StartProcessing>();

  let result = container.handle_message(StartProcessing(10));
  assert!(result.is_some());

  // The result would be ProcessedData(20) if we could extract it
  println!("AgentContainer processed message successfully");
}

#[test]
fn test_context_simple() {
  println!("\n=== Testing Context Pattern ===");

  let processor = ProcessorAgent { name: "ContextProcessor".to_string() };

  let context = Context::new(processor);
  let result = context.handle_with(StartProcessing(7));

  // result should be ProcessedData(14)
  println!("Context processed message and returned result");
}
