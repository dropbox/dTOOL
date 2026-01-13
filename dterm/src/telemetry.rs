use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryEvent {
    pub name: &'static str,
    pub fields: HashMap<String, String>,
}

impl TelemetryEvent {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            fields: HashMap::new(),
        }
    }

    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
}

pub trait TelemetrySink: Send + Sync {
    fn emit(&self, event: TelemetryEvent);
}

pub struct NoopTelemetry;

impl TelemetrySink for NoopTelemetry {
    fn emit(&self, _event: TelemetryEvent) {}
}

#[cfg(test)]
pub struct VecTelemetry {
    events: Mutex<Vec<TelemetryEvent>>,
}

#[cfg(test)]
impl VecTelemetry {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn events(&self) -> Vec<TelemetryEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl TelemetrySink for VecTelemetry {
    fn emit(&self, event: TelemetryEvent) {
        self.events.lock().unwrap().push(event);
    }
}
