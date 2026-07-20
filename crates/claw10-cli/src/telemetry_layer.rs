use std::io::Write;
use std::sync::Mutex;

use tracing::{Event, Subscriber};
use tracing_subscriber::field::Visit;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Custom tracing layer that writes raw JSON telemetry events to a file.
///
/// The telemetry service emits events with target `claw10_telemetry` where the
/// message is already a JSON-serialized `TelemetryEvent`. This layer captures
/// only those events and writes them as one JSON object per line, suitable for
/// consumption by Vector.
pub struct TelemetryLayer<W: Write + Send + 'static> {
    writer: Mutex<W>,
}

impl<W: Write + Send + 'static> TelemetryLayer<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(writer),
        }
    }
}

struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            // The debug representation of a string includes surrounding quotes.
            // Parse it as a JSON string to recover the original content.
            let debug = format!("{value:?}");
            self.message = serde_json::from_str::<String>(&debug).ok();
        }
    }
}

impl<S, W> Layer<S> for TelemetryLayer<W>
where
    S: Subscriber,
    W: Write + Send + 'static,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() != "claw10_telemetry" {
            return;
        }

        let mut visitor = MessageVisitor { message: None };
        event.record(&mut visitor);

        if let Some(msg) = visitor.message {
            if let Ok(mut writer) = self.writer.lock() {
                let _ = writeln!(writer, "{msg}");
            }
        }
    }
}
