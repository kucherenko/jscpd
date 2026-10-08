// Similarity as a user of the crate sees it: units extracted from source,
// signed under a policy, and the pairs the search reports.

use cpd_core::detect::PathFilters;
use cpd_core::models::{CpdClone, Location};
use cpd_similarity::functions::{extract_units, signatures};
use cpd_similarity::{
    CodeSize, FunctionSig, FunctionSource, SignaturePolicy, SimilarityDecorators,
    SimilarityIdentifiers, SimilarityIndex, SimilarityLiterals, bag_jaccard,
    find_similar_functions,
};
use cpd_tokenizer::tokenizer::{Mode, TokenizeOptions, tokenize_to_detection};

const ANY_SIZE: CodeSize = CodeSize {
    tokens: 0,
    lines: 0,
};

fn spans(src: &str, format: &str) -> Vec<(Location, Location)> {
    let options = TokenizeOptions::new(Mode::Mild);
    tokenize_to_detection(format, src, &options)
        .iter()
        .map(|t| (t.start.clone(), t.end.clone()))
        .collect()
}

/// The signatures of every unit of `src` under `policy`.
fn units(src: &str, format: &str, policy: SignaturePolicy) -> Vec<FunctionSig> {
    signatures(
        extract_units(src, format, ANY_SIZE, &[]),
        &spans(src, format),
        policy,
    )
}

/// The signature of the unit called `name` in `src`.
fn unit(src: &str, format: &str, name: &str, policy: SignaturePolicy) -> FunctionSig {
    units(src, format, policy)
        .into_iter()
        .find(|u| u.name == name)
        .unwrap_or_else(|| panic!("no unit {name} in {src}"))
}

/// The similarity of the units called `name` in `a` and `b`.
fn score(a: &str, b: &str, format: &str, name: &str, policy: SignaturePolicy) -> f32 {
    bag_jaccard(
        &unit(a, format, name, policy).shingles,
        &unit(b, format, name, policy).shingles,
    )
}

fn source(id: &str, src: &str, format: &str, policy: SignaturePolicy) -> FunctionSource {
    FunctionSource {
        id: id.into(),
        format: format.into(),
        real_path: String::new(),
        functions: units(src, format, policy),
    }
}

/// The pairs reported among `files` at `threshold`, by file and score.
fn pairs(files: &[(&str, &str)], format: &str, threshold: f32) -> Vec<(String, String, f32)> {
    let sources = files
        .iter()
        .map(|(id, src)| source(id, src, format, SignaturePolicy::default()))
        .collect();
    describe(&find_similar_functions(
        sources,
        threshold,
        10,
        1,
        &[],
        &PathFilters::default(),
    ))
}

fn describe(clones: &[CpdClone]) -> Vec<(String, String, f32)> {
    clones
        .iter()
        .map(|c| {
            (
                c.fragment_a.source_id.clone(),
                c.fragment_b.source_id.clone(),
                c.similarity.unwrap(),
            )
        })
        .collect()
}

fn identifiers(mode: SimilarityIdentifiers) -> SignaturePolicy {
    SignaturePolicy {
        identifiers: mode,
        ..SignaturePolicy::default()
    }
}

/// A function that loads through `store` with `method`, its own names
/// spelled `item`.
fn sync(method: &str, item: &str) -> String {
    format!(
        "function sync(store, ids) {{\n  const out = [];\n  for (const {item} of ids) {{\n    const row = store.{method}({item});\n    if (row) {{\n      out.push(row.value);\n    }}\n  }}\n  return out;\n}}\n"
    )
}

#[test]
fn renamed_variables_never_matter_and_called_methods_matter_only_role_aware() {
    use SimilarityIdentifiers::*;
    let (load, renamed, save) = (sync("load", "id"), sync("load", "key"), sync("save", "id"));
    for mode in [Ignore, RoleAware] {
        assert_eq!(
            score(&load, &renamed, "javascript", "sync", identifiers(mode)),
            1.0,
            "{mode:?}"
        );
    }
    assert_eq!(
        score(&load, &save, "javascript", "sync", identifiers(Ignore)),
        1.0
    );
    let role_aware = score(&load, &save, "javascript", "sync", identifiers(RoleAware));
    assert!(role_aware > 0.0 && role_aware < 1.0, "{role_aware}");
}

#[test]
fn role_aware_python_tells_called_methods_apart() {
    use SimilarityIdentifiers::*;
    let body = |method: &str| {
        format!(
            "def sync(store, ids):\n    out = []\n    for i in ids:\n        row = store.{method}(i)\n        if row:\n            out.append(row.value)\n    return out\n"
        )
    };
    let (load, save) = (body("load"), body("save"));
    assert_eq!(
        score(&load, &save, "python", "sync", identifiers(Ignore)),
        1.0
    );
    assert!(score(&load, &save, "python", "sync", identifiers(RoleAware)) < 1.0);
}

/// A NestJS-like controller whose method has the decorator `decorator`.
fn controller(decorator: &str) -> String {
    format!(
        "class Users {{\n  {decorator}\n  find(id: string) {{\n    const user = this.users.get(id);\n    if (!user) {{\n      throw new NotFound(id);\n    }}\n    return user;\n  }}\n}}\n"
    )
}

