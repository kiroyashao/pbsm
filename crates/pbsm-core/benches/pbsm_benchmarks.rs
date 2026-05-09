use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use pbsm_core::event_bus::SystemEventBus;
use pbsm_core::modules::belief_graph::graph::BeliefGraph;
use pbsm_core::modules::belief_graph::operations::BeliefGraphOperations;
use pbsm_core::modules::belief_graph::types::{BeliefNodeType, GraphConfig, SourceType};
use pbsm_core::modules::common::PredictionEvent;
use pbsm_core::modules::metacognition::controller::MetacognitiveController;
use pbsm_core::modules::prediction_engine::PredictionEngine;
use pbsm_core::orchestrator::{PbsmConfig, PbsmOrchestrator};
use std::collections::HashMap;
use std::sync::Arc;

fn bench_belief_graph_create_belief(c: &mut Criterion) {
    let mut group = c.benchmark_group("belief_graph/create_belief");
    for size in [10u64, 100, 500] {
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let config = GraphConfig {
                    max_nodes: 1000,
                    max_edges: 5000,
                    ..Default::default()
                };
                let graph = BeliefGraph::new(config);
                for i in 0..size {
                    let _ = BeliefGraphOperations::create_belief(
                        black_box(&graph),
                        BeliefNodeType::Concept,
                        format!("node_{}", i),
                        HashMap::new(),
                        "bench".to_string(),
                        SourceType::DirectObservation,
                        Some(vec![format!("tag_{}", i % 10)]),
                        None,
                    );
                }
            });
        });
    }
    group.finish();
}

fn bench_belief_graph_query_by_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("belief_graph/query_by_type");
    for size in [10u64, 100, 500] {
        let config = GraphConfig {
            max_nodes: 1000,
            max_edges: 5000,
            ..Default::default()
        };
        let graph = BeliefGraph::new(config);
        for i in 0..size {
            let _ = BeliefGraphOperations::create_belief(
                &graph,
                BeliefNodeType::Concept,
                format!("node_{}", i),
                HashMap::new(),
                "bench".to_string(),
                SourceType::DirectObservation,
                Some(vec![format!("tag_{}", i % 10)]),
                None,
            );
        }
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let indexes = graph.indexes_mut().read();
                let _result = indexes.query_by_type(black_box(BeliefNodeType::Concept));
            });
        });
    }
    group.finish();
}

fn bench_belief_graph_query_by_tag(c: &mut Criterion) {
    let mut group = c.benchmark_group("belief_graph/query_by_tag");
    for size in [10u64, 100, 500] {
        let config = GraphConfig {
            max_nodes: 1000,
            max_edges: 5000,
            ..Default::default()
        };
        let graph = BeliefGraph::new(config);
        for i in 0..size {
            let _ = BeliefGraphOperations::create_belief(
                &graph,
                BeliefNodeType::Concept,
                format!("node_{}", i),
                HashMap::new(),
                "bench".to_string(),
                SourceType::DirectObservation,
                Some(vec![format!("tag_{}", i % 10)]),
                None,
            );
        }
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let indexes = graph.indexes_mut().read();
                let _result = indexes.query_by_tag(black_box("tag_5"));
            });
        });
    }
    group.finish();
}

fn bench_event_bus_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/publish");
    for capacity in [64, 256, 1024] {
        group.bench_with_input(
            BenchmarkId::new("capacity", capacity),
            &capacity,
            |b, &cap| {
                b.iter(|| {
                    let bus = SystemEventBus::new(cap);
                    let event = PredictionEvent::PredictionCreated(
                        pbsm_core::modules::common::PredictionCreatedPayload {
                            prediction_id: uuid::Uuid::new_v4(),
                            action_type: pbsm_core::types::prediction::ActionType::ToolCall,
                            target_node: Some("test_target".to_string()),
                            expected_change_count: 3,
                        },
                    );
                    let _ = bus.publish(black_box(pbsm_core::event_bus::SystemEvent::Prediction(
                        event,
                    )));
                });
            },
        );
    }
    group.finish();
}

