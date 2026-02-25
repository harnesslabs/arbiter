use crate::{
  agent::{Agent, ControlSignal, Controller, LifeCycle, ProcessingAgent, State, log_handler_error},
  handler::{Envelope, HandleResult},
  network::{Network, memory::InMemory},
  protocol::AgentId,
};

pub(crate) fn spawn<L: LifeCycle>(mut agent: Agent<L, InMemory>) -> ProcessingAgent<L, InMemory> {
  let name = agent.name.clone();
  let address = agent.address();
  let local_agent_id = AgentId::from(address.to_string());
  let controller = Controller::new();
  let mut inner_controller = controller.inner;
  let outer_controller = controller.outer;

  let task = tokio::spawn(async move {
    loop {
      // ────────────────────────────────────────────────────────────────
      // Control-plane messages (START / STOP / GET_STATE)
      // ────────────────────────────────────────────────────────────────
      let prev_state = agent.state;
      tokio::select! {
        biased;
        control_signal = inner_controller.instruction_receiver.recv() => {
          match control_signal {
            Some(ControlSignal::Start) => {
              agent.state = State::Running;
              inner_controller.state_sender.send(State::Running).await.unwrap();
              let start_message = agent.inner.on_start();
              println!("sending start_message for agent {}", agent.name.as_deref().unwrap_or("unknown"));
              agent
                .connection
                .network
                .send(Envelope::package(start_message).with_sender(local_agent_id.clone()))
                .await;
            },
            Some(ControlSignal::Stop) => {
              agent.state = State::Stopped;
              inner_controller.state_sender.send(State::Stopped).await.unwrap();
              let stop_message = agent.inner.on_stop();
              agent
                .connection
                .network
                .send(Envelope::package(stop_message).with_sender(local_agent_id.clone()))
                .await;
              break;
            },
            Some(ControlSignal::GetState) => {
              inner_controller.state_sender.send(prev_state).await.unwrap();
            },
            None => {
              break;
            },
          }
        }
        // ────────────────────────────────────────────────────────────────
        // Application messages coming from the transport
        // ────────────────────────────────────────────────────────────────
        message = agent.connection.network.receive() => {
          if let Some(message) = message {
            if !message.meta.recipient.matches_agent(&local_agent_id) {
              continue;
            }

            println!("received message {:?} for agent {}", message, agent.name.as_deref().unwrap_or("unknown"));
            if let Some(message_type_id) = agent.resolve_handler_type_id(&message) {
              let handler = agent.handlers.get(&message_type_id).expect("handler id registry drift");
              let reply = handler(&mut agent.inner, message);
              println!("reply for agent {}", agent.name.as_deref().unwrap_or("unknown"));
              match reply {
                Ok(HandleResult::Message(message)) => {
                  let message = message.with_sender(local_agent_id.clone());
                  println!("sending reply {:?} for agent {}", message, agent.name.as_deref().unwrap_or("unknown"));
                  agent.connection.network.send(message).await;
                },
                Ok(HandleResult::None) => {},
                Ok(HandleResult::Stop) => break,
                Err(error) => {
                  log_handler_error(agent.name.as_deref().unwrap_or("unknown"), &error);
                },
              }
            }
          }
        }
      }
    }

    agent
  });

  ProcessingAgent { name, address, task, outer_controller }
}
