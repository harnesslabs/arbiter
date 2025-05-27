use super::*;
use crate::{
  environment::{Database, Middleware},
  machine::{Action, Behavior, ConfigurableBehavior, Event, EventStream, Filter},
  messager::{Message, MessageFrom, Messager},
};

pub struct Agent<DB: Database> {
  pub id: String,

  pub stream: Stream<DB>,

  pub sender: Sender<DB>,

  pub(crate) behaviors: Vec<Box<dyn Behavior<DB>>>,
}

impl<DB: Database> Agent<DB> {
  // TODO: Used to create from a config basically
  pub fn builder(id: &str) -> AgentBuilder<DB> {
    AgentBuilder { id: id.to_owned(), behaviors: Vec::new() }
  }
}

pub struct AgentBuilder<DB: Database> {
  pub id:    String,
  behaviors: Vec<Box<dyn Behavior<DB>>>,
}

impl<DB> AgentBuilder<DB>
where
  DB: Database + 'static,
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
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

  pub fn build(
    self,
    middleware: Middleware<DB>,
    messager: Messager,
  ) -> Result<Agent<DB>, ArbiterCoreError> {
    Ok(Agent {
      id:        self.id.clone(),
      sender:    Sender {
        id:                  self.id,
        state_change_sender: middleware.sender,
        message_sender:      messager.broadcast_sender,
      },
      stream:    Stream {
        stream:  create_unified_stream(middleware.receiver, messager.broadcast_receiver),
        filters: HashMap::new(),
      },
      behaviors: self.behaviors,
    })
  }
}

pub struct Sender<DB: Database> {
  id:                  String,
  state_change_sender: mpsc::Sender<(DB::Location, DB::State)>,
  message_sender:      broadcast::Sender<Message>,
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
        // TODO: We should automatically serialize here, but not doing it for now.
        Action::MessageTo(message) =>
          if let Err(e) = self.message_sender.send(Message {
            from: self.id.clone(),
            to:   message.to,
            data: message.data,
          }) {
            return Err(ArbiterCoreError::MessagerError(format!(
              "Failed to send message: {:?}",
              e
            )));
          },
      }
    }
    Ok(())
  }
}

pub struct Stream<DB: Database> {
  stream:  EventStream<Event<DB>>,
  filters: HashMap<String, Box<dyn Filter<DB>>>,
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
