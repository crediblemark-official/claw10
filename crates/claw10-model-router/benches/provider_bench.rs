use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn create_test_json() -> String {
    let mut obj = serde_json::Map::new();
    let mut models = vec![];
    for i in 0..100 {
        models.push(serde_json::Value::String(format!("model-{}", i)));
    }

    for p in 0..10 {
        obj.insert(format!("provider-{}", p), serde_json::Value::Array(models.clone()));
    }

    serde_json::to_string(&serde_json::Value::Object(obj)).unwrap()
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let json_str = create_test_json();

    c.bench_function("parse_profiles_original", |b| {
        b.iter(|| {
            let mut profiles = Vec::new();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(obj) = json.as_object() {
                    for (provider_name, models_arr) in obj {
                        if let Some(arr) = models_arr.as_array() {
                            for val in arr {
                                if let Some(model_id) = val.as_str() {
                                    profiles.push(claw10_model_router::types::ModelProfile {
                                        id: model_id.to_string(),
                                        provider: provider_name.clone(),
                                        model_name: model_id.to_string(),
                                        context_window: 128_000,
                                        max_output_tokens: 8_192,
                                        cost_per_1m_input: 0.0,
                                        cost_per_1m_output: 0.0,
                                        suitable_for: vec!["general".to_string()],
                                    });
                                }
                            }
                        }
                    }
                }
            }
            black_box(profiles)
        })
    });

    c.bench_function("parse_profiles_optimized_move_suitable_and_arc_string", |b| {
        b.iter(|| {
            let mut profiles = Vec::new();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(obj) = json.as_object() {
                    let suitable_for = vec!["general".to_string()];
                    for (provider_name, models_arr) in obj {
                        if let Some(arr) = models_arr.as_array() {
                            let provider_name_arc: std::sync::Arc<str> = provider_name.clone().into();
                            for val in arr {
                                if let Some(model_id) = val.as_str() {
                                    profiles.push(claw10_model_router::types::ModelProfile {
                                        id: model_id.to_string(),
                                        provider: provider_name_arc.to_string(), // Still string conversion
                                        model_name: model_id.to_string(),
                                        context_window: 128_000,
                                        max_output_tokens: 8_192,
                                        cost_per_1m_input: 0.0,
                                        cost_per_1m_output: 0.0,
                                        suitable_for: suitable_for.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            black_box(profiles)
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
