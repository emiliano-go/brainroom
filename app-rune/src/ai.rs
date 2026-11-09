//! The teaching agent thread and its event channel.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use brainroom_agent::{Event, Input, Message};

pub struct AgentHandle {
    pub inbox: Sender<Input>,
    pub events: Receiver<Event>,
}

pub fn start(
    history: Vec<Message>,
    mode: brainroom_agent::Mode,
    model: Option<String>,
    thinking_level: Option<String>,
) -> AgentHandle {
    let (inbox, inputs) = channel();
    let (output, events) = channel();
    let data_dir = crate::data::data_dir();
    let model = model.unwrap_or_else(|| brainroom_ai::model_id(&data_dir));
    std::thread::spawn(move || {
        let completer = brainroom_agent::OpenRouter {
            model,
            timeout: Duration::from_secs(120),
            thinking_level,
        };
        brainroom_agent::run_with_mode(&completer, data_dir, history, inputs, output, mode);
    });
    AgentHandle { inbox, events }
}
