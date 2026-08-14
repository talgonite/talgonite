//! Aggregates Bevy's per-system trace spans into per-frame CPU totals for the
//! debug console.

#[cfg(feature = "debug")]
use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(feature = "debug")]
use std::fmt;
use std::sync::{Arc, Mutex};
#[cfg(feature = "debug")]
use std::time::Instant;

use bevy::prelude::*;
#[cfg(feature = "debug")]
use tracing::field::{self, Visit};
#[cfg(feature = "debug")]
use tracing::span::{Attributes, Id};
#[cfg(feature = "debug")]
use tracing::{Event, Subscriber};
#[cfg(feature = "debug")]
use tracing_subscriber::layer::{Context, Layer};
#[cfg(feature = "debug")]
use tracing_subscriber::registry::LookupSpan;

use crate::resources::FrameMetrics;

/// Per-system microsecond totals for the current frame.
#[derive(Resource, Clone, Default)]
pub struct SystemTimingShare {
    totals: Arc<Mutex<HashMap<String, u64>>>,
}

impl SystemTimingShare {
    pub fn reset(&self) {
        if let Ok(mut totals) = self.totals.lock() {
            totals.clear();
        }
    }

    #[cfg(feature = "debug")]
    fn add(&self, system: String, us: u64) {
        if let Ok(mut totals) = self.totals.lock() {
            *totals.entry(system).or_insert(0) += us;
        }
    }

    pub fn snapshot_top(&self, n: usize) -> Vec<(String, u64)> {
        let mut rows: Vec<(String, u64)> = self
            .totals
            .lock()
            .map(|t| t.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default();
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        rows.truncate(n);
        rows
    }
}

/// Snapshot the frame's top systems into the console metrics. The share is
/// only populated when the `debug` feature's layer is active.
pub fn collect_system_timings(
    mut metrics: ResMut<FrameMetrics>,
    share: Option<Res<SystemTimingShare>>,
) {
    if let Some(share) = share {
        metrics.last_top_systems = share.snapshot_top(6);
    }
}

#[cfg(feature = "debug")]
thread_local! {
    static ENTERED: RefCell<Vec<(String, Instant)>> = RefCell::new(Vec::new());
}

/// Captures Bevy's `system` spans (from the `trace` feature) and accumulates
/// each system's elapsed time into the shared totals.
#[cfg(feature = "debug")]
pub struct SystemTimingLayer {
    share: SystemTimingShare,
    spans: Mutex<HashMap<Id, String>>,
}

#[cfg(feature = "debug")]
pub fn layer(share: SystemTimingShare) -> SystemTimingLayer {
    SystemTimingLayer {
        share,
        spans: Mutex::new(HashMap::new()),
    }
}

#[cfg(feature = "debug")]
impl SystemTimingLayer {
    fn span_name(attrs: &Attributes<'_>) -> Option<String> {
        if attrs.metadata().name() != "system" {
            return None;
        }
        struct NameVisitor(Option<String>);
        impl Visit for NameVisitor {
            fn record_str(&mut self, field: &field::Field, value: &str) {
                if field.name() == "name" {
                    self.0 = Some(value.to_string());
                }
            }

            fn record_debug(&mut self, _field: &field::Field, _value: &dyn fmt::Debug) {}
        }
        let mut visitor = NameVisitor(None);
        attrs.record(&mut visitor);
        visitor.0
    }
}

#[cfg(feature = "debug")]
impl<S> Layer<S> for SystemTimingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, _ctx: Context<'_, S>) {
        if let Some(name) = Self::span_name(attrs) {
            if let Ok(mut spans) = self.spans.lock() {
                spans.insert(id.clone(), name);
            }
        }
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        if let Ok(mut spans) = self.spans.lock() {
            spans.remove(&id);
        }
    }

    fn on_enter(&self, id: &Id, _ctx: Context<'_, S>) {
        let name = self
            .spans
            .lock()
            .ok()
            .and_then(|spans| spans.get(id).cloned());
        if let Some(name) = name {
            ENTERED.with(|stack| {
                stack.borrow_mut().push((name, Instant::now()));
            });
        }
    }

    fn on_exit(&self, id: &Id, _ctx: Context<'_, S>) {
        let tracked = self
            .spans
            .lock()
            .map(|spans| spans.contains_key(id))
            .unwrap_or(false);
        if !tracked {
            return;
        }
        ENTERED.with(|stack| {
            let mut stack = stack.borrow_mut();
            if let Some((name, start)) = stack.pop() {
                self.share.add(name, start.elapsed().as_micros() as u64);
            }
        });
    }

    fn on_event(&self, _event: &Event<'_>, _ctx: Context<'_, S>) {}
}