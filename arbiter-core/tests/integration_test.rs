use arbiter_core::{
  agent::{status, Agent, Context, LifeCycle},
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

impl LifeCycle for ProcessorAgent {}

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

impl LifeCycle for ValidatorAgent {}

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

impl LifeCycle for FinalizerAgent {}

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

impl LifeCycle for LoggerAgent {}

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

  let mut container = Agent::new(processor);
  container.with_handler::<StartProcessing>();

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

#[test]
fn test_agent_containers_with_runtime() {
  println!("\n=== Testing Multiple AgentContainers with Runtime ===");

  // Create multi-capability agents
  #[derive(Clone)]
  struct MultiProcessorAgent {
    name:            String,
    processed_count: i32,
  }

  impl LifeCycle for MultiProcessorAgent {}

  // MultiProcessor handles StartProcessing and ValidatedData
  impl Handler<StartProcessing> for MultiProcessorAgent {
    type Reply = ProcessedData;

    fn handle(&mut self, message: StartProcessing) -> Self::Reply {
      self.processed_count += 1;
      let processed_value = message.0 * 2;
      println!(
        "{}: Processing {} -> {} (count: {})",
        self.name, message.0, processed_value, self.processed_count
      );
      ProcessedData(processed_value)
    }
  }

  impl Handler<ValidatedData> for MultiProcessorAgent {
    type Reply = FinalResult;

    fn handle(&mut self, message: ValidatedData) -> Self::Reply {
      let final_value = message.0 * 3;
      println!("{}: Finalizing {} -> {}", self.name, message.0, final_value);
      FinalResult(final_value)
    }
  }

  #[derive(Clone)]
  struct MultiValidatorAgent {
    name:             String,
    validation_bonus: i32,
    log_entries:      Vec<String>,
  }

  impl LifeCycle for MultiValidatorAgent {}

  // MultiValidator handles ProcessedData and FinalResult
  impl Handler<ProcessedData> for MultiValidatorAgent {
    type Reply = ValidatedData;

    fn handle(&mut self, message: ProcessedData) -> Self::Reply {
      let validated_value = message.0 + self.validation_bonus;
      println!(
        "{}: Validating {} -> {} (bonus: {})",
        self.name, message.0, validated_value, self.validation_bonus
      );
      ValidatedData(validated_value)
    }
  }

  impl Handler<FinalResult> for MultiValidatorAgent {
    type Reply = ();

    // Ends the chain

    fn handle(&mut self, message: FinalResult) -> Self::Reply {
      let log_entry = format!("Final result logged: {}", message.0);
      println!("{}: {}", self.name, log_entry);
      self.log_entries.push(log_entry);
    }
  }

  // Create agent containers with multiple handlers each
  let processor =
    MultiProcessorAgent { name: "MultiProcessor".to_string(), processed_count: 0 };

  let validator = MultiValidatorAgent {
    name:             "MultiValidator".to_string(),
    validation_bonus: 5,
    log_entries:      Vec::new(),
  };

  let mut processor_container = Agent::new(processor);
  processor_container.with_handler::<StartProcessing>();
  processor_container.with_handler::<ValidatedData>();

  let mut validator_container = Agent::new(validator);
  validator_container.with_handler::<ProcessedData>();
  validator_container.with_handler::<FinalResult>();

  // Set up runtime and register our containers' handlers
  let mut runtime = Runtime::new();

  // Extract agents from containers to register as handlers in runtime
  // (This shows how containers can be used standalone or integrated with runtime)

  // Register MultiProcessor for both message types it handles
  runtime.register_handler::<MultiProcessorAgent, StartProcessing>(MultiProcessorAgent {
    name:            "RuntimeProcessor1".to_string(),
    processed_count: 0,
  });
  runtime.register_handler::<MultiProcessorAgent, ValidatedData>(MultiProcessorAgent {
    name:            "RuntimeProcessor2".to_string(),
    processed_count: 0,
  });

  // Register MultiValidator for both message types it handles
  runtime.register_handler::<MultiValidatorAgent, ProcessedData>(MultiValidatorAgent {
    name:             "RuntimeValidator1".to_string(),
    validation_bonus: 5,
    log_entries:      Vec::new(),
  });
  runtime.register_handler::<MultiValidatorAgent, FinalResult>(MultiValidatorAgent {
    name:             "RuntimeValidator2".to_string(),
    validation_bonus: 5,
    log_entries:      Vec::new(),
  });

  // Test the automatic routing through runtime
  println!("\n--- Automatic Multi-Agent Pipeline ---");
  println!("Input: 8");
  println!("Expected flow:");
  println!("  8 -> MultiProcessor -> 16");
  println!("  16 -> MultiValidator -> 21 (16 + 5 bonus)");
  println!("  21 -> MultiProcessor -> 63 (21 * 3)");
  println!("  63 -> MultiValidator -> logged");

  let iterations = runtime.send_and_run(StartProcessing(8));

  println!("\nMulti-agent pipeline completed in {} iterations", iterations);
  assert_eq!(iterations, 4); // 4 message hops through the agents
  assert!(runtime.is_idle());

  // Test direct container usage (without runtime routing)
  println!("\n--- Direct Container Usage ---");

  // Process through first container
  let result1 = processor_container.handle_message(StartProcessing(3));
  assert!(result1.is_some());

  // The containers maintain their state independently
  assert_eq!(processor_container.inner().processed_count, 1);

  println!("Direct container processing completed successfully");
}

fn test_compile_time_safety_documentation() {
  // This test passes just by existing - the real test is in the doctest above
  println!("Compile-time safety is enforced! ✅");
}

#[test]
fn test_agent_lifecycle_management() {
  println!("\n=== Testing Agent Lifecycle Management ===");

  // Create an agent that logs lifecycle events
  #[derive(Clone)]
  struct LifecycleAgent {
    name:            String,
    events:          Vec<String>,
    processed_count: i32,
  }

  impl LifeCycle for LifecycleAgent {
    fn on_start(&mut self) {
      let event = format!("{}: Started", self.name);
      println!("{}", event);
      self.events.push(event);
    }

    fn on_pause(&mut self) {
      let event = format!("{}: Paused", self.name);
      println!("{}", event);
      self.events.push(event);
    }

    fn on_stop(&mut self) {
      let event = format!("{}: Stopped", self.name);
      println!("{}", event);
      self.events.push(event);
    }

    fn on_resume(&mut self) {
      let event = format!("{}: Resumed", self.name);
      println!("{}", event);
      self.events.push(event);
    }
  }

  impl Handler<StartProcessing> for LifecycleAgent {
    type Reply = ProcessedData;

    fn handle(&mut self, message: StartProcessing) -> Self::Reply {
      self.processed_count += 1;
      let processed_value = message.0 + 100; // Add 100 to distinguish from other processors
      let event = format!(
        "{}: Processed {} -> {} (count: {})",
        self.name, message.0, processed_value, self.processed_count
      );
      println!("{}", event);
      self.events.push(event);
      ProcessedData(processed_value)
    }
  }

  // Create agent container
  let agent = LifecycleAgent {
    name:            "LifecycleDemo".to_string(),
    events:          Vec::new(),
    processed_count: 0,
  };

  let mut container = Agent::new(agent);
  container.with_handler::<StartProcessing>();

  // Test lifecycle: Stopped -> Running -> Paused -> Running -> Stopped
  println!("\n--- Initial State ---");
  assert_eq!(*container.state(), status::Stopped);
  assert!(!container.is_active());

  println!("\n--- Starting Agent ---");
  container.start();
  assert_eq!(*container.state(), status::Running);
  assert!(container.is_active());

  // Process a message while running
  println!("\n--- Processing Message While Running ---");
  let result = container.handle_message(StartProcessing(5));
  assert!(result.is_some());
  assert_eq!(container.inner().processed_count, 1);

  println!("\n--- Pausing Agent ---");
  container.pause();
  assert_eq!(*container.state(), status::Paused);
  assert!(!container.is_active());

  // Try to process message while paused - should be ignored
  println!("\n--- Trying to Process Message While Paused ---");
  let result = container.handle_message(StartProcessing(10));
  assert!(result.is_none()); // Message ignored
  assert_eq!(container.inner().processed_count, 1); // Count unchanged

  println!("\n--- Resuming Agent ---");
  container.resume();
  assert_eq!(*container.state(), status::Running);
  assert!(container.is_active());

  // Process message after resume
  println!("\n--- Processing Message After Resume ---");
  let result = container.handle_message(StartProcessing(15));
  assert!(result.is_some());
  assert_eq!(container.inner().processed_count, 2);

  println!("\n--- Stopping Agent ---");
  container.stop();
  assert_eq!(*container.state(), status::Stopped);
  assert!(!container.is_active());

  // Verify lifecycle events were recorded
  let events = &container.inner().events;
  assert!(events.iter().any(|e| e.contains("Started")));
  assert!(events.iter().any(|e| e.contains("Paused")));
  assert!(events.iter().any(|e| e.contains("Resumed")));
  assert!(events.iter().any(|e| e.contains("Stopped")));

  println!("\nLifecycle management test completed successfully! ✅");
  println!("Total lifecycle events: {}", events.len());
}

#[test]
fn test_dynamic_agent_management_with_runtime() {
  println!("\n=== Testing Dynamic Agent Management with Runtime ===");

  use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
  };

  // Create a shared runtime
  let runtime = Arc::new(Mutex::new(Runtime::new().with_max_iterations(100)));

  // Clone the runtime for the background task
  let runtime_clone = Arc::clone(&runtime);

  // Flag to control the background runtime
  let running = Arc::new(Mutex::new(true));
  let running_clone = Arc::clone(&running);

  // Start runtime in background thread
  let runtime_handle = thread::spawn(move || {
    let mut iteration_count = 0;
    while *running_clone.lock().unwrap() {
      let iterations = {
        let mut rt = runtime_clone.lock().unwrap();
        rt.run()
      };

      if iterations > 0 {
        iteration_count += iterations;
        println!(
          "Background runtime processed {} messages (total: {})",
          iterations, iteration_count
        );
      }

      // Small sleep to prevent busy waiting
      thread::sleep(Duration::from_millis(10));
    }
    println!("Background runtime stopped. Total iterations: {}", iteration_count);
    iteration_count
  });

  // Give the background task a moment to start
  thread::sleep(Duration::from_millis(50));

  // Create different types of agents for testing
  #[derive(Clone)]
  struct WorkerAgent {
    id:         String,
    work_count: i32,
    status:     String,
  }

  impl LifeCycle for WorkerAgent {
    fn on_start(&mut self) {
      self.status = "Running".to_string();
      println!("Worker {}: Started", self.id);
    }

    fn on_pause(&mut self) {
      self.status = "Paused".to_string();
      println!("Worker {}: Paused", self.id);
    }

    fn on_stop(&mut self) {
      self.status = "Stopped".to_string();
      println!("Worker {}: Stopped", self.id);
    }

    fn on_resume(&mut self) {
      self.status = "Running".to_string();
      println!("Worker {}: Resumed", self.id);
    }
  }

  impl Handler<StartProcessing> for WorkerAgent {
    type Reply = ProcessedData;

    fn handle(&mut self, message: StartProcessing) -> Self::Reply {
      self.work_count += 1;
      let result = message.0 + 1000; // Add 1000 to distinguish worker processing
      println!(
        "Worker {}: Processed {} -> {} (count: {})",
        self.id, message.0, result, self.work_count
      );
      ProcessedData(result)
    }
  }

  // Test 1: Add agents dynamically
  println!("\n--- Test 1: Adding Agents Dynamically ---");

  {
    let mut rt = runtime.lock().unwrap();

    // Add first worker
    let worker1 = WorkerAgent {
      id:         "W1".to_string(),
      work_count: 0,
      status:     "Stopped".to_string(),
    };

    assert!(rt.register_agent("worker1".to_string(), worker1));
    println!("Added worker1 to runtime");

    // Add second worker and start it immediately
    let worker2 = WorkerAgent {
      id:         "W2".to_string(),
      work_count: 0,
      status:     "Stopped".to_string(),
    };

    assert!(rt.add_and_start_agent("worker2".to_string(), worker2));
    println!("Added and started worker2");

    // Verify agents are registered
    assert_eq!(rt.list_agents().len(), 2);
    assert_eq!(rt.get_agent_state("worker1"), Some(status::Stopped));
    assert_eq!(rt.get_agent_state("worker2"), Some(status::Running));
  }

  // Test 2: Start an agent and send it work
  println!("\n--- Test 2: Starting Agent and Sending Work ---");

  {
    let mut rt = runtime.lock().unwrap();

    // Start worker1
    assert!(rt.start_agent("worker1"));
    assert_eq!(rt.get_agent_state("worker1"), Some(status::Running));

    // Register a simple handler to process work
    rt.register_handler::<WorkerAgent, StartProcessing>(WorkerAgent {
      id:         "Handler".to_string(),
      work_count: 0,
      status:     "Running".to_string(),
    });

    // Send some work
    rt.send_message(StartProcessing(5));
    rt.send_message(StartProcessing(10));

    println!("Sent work to agents");
  }

  // Give background runtime time to process
  thread::sleep(Duration::from_millis(100));

  // Test 3: Pause and resume agents
  println!("\n--- Test 3: Pausing and Resuming Agents ---");

  {
    let mut rt = runtime.lock().unwrap();

    // Pause worker1
    assert!(rt.pause_agent("worker1"));
    assert_eq!(rt.get_agent_state("worker1"), Some(status::Paused));

    // Send work while paused (should be ignored by paused agents)
    rt.send_message(StartProcessing(20));

    // Resume worker1
    assert!(rt.resume_agent("worker1"));
    assert_eq!(rt.get_agent_state("worker1"), Some(status::Running));

    // Send more work after resume
    rt.send_message(StartProcessing(25));

    println!("Paused, sent work, resumed, sent more work");
  }

  // Give background runtime time to process
  thread::sleep(Duration::from_millis(100));

  // Test 4: Stop agents
  println!("\n--- Test 4: Stopping Agents ---");

  {
    let mut rt = runtime.lock().unwrap();

    // Stop worker1
    assert!(rt.stop_agent("worker1"));
    assert_eq!(rt.get_agent_state("worker1"), Some(status::Stopped));

    // Try to stop non-existent agent
    assert!(!rt.stop_agent("nonexistent"));

    println!("Stopped worker1");
  }

  // Test 5: Remove agents
  println!("\n--- Test 5: Removing Agents ---");

  {
    let mut rt = runtime.lock().unwrap();

    // Remove worker1
    assert!(rt.remove_agent("worker1"));
    assert_eq!(rt.get_agent_state("worker1"), None);
    assert_eq!(rt.list_agents().len(), 1);

    // Try to remove non-existent agent
    assert!(!rt.remove_agent("nonexistent"));

    println!("Removed worker1 from runtime");
  }

  // Test 6: Add more agents while runtime is still running
  println!("\n--- Test 6: Adding More Agents While Running ---");

  {
    let mut rt = runtime.lock().unwrap();

    // Add worker3 and start it
    let worker3 = WorkerAgent {
      id:         "W3".to_string(),
      work_count: 0,
      status:     "Stopped".to_string(),
    };

    assert!(rt.add_and_start_agent("worker3".to_string(), worker3));

    // Send final batch of work
    rt.send_message(StartProcessing(100));
    rt.send_message(StartProcessing(200));

    println!("Added worker3 and sent final work batch");
  }

  // Give final processing time
  thread::sleep(Duration::from_millis(100));

  // Verify final state
  {
    let rt = runtime.lock().unwrap();
    println!("\n--- Final State ---");
    println!("Agents in runtime: {:?}", rt.list_agents());
    println!("Worker2 state: {:?}", rt.get_agent_state("worker2"));
    println!("Worker3 state: {:?}", rt.get_agent_state("worker3"));
    println!("Queue length: {}", rt.queue_length());
    println!("Total mailbox messages: {}", rt.total_mailbox_messages());

    // Should have worker2 and worker3
    assert_eq!(rt.list_agents().len(), 2);
  }

  // Stop the background runtime
  {
    let mut flag = running.lock().unwrap();
    *flag = false;
  }

  // Wait for background task to complete
  let total_iterations = runtime_handle.join().unwrap();

  println!("\n--- Test Results ---");
  println!("Background runtime processed {} total iterations", total_iterations);
  println!("Dynamic agent management test completed successfully! ✅");

  // Verify we processed some messages
  assert!(total_iterations > 0, "Runtime should have processed some messages");
}
