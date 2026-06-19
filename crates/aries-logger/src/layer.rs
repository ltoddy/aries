use std::fmt;
use std::io::Write;
use std::sync::Arc;

use colored::Colorize;
use parking_lot::Mutex;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber, span};
use tracing_subscriber::layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use crate::router::Inner;

#[derive(Clone, Debug)]
pub struct SessionId(pub String);

#[derive(Clone)]
pub struct RouterLayer {
    pub inner: Arc<Mutex<Inner>>,
}

impl<S> layer::Layer<S> for RouterLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        struct Visitor(Option<String>);

        impl Visit for Visitor {
            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() == "session_id" {
                    self.0 = Some(value.to_owned());
                }
            }

            fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
                if field.name() == "session_id" {
                    self.0 = Some(format!("{value:?}"));
                }
            }
        }

        let mut visitor = Visitor(None);
        attrs.record(&mut visitor);

        if let (Some(session_id), Some(span)) = (visitor.0, ctx.span(id)) {
            span.extensions_mut().insert(SessionId(session_id));
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let session_id = ctx.lookup_current().and_then(|mut current| {
            loop {
                if let Some(SessionId(id)) = current.extensions().get::<SessionId>() {
                    break Some(id.clone());
                }
                match current.parent() {
                    Some(parent) => current = parent,
                    None => break None,
                }
            }
        });

        let line = format_event(event, session_id.as_deref());

        let mut inner = self.inner.lock();
        let writer = match session_id.as_deref().and_then(|id| inner.sessions.get_mut(id)) {
            Some(sw) => &mut sw.writer,
            None => &mut inner.current_writer.writer,
        };

        let _ = writer.write(line.as_bytes());

        if event.metadata().level() <= &Level::WARN {
            let colored = match *event.metadata().level() {
                Level::ERROR => line.red().bold().to_string(),
                _ => line.yellow().to_string(),
            };
            print!("{colored}");
        }
    }
}

pub fn format_event(event: &Event<'_>, session_id: Option<&str>) -> String {
    let metadata = event.metadata();
    let ts = jiff::Timestamp::now();

    struct Message(Option<String>);

    impl Visit for Message {
        fn record_str(&mut self, f: &Field, v: &str) {
            if f.name() == "message" {
                self.0 = Some(v.to_owned());
            }
        }

        fn record_debug(&mut self, f: &Field, v: &dyn std::fmt::Debug) {
            if f.name() == "message" {
                self.0 = Some(format!("{v:?}"));
            }
        }
    }

    let mut msg = Message(None);
    event.record(&mut msg);
    let message = msg.0.unwrap_or_default();

    struct Fields(Vec<String>);

    impl Visit for Fields {
        fn record_str(&mut self, f: &Field, v: &str) {
            if f.name() != "message" {
                self.0.push(format!("{}=\"{v}\"", f.name()));
            }
        }
        fn record_debug(&mut self, f: &Field, v: &dyn std::fmt::Debug) {
            if f.name() != "message" {
                self.0.push(format!("{}={v:?}", f.name()));
            }
        }
    }

    let mut fields = Fields(Vec::new());
    event.record(&mut fields);

    let sid = session_id.map_or(String::new(), |id| format!("[{id}] "));

    if fields.0.is_empty() {
        format!("{} {:5} {}{} {}\n", ts, metadata.level(), sid, metadata.target(), message,)
    } else {
        format!(
            "{} {:5} {}{} {} {}\n",
            ts,
            metadata.level(),
            sid,
            metadata.target(),
            message,
            fields.0.join(" "),
        )
    }
}
