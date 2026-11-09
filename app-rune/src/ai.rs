//! The teaching agent thread and its event channel.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use brainroom_agent::{Event, Input, Message};

pub struct AgentHandle {
    pub inbox: Sender<Input>,
    pub events: Receiver<Event>,
}

pub fn start(history: Vec<Message>, practice: bool) -> AgentHandle {
    let (inbox, inputs) = channel();
    let (output, events) = channel();
    let model = brainroom_ai::model_id(&crate::data::data_dir());
    let data_dir = crate::data::data_dir();
    std::thread::spawn(move || {
        let completer = brainroom_agent::OpenRouter {
            model,
            timeout: Duration::from_secs(120),
        };
        let mode = if practice {
            brainroom_agent::Mode::Practice
        } else {
            brainroom_agent::Mode::Teach
        };
        brainroom_agent::run_with_mode(&completer, data_dir, history, inputs, output, mode);
    });
    AgentHandle { inbox, events }
}
