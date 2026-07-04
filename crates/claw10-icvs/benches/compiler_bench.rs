use criterion::{black_box, criterion_group, criterion_main, Criterion};
use claw10_icvs::IcvsCompiler;

fn bench_parse(c: &mut Criterion) {
    let source = r#"
    [node:rule1]
    type = "Rule"
    severity = "Must"
    content = "Test rule 1"

    [node:rule2]
    type = "Rule"
    severity = "Must"
    content = "Test rule 2"

    [node:rule3]
    type = "Rule"
    severity = "Should"
    content = "Test rule 3"

    [edge:rule1->rule2]
    label = "dependency"

    [edge:rule2->rule3]
    label = "dependency"

    [target:agent1]
    resolve = ["rule1", "rule2"]
    "#;

    c.bench_function("parse", |b| b.iter(|| IcvsCompiler::parse(black_box(source))));
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
