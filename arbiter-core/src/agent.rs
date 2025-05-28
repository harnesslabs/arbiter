use std::collections::HashMap;

use actix::prelude::*;

use super::*;
use crate::{
  environment::{Database, Middleware},
  error::ArbiterCoreError,
  machine::{Action, Behavior, ConfigurableBehavior, Event, EventStream, Filter},
  messager::{Message, MessageFrom, MessageTo},
  time::{SimulationTime, Tick, TimeSubscriber},
};

pub struct Agent<DB: Database + 'static> {
  pub id: String,

  pub stream: Stream<DB>,

  pub sender: Sender<DB>,

  pub(crate) behaviors: Vec<Box<dyn Behavior<DB>>>,

  pub(crate) time_subscriber: TimeSubscriber,

  middleware: Middleware<DB>,
}

impl<DB: Database + 'static> Actor for Agent<DB>
where
  DB::Location: Send + Sync,
  DB::State: Send + Sync,
{
  type Context = Context<Self>;

  fn started(&mut self, _ctx: &mut Context<Self>) {
    // Initialize agent with default time subscriber
    self.time_subscriber = TimeSubscriber::new();
  }
}

#[derive(Message)]
#[rtype(result = "Result<(), ArbiterCoreError>")]
pub struct ExecuteActions<DB: Database + 'static>(pub Vec<Action<DB>>);

impl<DB: Database + 'static> Handler<ExecuteActions<DB>> for Agent<DB>
where
  DB::Location: Send + Sync,
  DB::State: Send + Sync,
{
  type Result = Result<(), ArbiterCoreError>;

  fn handle(&mut self, msg: ExecuteActions<DB>, _ctx: &mut Context<Self>) -> Self::Result {
    for action in msg.0 {
      match action {
        Action::StateChange(location, state) => {
          if let Err(e) = self.middleware.sender.try_send((location, state)) {
            return Err(ArbiterCoreError::DatabaseError(format!(
              "Failed to send state change: {:?}",
              e
            )));
          }
        },
        Action::MessageTo(msg) => {
          if let Err(e) = self.middleware.broadcast_sender.send(Message {
            from: self.id.clone(),
            to:   msg.to,
            data: msg.data,
          }) {
            return Err(ArbiterCoreError::MessagerError(format!(
              "Failed to send message: {:?}",
              e
            )));
          }
        },
      }
    }
    Ok(())
  }
}

// Implement Tick handling for time-aware behaviors
impl<DB: Database + 'static> Handler<Tick> for Agent<DB>
where
  DB::Location: Send + Sync,
  DB::State: Send + Sync,
{
  type Result = ();

  fn handle(&mut self, msg: Tick, ctx: &mut Context<Self>) {
    self.time_subscriber.last_tick = Some(msg.0.clone());

    // Process behaviors with the new tick
    for behavior in &mut self.behaviors {
      if let Ok((_, actions)) = behavior.process_tick(msg.0.clone()) {
        // Execute actions resulting from the tick
        ctx.notify(ExecuteActions(actions.into_vec()));
      }
    }
  }
}

impl<DB: Database + 'static> Agent<DB>
where
  DB::Location: Send + Sync,
  DB::State: Send + Sync,
{
  pub fn builder(id: &str) -> AgentBuilder<DB> {
    AgentBuilder { id: id.to_owned(), behaviors: Vec::new() }
  }

  pub fn current_time(&self) -> Option<SimulationTime> { self.time_subscriber.last_tick.clone() }
}

pub struct AgentBuilder<DB: Database> {
  pub id:    String,
  behaviors: Vec<Box<dyn Behavior<DB>>>,
}

