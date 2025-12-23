//! Data flow graph construction for visualizing producer/consumer relationships.

use std::collections::{HashMap, HashSet};

use super::monitor::MonitorData;

/// Data flow graph showing producer/consumer relationships.
///
/// Used by the Flow view to display which modules communicate
/// with each other via message bus topics.
#[derive(Debug, Clone)]
pub struct DataFlowGraph {
    /// Mapping of topic -> modules that write to it.
    pub producers: HashMap<String, Vec<String>>,
    /// Mapping of topic -> modules that read from it.
    pub consumers: HashMap<String, Vec<String>>,
    /// All unique topics, sorted alphabetically.
    pub topics: Vec<String>,
}

impl DataFlowGraph {
    /// Build a flow graph from monitor data.
    ///
    /// Extracts all read/write relationships and organizes them
    /// by topic for efficient lookup.
    pub fn from_monitor_data(data: &MonitorData) -> Self {
        let mut producers: HashMap<String, Vec<String>> = HashMap::new();
        let mut consumers: HashMap<String, Vec<String>> = HashMap::new();
        let mut topic_set: HashSet<String> = HashSet::new();

        for module in &data.modules {
            for write in &module.writes {
                topic_set.insert(write.topic.clone());
                producers
                    .entry(write.topic.clone())
                    .or_default()
                    .push(module.name.clone());
            }
            for read in &module.reads {
                topic_set.insert(read.topic.clone());
                consumers
                    .entry(read.topic.clone())
                    .or_default()
                    .push(module.name.clone());
            }
        }

        let mut topics: Vec<String> = topic_set.into_iter().collect();
        topics.sort();

        Self {
            producers,
            consumers,
            topics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::monitor::{HealthStatus, ModuleData, TopicRead, TopicWrite};
    use std::time::Instant;

    fn make_module(name: &str, reads: Vec<&str>, writes: Vec<&str>) -> ModuleData {
        ModuleData {
            name: name.to_string(),
            reads: reads
                .into_iter()
                .map(|t| TopicRead {
                    topic: t.to_string(),
                    read: 0,
                    pending_for: None,
                    unread: None,
                    status: HealthStatus::Healthy,
                })
                .collect(),
            writes: writes
                .into_iter()
                .map(|t| TopicWrite {
                    topic: t.to_string(),
                    written: 0,
                    pending_for: None,
                    status: HealthStatus::Healthy,
                })
                .collect(),
            total_read: 0,
            total_written: 0,
            health: HealthStatus::Healthy,
        }
    }

    fn make_monitor_data(modules: Vec<ModuleData>) -> MonitorData {
        MonitorData {
            modules,
            last_updated: Instant::now(),
        }
    }

    #[test]
    fn empty_data_produces_empty_graph() {
        let data = make_monitor_data(vec![]);
        let graph = DataFlowGraph::from_monitor_data(&data);

        assert!(graph.producers.is_empty());
        assert!(graph.consumers.is_empty());
        assert!(graph.topics.is_empty());
    }

    #[test]
    fn single_producer_tracked() {
        let data = make_monitor_data(vec![make_module("api", vec![], vec!["events"])]);
        let graph = DataFlowGraph::from_monitor_data(&data);

        assert_eq!(graph.topics, vec!["events"]);
        assert_eq!(graph.producers.get("events").unwrap(), &vec!["api"]);
        assert!(!graph.consumers.contains_key("events"));
    }

    #[test]
    fn single_consumer_tracked() {
        let data = make_monitor_data(vec![make_module("worker", vec!["events"], vec![])]);
        let graph = DataFlowGraph::from_monitor_data(&data);

        assert_eq!(graph.topics, vec!["events"]);
        assert!(!graph.producers.contains_key("events"));
        assert_eq!(graph.consumers.get("events").unwrap(), &vec!["worker"]);
    }

    #[test]
    fn producer_consumer_pipeline() {
        let data = make_monitor_data(vec![
            make_module("api", vec![], vec!["orders"]),
            make_module("processor", vec!["orders"], vec!["notifications"]),
            make_module("notifier", vec!["notifications"], vec![]),
        ]);
        let graph = DataFlowGraph::from_monitor_data(&data);

        assert_eq!(graph.topics, vec!["notifications", "orders"]);

        assert_eq!(graph.producers.get("orders").unwrap(), &vec!["api"]);
        assert_eq!(graph.consumers.get("orders").unwrap(), &vec!["processor"]);

        assert_eq!(
            graph.producers.get("notifications").unwrap(),
            &vec!["processor"]
        );
        assert_eq!(
            graph.consumers.get("notifications").unwrap(),
            &vec!["notifier"]
        );
    }

    #[test]
    fn multiple_producers_same_topic() {
        let data = make_monitor_data(vec![
            make_module("api-1", vec![], vec!["events"]),
            make_module("api-2", vec![], vec!["events"]),
        ]);
        let graph = DataFlowGraph::from_monitor_data(&data);

        let producers = graph.producers.get("events").unwrap();
        assert_eq!(producers.len(), 2);
        assert!(producers.contains(&"api-1".to_string()));
        assert!(producers.contains(&"api-2".to_string()));
    }

    #[test]
    fn multiple_consumers_same_topic() {
        let data = make_monitor_data(vec![
            make_module("worker-1", vec!["events"], vec![]),
            make_module("worker-2", vec!["events"], vec![]),
        ]);
        let graph = DataFlowGraph::from_monitor_data(&data);

        let consumers = graph.consumers.get("events").unwrap();
        assert_eq!(consumers.len(), 2);
        assert!(consumers.contains(&"worker-1".to_string()));
        assert!(consumers.contains(&"worker-2".to_string()));
    }

    #[test]
    fn topics_are_sorted_alphabetically() {
        let data = make_monitor_data(vec![make_module(
            "m",
            vec!["zebra", "alpha", "middle"],
            vec![],
        )]);
        let graph = DataFlowGraph::from_monitor_data(&data);

        assert_eq!(graph.topics, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn module_can_be_both_producer_and_consumer() {
        let data = make_monitor_data(vec![make_module(
            "transformer",
            vec!["input"],
            vec!["output"],
        )]);
        let graph = DataFlowGraph::from_monitor_data(&data);

        assert_eq!(graph.topics.len(), 2);
        assert_eq!(graph.consumers.get("input").unwrap(), &vec!["transformer"]);
        assert_eq!(graph.producers.get("output").unwrap(), &vec!["transformer"]);
    }
}