#[test]
fn decorators_count_by_name_or_whole_as_the_mode_says() {
    use SimilarityDecorators::*;
    let policy = |decorators| SignaturePolicy {
        decorators,
        literals: SimilarityLiterals::Values,
        ..SignaturePolicy::default()
    };
    let get = controller("@Get(':id')");
    let post = controller("@Post(':id')");
    let other_route = controller("@Get(':name')");
    let bare = controller("");
    let class = |a: &str, b: &str, mode| score(a, b, "typescript", "Users", policy(mode));

    assert_eq!(class(&get, &post, Omit), 1.0, "decorators are left out");
    assert_eq!(class(&get, &bare, Omit), 1.0);
    assert!(class(&get, &post, Names) < 1.0, "by name, Get is not Post");
    assert_eq!(
        class(&get, &other_route, Names),
        1.0,
        "by name, the arguments do not count"
    );
    assert!(class(&get, &bare, Names) < 1.0);
    assert!(
        class(&get, &other_route, Full) < 1.0,
        "whole, the arguments count as the literal mode says"
    );
    assert_eq!(class(&get, &get, Full), 1.0);
}

#[test]
fn python_decorators_count_as_the_mode_says() {
    use SimilarityDecorators::*;
    let def = |decorator: &str| {
        format!(
            "{decorator}\ndef handler(request):\n    user = request.user\n    if not user:\n        return deny(request)\n    return render(request, user)\n"
        )
    };
    let policy = |decorators| SignaturePolicy {
        decorators,
        ..SignaturePolicy::default()
    };
    let (cached, logged) = (def("@cache"), def("@log"));
    let (indexed, called) = (def("@registry[\"x\"]"), def("@(lambda f: f)"));
    let s = |a: &str, b: &str, mode| score(a, b, "python", "handler", policy(mode));
    assert_eq!(s(&cached, &logged, Omit), 1.0);
    assert!(s(&cached, &logged, Names) < 1.0);
    assert_eq!(
        s(&indexed, &def("@registry[\"y\"]"), Names),
        1.0,
        "a subscript goes by what it indexes"
    );
    assert!(s(&indexed, &called, Names) < 1.0);
}

#[test]
fn the_threshold_is_inclusive() {
    let a = sync("load", "id");
    let b = "function sync(store, ids) {\n  const out = [];\n  for (const id of ids) {\n    const row = store.load(id);\n    if (row && row.ok) {\n      out.push(row.value);\n    }\n  }\n  return out;\n}\n";
    let files = [("a.js", a.as_str()), ("b.js", b)];
    let found = pairs(&files, "javascript", 0.5);
    assert_eq!(found.len(), 1, "{found:?}");
    let similarity = found[0].2;
    assert!(similarity < 1.0, "an edited copy: {similarity}");
    assert_eq!(
        pairs(&files, "javascript", similarity).len(),
        1,
        "a pair scoring the threshold is reported"
    );
    assert!(
        pairs(&files, "javascript", similarity.next_up()).is_empty(),
        "and one just below it is not"
    );
}

#[test]
fn a_pair_scores_the_same_whichever_side_asks() {
    let a = sync("load", "id");
    let b = "function sync(store, ids) {\n  const out = [];\n  for (const id of ids) {\n    const row = store.load(id);\n    out.push(row.value);\n  }\n  return out;\n}\n";
    let a_src = source("a.js", &a, "javascript", SignaturePolicy::default());
    let b_src = source("b.js", b, "javascript", SignaturePolicy::default());
    let one_way =
        SimilarityIndex::build(vec![a_src.clone()], 10, 1).query(&b_src.functions[0], 0.1);
    let other_way =
        SimilarityIndex::build(vec![b_src.clone()], 10, 1).query(&a_src.functions[0], 0.1);
    assert_eq!(one_way.len(), 1);
    assert_eq!(one_way[0].2, other_way[0].2);
    let forward = describe(&find_similar_functions(
        vec![a_src.clone(), b_src.clone()],
        0.1,
        10,
        1,
        &[],
        &PathFilters::default(),
    ));
    let backward = describe(&find_similar_functions(
        vec![b_src, a_src],
        0.1,
        10,
        1,
        &[],
        &PathFilters::default(),
    ));
    assert_eq!(forward, backward);
    assert_eq!(forward[0].2, one_way[0].2);
}

#[test]
fn empty_and_tiny_sources_make_no_pairs() {
    assert!(units("", "javascript", SignaturePolicy::default()).is_empty());
    assert!(units("", "python", SignaturePolicy::default()).is_empty());
    assert!(
        units(
            "// only a comment\n",
            "javascript",
            SignaturePolicy::default()
        )
        .is_empty()
    );
    assert!(pairs(&[("a.js", ""), ("b.js", "")], "javascript", 0.1).is_empty());
    assert!(find_similar_functions(vec![], 0.1, 0, 0, &[], &PathFilters::default()).is_empty());
    // Identical tiny functions pair only when the size limits let them.
    let tiny = "function f() {}\n";
    let found = |min_tokens| {
        describe(&find_similar_functions(
            vec![
                source("a.js", tiny, "javascript", SignaturePolicy::default()),
                source("b.js", tiny, "javascript", SignaturePolicy::default()),
            ],
            0.5,
            min_tokens,
            0,
            &[],
            &PathFilters::default(),
        ))
    };
    assert!(found(10).is_empty(), "{:?}", found(10));
    assert_eq!(
        found(0),
        [("a.js".to_string(), "b.js".to_string(), 1.0)],
        "without limits, identical copies score 1"
    );
}

#[test]
fn copies_in_a_language_without_an_extractor_have_no_units() {
    let src = "fn main() { let x = 1; println!(\"{x}\"); }\n";
    assert!(units(src, "rust", SignaturePolicy::default()).is_empty());
}