impl<DB> AgentBuilder<DB>
where
  DB: Database + 'static,
  DB::Location: Send + Sync,
  DB::State: Send + Sync,
{
  pub fn with_behavior<B: Behavior<DB> + 'static>(mut self, behavior: B) -> Self {
    self.behaviors.push(Box::new(behavior));
    self
  }

  pub fn with_behavior_from_config<B: ConfigurableBehavior<DB> + 'static>(
    mut self,
    behavior: B,
  ) -> Self {
    self.behaviors.push(behavior.create_behavior());
    self
  }

  pub fn build(self, middleware: Middleware<DB>) -> Result<Agent<DB>, ArbiterCoreError> {
    Ok(Agent {
      id: self.id.clone(),
      sender: Sender {
        id:                  self.id,
        state_change_sender: middleware.sender.clone(),
        message_sender:      middleware.broadcast_sender.clone(),
      },
      stream: Stream {
        stream:  create_unified_stream(
          middleware.receiver.resubscribe(),
          middleware.broadcast_receiver.resubscribe(),
        ),
        filters: HashMap::new(),
      },
      behaviors: self.behaviors,
      time_subscriber: TimeSubscriber::new(),
      middleware,
    })
  }
}

pub struct Stream<DB: Database> {
  stream:  EventStream<Event<DB>>,
  filters: HashMap<String, Box<dyn Filter<DB>>>,
}

impl<DB: Database> Clone for Stream<DB> {
  fn clone(&self) -> Self {
    // Create a new empty stream since we can't clone the dynamic Stream
    Self { stream: Box::pin(futures::stream::empty()), filters: HashMap::new() }
  }
}

pub struct Sender<DB: Database> {
  id:                  String,
  state_change_sender: mpsc::Sender<(DB::Location, DB::State)>,
  message_sender:      broadcast::Sender<Message>,
}

impl<DB: Database> Clone for Sender<DB> {
  fn clone(&self) -> Self {
    Self {
      id:                  self.id.clone(),
      state_change_sender: self.state_change_sender.clone(),
      message_sender:      self.message_sender.clone(),
    }
  }
}

impl<DB: Database> Sender<DB> {
  /// Execute a list of actions
  pub async fn execute_actions(
    &self,
    actions: crate::machine::Actions<DB>,
  ) -> Result<(), ArbiterCoreError> {
    for action in actions.into_vec() {
      match action {
        Action::StateChange(location, state) => {
          if let Err(e) = self.state_change_sender.send((location, state)).await {
            return Err(ArbiterCoreError::DatabaseError(format!(
              "Failed to send state change: {:?}",
              e
            )));
          }
        },
        Action::MessageTo(message) => {
          if let Err(e) = self.message_sender.send(Message {
            from: self.id.clone(),
            to:   message.to,
            data: message.data,
          }) {
            return Err(ArbiterCoreError::MessagerError(format!(
              "Failed to send message: {:?}",
              e
            )));
          }
        },
      }
    }
    Ok(())
  }
}

impl<DB: Database> Stream<DB> {
  /// Add a filter to the stream with a given identifier
  pub fn add_filter(&mut self, id: String, filter: Box<dyn Filter<DB>>) {
    self.filters.insert(id, filter);
  }

  /// Get a reference to the filters
  pub fn filters(&self) -> &HashMap<String, Box<dyn Filter<DB>>> { &self.filters }

  /// Get a mutable reference to the event stream
  pub fn stream_mut(&mut self) -> &mut EventStream<Event<DB>> { &mut self.stream }
}

fn create_unified_stream<DB: Database>(
  middleware_receiver: broadcast::Receiver<(DB::Location, DB::State)>,
  messager_receiver: broadcast::Receiver<Message>,
) -> EventStream<Event<DB>>
where
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
  let middleware_stream =
    Box::pin(stream::unfold(middleware_receiver, |mut receiver| async move {
      loop {
        match receiver.recv().await {
          Ok((location, state)) => return Some((Event::StateChange(location, state), receiver)),
          Err(broadcast::error::RecvError::Closed) => return None,
          Err(broadcast::error::RecvError::Lagged(_)) => {},
        }
      }
    }));

  let messager_stream = Box::pin(stream::unfold(messager_receiver, |mut receiver| async move {
    loop {
      match receiver.recv().await {
        Ok(message) =>
          return Some((
            Event::MessageFrom(MessageFrom { from: message.from, data: message.data }),
            receiver,
          )),
        Err(broadcast::error::RecvError::Closed) => return None,
        Err(broadcast::error::RecvError::Lagged(_)) => {},
      }
    }
  }));

  Box::pin(futures::stream::select(middleware_stream, messager_stream))
}