fn bench_event_bus_subscribe_and_receive(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/subscribe_receive");
    group.bench_function("1_publisher_1_subscriber", |b| {
        b.iter(|| {
            let bus = Arc::new(SystemEventBus::new(256));
            let mut rx = bus.subscribe();
            let event = PredictionEvent::PredictionCreated(
                pbsm_core::modules::common::PredictionCreatedPayload {
                    prediction_id: uuid::Uuid::new_v4(),
                    action_type: pbsm_core::types::prediction::ActionType::ToolCall,
                    target_node: None,
                    expected_change_count: 1,
                },
            );
            let _ = bus.publish(pbsm_core::event_bus::SystemEvent::Prediction(event));
            let _ = rx.try_recv();
        });
    });
    group.finish();
}

fn bench_metacognitive_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("metacognitive/attention");
    let rt = tokio::runtime::Runtime::new().unwrap();
    group.bench_function("get_attention_status", |b| {
        let controller = MetacognitiveController::new();
        b.to_async(&rt).iter(|| async {
            let _status = controller.get_attention_status().await;
        });
    });
    group.finish();
}

fn bench_metacognitive_anomaly_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("metacognitive/anomaly_detection");
    group.bench_function("detect_anomalies_default", |b| {
        let controller = MetacognitiveController::new();
        b.iter(|| {
            let _report = controller.detect_anomalies(black_box(None));
        });
    });
    group.finish();
}

fn bench_orchestrator_execute_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("orchestrator/execute_cycle");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = PbsmConfig::default();
    let orchestrator = PbsmOrchestrator::new(config);
    group.bench_function("default_config", |b| {
        b.to_async(&rt).iter(|| async {
            let _result = orchestrator.execute_cycle().await;
        });
    });
    group.finish();
}

fn bench_orchestrator_start_task(c: &mut Criterion) {
    let mut group = c.benchmark_group("orchestrator/start_task");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = PbsmConfig::default();
    let orchestrator = Arc::new(PbsmOrchestrator::new(config));
    group.bench_function("default_config", |b| {
        b.to_async(&rt).iter(|| {
            let orch = orchestrator.clone();
            async move {
                let _result = orch.start_task("benchmark task".to_string(), None).await;
            }
        });
    });
    group.finish();
}

fn bench_prediction_engine_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("prediction_engine/create_prediction");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let engine = Arc::new(PredictionEngine::new());
    group.bench_function("default_engine", |b| {
        b.to_async(&rt).iter(|| {
            let eng = engine.clone();
            async move {
                let request = pbsm_core::types::prediction::ActionRequest {
                    action_type: pbsm_core::types::prediction::ActionType::ToolCall,
                    action_name: "benchmark_action".to_string(),
                    parameters: serde_json::json!({"key": "value"}),
                    target_id: Some("target_1".to_string()),
                };
                let _ = eng.create_prediction(request, None).await;
            }
        });
    });
    group.finish();
}

fn bench_config_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("config/serialization");
    let config = PbsmConfig::default();
    group.bench_function("to_json", |b| {
        b.iter(|| {
            let _json = serde_json::to_string(black_box(&config)).unwrap();
        });
    });
    group.bench_function("to_toml", |b| {
        b.iter(|| {
            let _toml = toml::to_string(black_box(&config)).unwrap();
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_belief_graph_create_belief,
    bench_belief_graph_query_by_type,
    bench_belief_graph_query_by_tag,
    bench_event_bus_publish,
    bench_event_bus_subscribe_and_receive,
    bench_metacognitive_attention,
    bench_metacognitive_anomaly_detection,
    bench_orchestrator_execute_cycle,
    bench_orchestrator_start_task,
    bench_prediction_engine_create,
    bench_config_serialization,
);
criterion_main!(benches);
