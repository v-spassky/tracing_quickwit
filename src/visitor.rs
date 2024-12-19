use tracing::field::{Field, Visit};

pub(crate) struct TargetFieldVisitor {
    pub(crate) target_field: String,
    pub(crate) target_value: Option<String>,
}

impl TargetFieldVisitor {
    pub(crate) fn new(target_field: &str) -> Self {
        Self {
            target_field: target_field.to_string(),
            target_value: None,
        }
    }
}

impl Visit for TargetFieldVisitor {
    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == self.target_field {
            self.target_value = Some(value.to_string());
        }
    }
}

pub(crate) struct LogVisitor {
    pub(crate) log: serde_json::Map<String, serde_json::Value>,
}

impl LogVisitor {
    pub(crate) fn new() -> Self {
        Self {
            log: serde_json::Map::new(),
        }
    }
}

impl Visit for LogVisitor {
    // TODO: Implement all methods defined in `Visit`.
    // TODO: Should we serialize numbers into strings?

    fn record_str(&mut self, field: &Field, value: &str) {
        self.log.insert(field.name().to_string(), value.into());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.log.insert(field.name().to_string(), value.into());
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        // TODO: This isn't ideal and must be documented (Quickwit uses `u64`).
        self.log
            .insert(field.name().to_string(), (value as u64).into());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.log
            .insert(field.name().to_string(), format!("{:?}", value).into());
    }
}
