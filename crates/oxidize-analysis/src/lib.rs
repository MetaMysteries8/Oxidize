use anyhow::{Context, Result, ensure};
use oxidize_core::{Config, files, fingerprint, process::Runner};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    pub path: String,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
    pub calls: Vec<String>,
    pub dependencies: Vec<String>,
    pub blockers: Vec<String>,
    pub priority_estimate: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParseError {
    pub path: String,
    pub error: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Analysis {
    pub schema_version: u32,
    pub source_fingerprint: String,
    pub languages_by_files: BTreeMap<String, usize>,
    pub manifests: Vec<String>,
    pub entrypoints: Vec<String>,
    pub test_files: Vec<String>,
    pub components: Vec<Component>,
    pub edges: Vec<Edge>,
    pub errors: Vec<ParseError>,
    pub limitations: Vec<String>,
}
#[derive(Deserialize)]
struct PythonAnalysis {
    components: Vec<Component>,
    edges: Vec<Edge>,
    errors: Vec<ParseError>,
}

pub fn analyze(root: &Path, config: &Config) -> Result<Analysis> {
    let paths = files(root)?;
    let mut languages = BTreeMap::new();
    let mut manifests = vec![];
    let mut entrypoints = vec![];
    let mut tests = vec![];
    for path in &paths {
        let language = match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
            "py" => "Python",
            "js" | "mjs" | "cjs" => "JavaScript",
            "ts" | "tsx" => "TypeScript",
            "go" => "Go",
            "rs" => "Rust",
            "c" | "h" => "C",
            "cpp" | "hpp" | "cc" => "C++",
            "java" => "Java",
            _ => continue,
        };
        *languages.entry(language.into()).or_insert(0) += 1;
        if path.file_name().and_then(|x| x.to_str()).is_some_and(|n| {
            n.starts_with("test_") || n.ends_with("_test.go") || n.ends_with(".test.js")
        }) {
            tests.push(path.to_string_lossy().into_owned());
        }
        if path.file_name().and_then(|x| x.to_str()).is_some_and(|n| {
            matches!(
                n,
                "main.py" | "__main__.py" | "main.rs" | "main.go" | "index.js"
            )
        }) {
            entrypoints.push(path.to_string_lossy().into_owned());
        }
    }
    for path in &paths {
        if path.file_name().and_then(|x| x.to_str()).is_some_and(|n| {
            matches!(
                n,
                "Cargo.toml"
                    | "pyproject.toml"
                    | "requirements.txt"
                    | "package.json"
                    | "go.mod"
                    | "Makefile"
            )
        }) {
            manifests.push(path.to_string_lossy().into_owned());
        }
    }
    let python_paths: Vec<_> = paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "py"))
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    // Batch arguments stay bounded even for large repositories. No repository code is run here.
    let mut python = PythonAnalysis {
        components: vec![],
        edges: vec![],
        errors: vec![],
    };
    if !python_paths.is_empty() {
        // Include all paths for cross-file edges; avoid a shell/command-line-sized repository dump.
        ensure!(
            serde_json::to_vec(&python_paths)?.len() <= 100_000,
            "Python path index exceeds 100 KiB; large-repository indexing is not implemented"
        );
        let out = Runner {
            config: &config.execution,
            trust_code: true,
        }
        .run(
            root,
            &[
                config.python.clone(),
                "-I".into(),
                "-c".into(),
                include_str!("analyze.py").into(),
                root.to_string_lossy().into_owned(),
                serde_json::to_string(&python_paths)?,
            ],
        )?;
        out.require_success("Python AST analysis")?;
        python = serde_json::from_str(&out.stdout).context("decode AST analysis")?;
    }
    python.components.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Analysis { schema_version: 1, source_fingerprint: fingerprint(root)?, languages_by_files: languages, manifests, entrypoints, test_files: tests,
        components: python.components, edges: python.edges, errors: python.errors,
        limitations: vec!["Python AST analysis only; other languages are detected, not migrated.".into(), "Call/import graph is static and incomplete for dynamic imports and dispatch.".into(), "Priority scores estimate syntactic cost; no hotspots or speedups are measured by analysis.".into(), "No runtime profiling, service discovery, or verified functionality coverage yet.".into()] })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Plan {
    pub candidates: Vec<Component>,
    pub blocked: Vec<Component>,
    pub limitations: Vec<String>,
}
pub fn plan(analysis: &Analysis) -> Plan {
    let mut remaining: Vec<_> = analysis
        .components
        .iter()
        .filter(|c| c.blockers.is_empty())
        .cloned()
        .collect();
    remaining.sort_by(|a, b| {
        b.priority_estimate
            .cmp(&a.priority_estimate)
            .then(a.id.cmp(&b.id))
    });
    let eligible: BTreeSet<_> = remaining.iter().map(|c| c.id.clone()).collect();
    let mut ordered = vec![];
    let mut done = BTreeSet::new();
    while !remaining.is_empty() {
        let index = remaining.iter().position(|c| {
            c.dependencies
                .iter()
                .all(|d| !eligible.contains(d) || done.contains(d))
        });
        let Some(index) = index else { break }; // Cycles need grouped migration; disclose instead of arbitrary ordering.
        let component = remaining.remove(index);
        done.insert(component.id.clone());
        ordered.push(component);
    }
    for c in &mut remaining {
        c.blockers
            .push("dependency cycle requires grouped migration (not implemented)".into());
    }
    let mut blocked: Vec<_> = analysis
        .components
        .iter()
        .filter(|c| !c.blockers.is_empty())
        .cloned()
        .collect();
    blocked.extend(remaining);
    Plan {
        candidates: ordered,
        blocked,
        limitations: analysis.limitations.clone(),
    }
}
pub fn markdown(analysis: &Analysis, plan: &Plan) -> String {
    let mut out = format!(
        "# Oxidize migration plan\n\nSource fingerprint: `{}`\n\nLanguages (file counts, not functionality coverage): {:?}\n\n## Candidates\n\n",
        analysis.source_fingerprint, analysis.languages_by_files
    );
    for (i, c) in plan.candidates.iter().enumerate() {
        out += &format!(
            "{}. `{}` — PyO3; heuristic priority {} (estimate); dependencies: {:?}\n",
            i + 1,
            c.id,
            c.priority_estimate,
            c.dependencies
        );
    }
    out += "\n## Blocked\n\n";
    for c in &plan.blocked {
        out += &format!("- `{}`: {}\n", c.id, c.blockers.join(", "));
    }
    out += "\n## Limits\n\n";
    for l in &plan.limitations {
        out += &format!("- {l}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn component(id: &str, deps: &[&str], score: i64) -> Component {
        Component {
            id: id.into(),
            path: "a.py".into(),
            name: id.into(),
            start_line: 1,
            end_line: 2,
            signature: "x".into(),
            calls: vec![],
            dependencies: deps.iter().map(|x| (*x).into()).collect(),
            blockers: vec![],
            priority_estimate: score,
        }
    }
    #[test]
    fn dependencies_precede_callers_and_cycles_block() {
        let a = Analysis {
            schema_version: 1,
            source_fingerprint: String::new(),
            languages_by_files: BTreeMap::new(),
            manifests: vec![],
            entrypoints: vec![],
            test_files: vec![],
            components: vec![
                component("caller", &["leaf"], 100),
                component("leaf", &[], 1),
                component("cycle", &["cycle"], 5),
            ],
            edges: vec![],
            errors: vec![],
            limitations: vec![],
        };
        let p = plan(&a);
        assert_eq!(
            p.candidates
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            vec!["leaf", "caller"]
        );
        assert_eq!(p.blocked.len(), 1);
    }
}
