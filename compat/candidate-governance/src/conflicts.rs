use crate::candidate::CandidateSnapshot;
pub enum DependencyKind {
    Api,
    Schema,
    Event,
}
pub struct Dependency {
    pub provider_task: String,
    pub consumer_task: String,
    pub symbol: String,
    pub kind: DependencyKind,
}
pub struct GraphCoverage {
    pub version: String,
    pub required: Vec<String>,
    pub covered: Vec<String>,
    pub dependencies: Vec<Dependency>,
}
pub struct ConflictObservation {
    pub risks: Vec<String>,
    pub missing: Vec<String>,
    pub complete: bool,
    pub unknown: bool,
}
pub fn analyze_impacts(
    snapshots: &[CandidateSnapshot],
    graph: Option<&GraphCoverage>,
) -> ConflictObservation {
    let Some(graph) = graph else {
        return ConflictObservation {
            risks: vec![],
            missing: vec!["graph".into()],
            complete: false,
            unknown: true,
        };
    };
    if graph.version != "gitguard.graph-fixture/v1alpha1" {
        return ConflictObservation {
            risks: vec![],
            missing: graph.required.clone(),
            complete: false,
            unknown: true,
        };
    }
    let tasks = snapshots
        .iter()
        .map(|s| s.task_id())
        .collect::<std::collections::BTreeSet<_>>();
    let mut risks = graph
        .dependencies
        .iter()
        .filter(|d| {
            d.provider_task != d.consumer_task
                && tasks.contains(d.provider_task.as_str())
                && tasks.contains(d.consumer_task.as_str())
        })
        .map(|d| d.symbol.clone())
        .collect::<Vec<_>>();
    risks.sort();
    risks.dedup();
    let mut missing = graph
        .required
        .iter()
        .filter(|c| !graph.covered.contains(c))
        .cloned()
        .collect::<Vec<_>>();
    missing.sort();
    missing.dedup();
    let complete = missing.is_empty();
    ConflictObservation {
        risks,
        missing,
        complete,
        unknown: !complete,
    }
}
