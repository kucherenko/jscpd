//! Two codebases compared function by function (`--compare`): a project and
//! its port to another language, or two implementations of one app, such as
//! the iOS and the Android one. Every function of one side is paired with
//! the functions of the other side that do the same job; what stays unpaired
//! on a side is what the other side lacks.
//!
//! Functions pair up in two steps.
//!
//! 1. The rule of `--semantic` (see [`crate::search`]), between the two
//!    sides only: mutual near-best matches that reach the threshold and stand
//!    out from their backgrounds, where a function's background is the other
//!    side. Functions smaller than `--min-tokens` or `--min-lines` stay out
//!    of this step, as they do in `--semantic`: a short function resembles
//!    too many others.
//! 2. Names. A function the first step left unpaired pairs with an unpaired
//!    function of the other side under the same name, once case and
//!    underscores are ignored (`encodeBinary`, `encode_binary`), when their
//!    similarity reaches the threshold of step 1. Here size does not matter:
//!    a port often makes a function shorter, and a short function is exactly
//!    the one step 1 cannot see. Names repeat across a codebase (`load`,
//!    `checkPermissions` in every plugin), so a name pair has to stay within
//!    the modules step 1 has linked: a module is the folder right under the
//!    deepest folder all files of a side share (`notification` in
//!    `android/notification/…`), and two modules are linked when one of
//!    them holds the most of the other's step-1 pairs, so a single stray
//!    pair links nothing. Two modules step 1 paired nothing in may pair by
//!    name too. Namesakes in two files that step 1 has linked go first, then
//!    the most similar.
//!
//! Only functions of at least `--min-tokens` tokens and `--min-lines` lines
//! (counting the first and the last) count toward a side's totals; a smaller one shows up only as the partner of one that
//! counts. Anonymous functions (callbacks, closures) take no part: they are
//! pieces of the function around them, not something to port on their own.

use crate::search::{
    Embedder, Item, SemanticScope, SemanticUnit, Thresholds, UnitSource, VectorSpace, call_pairs,
    dot, grammar_ids, matched_pairs,
};
use cpd_core::paths::clean_source_id;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Component, Path};

/// Settings of a comparison.
#[derive(Debug, Clone, Copy)]
pub struct CompareParams {
    /// The similarities the rules need, on the scale of the embedder's model.
    pub thresholds: Thresholds,
    /// Functions with fewer detection tokens do not count and do not take
    /// part in step 1.
    pub min_tokens: usize,
    /// Functions of fewer lines, the first and the last included, do not
    /// count and do not take part in step 1.
    pub min_lines: usize,
}

/// A function of one side: `sides[side][source].units[unit]` of the sources
/// given to [`compare`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionRef {
    pub side: usize,
    pub source: usize,
    pub unit: usize,
    /// Whether the function is big enough to count toward its side's totals.
    pub counted: bool,
}

/// How a pair was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchedBy {
    /// The code: step 1 of the module docs.
    Code,
    /// The name, with enough similarity: step 2.
    Name,
}

impl MatchedBy {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchedBy::Code => "code",
            MatchedBy::Name => "name",
        }
    }
}

/// How close a pair's code is, on the scale of the model that scored it:
/// a cosine of 0.6 is high for one model and low for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Below the middle of the pair's threshold and the high bar: related
    /// code pairs here too, so read both functions.
    Low,
    /// Between the two.
    Medium,
    /// At least the group floor of the rules (0.7125 with CodeRankEmbed, 0.8
    /// with jina-embeddings-v2-base-code), and at least the pair's
    /// threshold: almost always the same function.
    High,
}

impl Level {
    /// The level of `similarity` for a pair within one language or across
    /// two, under `bars`.
    pub fn of(similarity: f32, same_language: bool, bars: &Thresholds) -> Self {
        let threshold = bars.for_pair(same_language);
        let high = bars.group_floor.max(threshold);
        if similarity >= high {
            Level::High
        } else if similarity >= (threshold + high) / 2.0 {
            Level::Medium
        } else {
            Level::Low
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Level::Low => "low",
            Level::Medium => "medium",
            Level::High => "high",
        }
    }
}

/// Two functions that do the same job, one on each side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    /// Indexes into [`Comparison::functions`]: `a` on side 0, `b` on side 1.
    pub a: usize,
    pub b: usize,
    /// Cosine similarity of their vectors.
    pub similarity: f32,
    pub level: Level,
    pub matched_by: MatchedBy,
}

/// The result of [`compare`].
#[derive(Debug, Clone, Default)]
pub struct Comparison {
    /// The named functions of both sides, side 0 first, in source order.
    pub functions: Vec<FunctionRef>,
    /// Every pair, in the order of their side-0 function.
    pub pairs: Vec<Pair>,
}

impl Comparison {
    /// Whether the function at `index` of [`Self::functions`] has a partner.
    pub fn paired(&self) -> Vec<bool> {
        let mut paired = vec![false; self.functions.len()];
        for pair in &self.pairs {
            paired[pair.a] = true;
            paired[pair.b] = true;
        }
        paired
    }
}

/// Pair the functions of `sides[0]` with those of `sides[1]`; see the
/// module docs. The two sides must not share a file. Fails only when the
/// embedder does.
pub fn compare(
    sides: [&[UnitSource]; 2],
    embedder: &dyn Embedder,
    params: &CompareParams,
) -> Result<Comparison, String> {
    let flat: Vec<(usize, usize, &UnitSource)> = sides
        .iter()
        .enumerate()
        .flat_map(|(side, sources)| {
            sources
                .iter()
                .enumerate()
                .map(move |(index, source)| (side, index, source))
        })
        .collect();
    let module_names = sides.map(|sources| {
        modules(
            &sources
                .iter()
                .map(|s| clean_source_id(&s.id))
                .collect::<Vec<_>>(),
        )
    });
    let mut files: FxHashMap<(usize, &str), u32> = FxHashMap::default();
    let mut module_ids: FxHashMap<(usize, &str), u32> = FxHashMap::default();
    let mut items = Vec::new();
    let mut module_of = Vec::new();
    let mut functions = Vec::new();
    for (flat_index, &(side, source_index, source)) in flat.iter().enumerate() {
        let next = files.len() as u32;
        let file = *files
            .entry((side, clean_source_id(&source.id)))
            .or_insert(next);
        let next = module_ids.len() as u32;
        let module = *module_ids
            .entry((side, module_names[side][source_index].as_str()))
            .or_insert(next);
        for (unit_index, unit) in source.units.iter().enumerate() {
            if unit.name.starts_with('<') {
                continue;
            }
            module_of.push(module);
            items.push(Item {
                source: flat_index,
                unit: unit_index,
                file,
            });
            functions.push(FunctionRef {
                side,
                source: source_index,
                unit: unit_index,
                counted: (unit.token_count as usize) >= params.min_tokens
                    && (unit.line_span() as usize) + 1 >= params.min_lines,
            });
        }
    }
    let both_sides = [0, 1].map(|side| functions.iter().any(|f| f.side == side));
    if both_sides.contains(&false) {
        return Ok(Comparison {
            functions,
            pairs: Vec::new(),
        });
    }

    let unit = |item: &Item| -> &SemanticUnit { &flat[item.source].2.units[item.unit] };
    let texts: Vec<&str> = items.iter().map(|item| unit(item).text.as_str()).collect();
    let vectors = embedder.embed(&texts)?;
    let space = VectorSpace::new(&vectors, texts.len())?;
    let grammars = grammar_ids(&items, |item| unit(item).grammar);
    let related = call_pairs(&items, unit);

    // Step 1: the rule of --semantic, across the sides, between functions
    // that count.
    let side_of = |i: usize| functions[i].side;
    let rows = space.scan(
        &items,
        &grammars.of_item,
        grammars.count,
        &related,
        |i, j| side_of(i) != side_of(j) && functions[i].counted && functions[j].counted,
    );
    let mut pairs: Vec<Pair> = matched_pairs(
        &rows,
        &grammars.of_item,
        &params.thresholds,
        SemanticScope::All,
    )
    .into_iter()
    .map(|(i, j, similarity)| {
        let (a, b) = if side_of(i) == 0 { (i, j) } else { (j, i) };
        let same_language = grammars.of_item[i] == grammars.of_item[j];
        Pair {
            a,
            b,
            similarity: similarity.min(1.0),
            level: Level::of(similarity, same_language, &params.thresholds),
            matched_by: MatchedBy::Code,
        }
    })
    .collect();

    // Step 2: namesakes among the functions left unpaired.
    let mut paired = vec![false; functions.len()];
    let mut linked_files: FxHashSet<(u32, u32)> = FxHashSet::default();
    // Code pairs per module and module of the other side.
    let mut links: FxHashMap<u32, FxHashMap<u32, usize>> = FxHashMap::default();
    for pair in &pairs {
        paired[pair.a] = true;
        paired[pair.b] = true;
        let (ma, mb) = (module_of[pair.a], module_of[pair.b]);
        *links.entry(ma).or_default().entry(mb).or_default() += 1;
        *links.entry(mb).or_default().entry(ma).or_default() += 1;
        linked_files.insert((items[pair.a].file, items[pair.b].file));
    }
    // Whether `other` holds the most code pairs of `module` (ties count).
    let main_link = |module: u32, other: u32| {
        links.get(&module).is_some_and(|counts| {
            let most = counts.values().copied().max().unwrap_or(0);
            counts.get(&other) == Some(&most)
        })
    };
    let may_pair = |a: usize, b: usize| {
        let (ma, mb) = (module_of[a], module_of[b]);
        main_link(ma, mb)
            || main_link(mb, ma)
            || !(links.contains_key(&ma) || links.contains_key(&mb))
    };
    let mut by_name: FxHashMap<String, [Vec<usize>; 2]> = FxHashMap::default();
    for (i, item) in items.iter().enumerate() {
        let key = name_key(&unit(item).name);
        if !paired[i] && !key.is_empty() {
            by_name.entry(key).or_default()[side_of(i)].push(i);
        }
    }
    let mut by_name: Vec<_> = by_name.into_iter().collect();
    by_name.sort_unstable_by(|x, y| x.0.cmp(&y.0));
    for (_, [left, right]) in by_name {
        let mut candidates: Vec<(bool, f32, usize, usize)> = Vec::new();
        for &a in &left {
            for &b in &right {
                let same_language = grammars.of_item[a] == grammars.of_item[b];
                let similarity = dot(space.row(a), space.row(b));
                let counts = functions[a].counted || functions[b].counted;
                if counts
                    && may_pair(a, b)
                    && similarity >= params.thresholds.for_pair(same_language)
                    && related[a].binary_search(&b).is_err()
                {
                    let files = linked_files.contains(&(items[a].file, items[b].file));
                    candidates.push((files, similarity, a, b));
                }
            }
        }
        // Linked files first, then the most similar; a tie goes to the
        // earlier functions.
        candidates.sort_by(|x, y| {
            (y.0.cmp(&x.0))
                .then(y.1.total_cmp(&x.1))
                .then((x.2, x.3).cmp(&(y.2, y.3)))
        });
        for (_, similarity, a, b) in candidates {
            if paired[a] || paired[b] {
                continue;
            }
            paired[a] = true;
            paired[b] = true;
            let same_language = grammars.of_item[a] == grammars.of_item[b];
            pairs.push(Pair {
                a,
                b,
                similarity: similarity.min(1.0),
                level: Level::of(similarity, same_language, &params.thresholds),
                matched_by: MatchedBy::Name,
            });
        }
    }
    pairs.sort_by_key(|pair| (pair.a, pair.b));
    Ok(Comparison { functions, pairs })
}

/// The module of each of `files`, the paths of one side: the folder right
/// under the deepest folder they all share, or `""` for a file in that
/// folder itself.
fn modules(files: &[&str]) -> Vec<String> {
    let dirs: Vec<Vec<Component<'_>>> = files
        .iter()
        .map(|f| {
            let mut parts: Vec<Component<'_>> = Path::new(f).components().collect();
            parts.pop();
            parts
        })
        .collect();
    let shared = dirs
        .iter()
        .skip(1)
        .fold(dirs.first().map_or(0, Vec::len), |n, dir| {
            n.min(dir.len())
                .min(dirs[0].iter().zip(dir).take_while(|(x, y)| x == y).count())
        });
    dirs.iter()
        .map(|dir| {
            dir.get(shared).map_or(String::new(), |c| {
                c.as_os_str().to_string_lossy().into_owned()
            })
        })
        .collect()
}

/// A function name with case and underscores ignored, so the names one
/// function gets in different languages meet: `encodeBinary`,
/// `encode_binary`, `_encode_binary` and `EncodeBinary` are all
/// `encodebinary`.
pub fn name_key(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::Location;
    use std::collections::HashMap;

    /// A function on lines `line..line + lines` of `tokens` tokens whose
    /// text starts with the key of its vector.
    fn unit(grammar: &'static str, name: &str, line: u32, lines: u32, tokens: u32) -> SemanticUnit {
        SemanticUnit {
            grammar,
            name: name.to_string(),
            start: Location::new(line, 0, line * 100),
            end: Location::new(line + lines, 0, line * 100 + 90),
            range: [line * 10, line * 10 + tokens - 1],
            token_count: tokens,
            text: format!("{name} body"),
        }
    }

    fn source(id: &str, format: &str, units: Vec<SemanticUnit>) -> UnitSource {
        UnitSource {
            id: id.to_string(),
            format: format.to_string(),
            units,
            path_label: Default::default(),
        }
    }

    const PARAMS: CompareParams = CompareParams {
        thresholds: Thresholds::REFERENCE,
        min_tokens: 50,
        min_lines: 5,
    };

    /// Embeds a text as the vector registered for its first word.
    struct Table(HashMap<String, Vec<f32>>);

    impl Embedder for Table {
        fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
            texts
                .iter()
                .map(|t| {
                    let key = t.split_whitespace().next().unwrap_or_default();
                    self.0
                        .get(key)
                        .cloned()
                        .ok_or(format!("no vector for {key}"))
                })
                .collect()
        }
    }

    /// A unit vector along `axis` blended with `noise` of `noise_axis`.
    fn axis_vec(axis: usize, noise_axis: usize, noise: f32) -> Vec<f32> {
        let mut v = vec![0.0; 128];
        v[axis] = 1.0;
        v[noise_axis] += noise;
        v
    }

    /// Unrelated functions on both sides, so every background is big
    /// enough for a z-score.
    fn fillers(side: &str, grammar: &'static str, format: &str) -> Vec<UnitSource> {
        (0..30)
            .map(|k| {
                source(
                    &format!("{side}/filler{k}.x"),
                    format,
                    vec![unit(grammar, &format!("{side}filler{k}"), 1, 9, 60)],
                )
            })
            .collect()
    }

    /// An embedder knowing `named` and the fillers of both sides, which lie
    /// mostly on axes of their own.
    fn table(named: &[(&str, Vec<f32>)]) -> Table {
        let mut table: HashMap<String, Vec<f32>> = named
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (side, offset) in [("java", 64), ("python", 96)] {
            for k in 0..30 {
                let mut v = axis_vec(offset + k, 0, 0.15);
                v[1] = 0.15;
                table.insert(format!("{side}filler{k}"), v);
            }
        }
        Table(table)
    }

    /// The names of each pair, side 0 first, and how it was found.
    fn pair_names<'a>(
        sides: [&'a [UnitSource]; 2],
        result: &Comparison,
    ) -> Vec<(&'a str, &'a str, MatchedBy)> {
        let name = |index: usize| {
            let f = result.functions[index];
            sides[f.side][f.source].units[f.unit].name.as_str()
        };
        result
            .pairs
            .iter()
            .map(|p| (name(p.a), name(p.b), p.matched_by))
            .collect()
    }

    #[test]
    fn a_ported_function_pairs_by_its_code_and_a_missing_one_stays_unpaired() {
        let mut java = vec![source(
            "java/QrCode.java",
            "java",
            vec![
                unit("java", "drawVersion", 10, 20, 120),
                unit("java", "makeKanji", 40, 20, 120),
            ],
        )];
        java.extend(fillers("java", "java", "java"));
        let mut python = vec![source(
            "python/qrcodegen.py",
            "python",
            vec![unit("python", "draw_version_bits", 10, 15, 90)],
        )];
        python.extend(fillers("python", "python", "python"));
        let embedder = table(&[
            ("drawVersion", axis_vec(0, 2, 0.4)),
            ("draw_version_bits", axis_vec(0, 3, 0.5)),
            ("makeKanji", axis_vec(1, 2, 0.1)),
        ]);
        let sides = [java.as_slice(), python.as_slice()];
        let result = compare(sides, &embedder, &PARAMS).unwrap();
        assert_eq!(
            pair_names(sides, &result),
            vec![("drawVersion", "draw_version_bits", MatchedBy::Code)]
        );
        let paired = result.paired();
        let missing: Vec<&str> = result
            .functions
            .iter()
            .zip(&paired)
            .filter(|(f, p)| f.counted && !**p && f.source == 0)
            .map(|(f, _)| sides[f.side][f.source].units[f.unit].name.as_str())
            .collect();
        assert_eq!(missing, vec!["makeKanji"]);
    }

    #[test]
    fn functions_of_one_side_never_pair() {
        let java = vec![
            source("java/A.java", "java", vec![unit("java", "twinA", 1, 9, 60)]),
            source("java/B.java", "java", vec![unit("java", "twinB", 1, 9, 60)]),
        ];
        let python = fillers("python", "python", "python");
        let embedder = table(&[
            ("twinA", axis_vec(0, 1, 0.1)),
            ("twinB", axis_vec(0, 1, 0.2)),
        ]);
        let result = compare([&java, &python], &embedder, &PARAMS).unwrap();
        assert!(result.pairs.is_empty(), "{:?}", result.pairs);
    }

    #[test]
    fn a_short_port_pairs_by_its_name() {
        // The Python version is under --min-tokens, so step 1 does not see
        // it; the name and the similarity pair it.
        let mut java = vec![source(
            "java/QrCode.java",
            "java",
            vec![unit("java", "finderPenaltyAddHistory", 10, 8, 70)],
        )];
        java.extend(fillers("java", "java", "java"));
        let mut python = vec![source(
            "python/qrcodegen.py",
            "python",
            vec![unit("python", "_finder_penalty_add_history", 10, 3, 25)],
        )];
        python.extend(fillers("python", "python", "python"));
        let embedder = table(&[
            ("finderPenaltyAddHistory", axis_vec(0, 2, 0.5)),
            ("_finder_penalty_add_history", axis_vec(0, 3, 0.6)),
        ]);
        let sides = [java.as_slice(), python.as_slice()];
        let result = compare(sides, &embedder, &PARAMS).unwrap();
        assert_eq!(
            pair_names(sides, &result),
            vec![(
                "finderPenaltyAddHistory",
                "_finder_penalty_add_history",
                MatchedBy::Name
            )]
        );
        let python_fn = result.functions[result.pairs[0].b];
        assert!(!python_fn.counted, "a short partner does not count");
    }

    #[test]
    fn namesakes_that_do_different_things_do_not_pair() {
        let java = vec![source(
            "java/Reader.java",
            "java",
            vec![unit("java", "status", 1, 9, 60)],
        )];
        let python = vec![source(
            "python/writer.py",
            "python",
            vec![unit("python", "Status", 1, 9, 60)],
        )];
        let embedder = table(&[
            ("status", axis_vec(0, 2, 0.1)),
            ("Status", axis_vec(1, 3, 0.1)),
        ]);
        let result = compare([&java, &python], &embedder, &PARAMS).unwrap();
        assert!(result.pairs.is_empty(), "{:?}", result.pairs);
    }

    #[test]
    fn namesakes_pair_best_first_and_once() {
        // Two `toString`s per side, the Kotlin ones too short for step 1:
        // each pairs with the one it resembles.
        let java = vec![
            source(
                "java/A.java",
                "java",
                vec![unit("java", "toString", 1, 9, 60)],
            ),
            source(
                "java/B.java",
                "java",
                vec![unit("java", "toString", 20, 9, 60)],
            ),
        ];
        let kotlin = vec![
            source(
                "kt/A.kt",
                "kotlin",
                vec![unit("kotlin", "toString", 1, 2, 20)],
            ),
            source(
                "kt/B.kt",
                "kotlin",
                vec![unit("kotlin", "toString", 20, 2, 20)],
            ),
        ];
        // Vectors by position: the table keys on the first word, so give
        // each function a text of its own.
        let mut sides = [java, kotlin];
        for (s, prefix) in sides.iter_mut().zip(["j", "k"]) {
            for (k, src) in s.iter_mut().enumerate() {
                src.units[0].text = format!("{prefix}{k} body");
            }
        }
        let embedder = table(&[
            ("j0", axis_vec(0, 2, 0.3)),
            ("j1", axis_vec(1, 2, 0.3)),
            ("k0", axis_vec(1, 3, 0.4)),
            ("k1", axis_vec(0, 3, 0.4)),
        ]);
        let result = compare([&sides[0], &sides[1]], &embedder, &PARAMS).unwrap();
        let files: Vec<(&str, &str)> = result
            .pairs
            .iter()
            .map(|p| {
                let file = |i: usize| {
                    let f = result.functions[i];
                    sides[f.side][f.source].id.as_str()
                };
                (file(p.a), file(p.b))
            })
            .collect();
        assert_eq!(
            files,
            vec![("java/A.java", "kt/B.kt"), ("java/B.java", "kt/A.kt")]
        );
    }

    #[test]
    fn anonymous_functions_take_no_part() {
        let java = vec![source(
            "java/A.java",
            "java",
            vec![
                unit("java", "<lambda>", 1, 9, 60),
                unit("java", "run", 20, 9, 60),
            ],
        )];
        let python = vec![source(
            "python/a.py",
            "python",
            vec![unit("python", "<lambda>", 1, 9, 60)],
        )];
        let result = compare([&java, &python], &table(&[]), &PARAMS).unwrap();
        assert_eq!(result.functions.len(), 1);
        assert!(result.pairs.is_empty());
    }

    #[test]
    fn a_namesake_in_another_module_does_not_take_the_pair() {
        // Two plugins per side. `load` pairs by code in both, which links
        // android/camera with ios/camera and android/nfc with ios/nfc; the
        // short `checkPermissions` of the nfc plugin pairs with its own
        // namesake, even though camera's is more similar, and camera's
        // `status`, alone on the Android side, finds no partner in nfc.
        let android = vec![
            source(
                "android/camera/src/Camera.kt",
                "kotlin",
                vec![unit("kotlin", "cameraLoad", 1, 9, 60)],
            ),
            source(
                "android/nfc/src/Nfc.kt",
                "kotlin",
                vec![
                    unit("kotlin", "nfcLoad", 1, 9, 60),
                    unit("kotlin", "checkPermissions", 20, 9, 60),
                ],
            ),
            source(
                "android/camera/src/Status.kt",
                "kotlin",
                vec![unit("kotlin", "status", 1, 9, 60)],
            ),
        ];
        let ios = vec![
            source(
                "ios/camera/Sources/Camera.swift",
                "swift",
                vec![
                    unit("swift", "cameraLoadIos", 1, 9, 60),
                    unit("swift", "checkPermissions", 20, 2, 20),
                ],
            ),
            source(
                "ios/nfc/Sources/Nfc.swift",
                "swift",
                vec![
                    unit("swift", "nfcLoadIos", 1, 9, 60),
                    unit("swift", "checkPermissions", 20, 2, 20),
                    unit("swift", "status", 40, 2, 20),
                ],
            ),
        ];
        let mut sides = [android, ios];
        // Distinct texts for the namesakes, keyed by file.
        for (side, prefix) in sides.iter_mut().zip(["a", "i"]) {
            for (k, src) in side.iter_mut().enumerate() {
                for u in &mut src.units {
                    if matches!(u.name.as_str(), "checkPermissions" | "status") {
                        u.text = format!("{prefix}{k}{} body", u.name);
                    }
                }
            }
        }
        let embedder = table(&[
            ("cameraLoad", axis_vec(0, 5, 0.2)),
            ("cameraLoadIos", axis_vec(0, 6, 0.2)),
            ("nfcLoad", axis_vec(1, 5, 0.2)),
            ("nfcLoadIos", axis_vec(1, 6, 0.2)),
            ("a1checkPermissions", axis_vec(2, 5, 0.2)),
            ("i0checkPermissions", axis_vec(2, 6, 0.1)),
            ("i1checkPermissions", axis_vec(2, 6, 0.6)),
            ("a2status", axis_vec(3, 5, 0.2)),
            ("i1status", axis_vec(3, 6, 0.2)),
        ]);
        let result = compare([&sides[0], &sides[1]], &embedder, &PARAMS).unwrap();
        let described: Vec<(&str, &str, MatchedBy)> = result
            .pairs
            .iter()
            .map(|p| {
                let file = |i: usize| {
                    let f = result.functions[i];
                    sides[f.side][f.source].id.as_str()
                };
                (file(p.a), file(p.b), p.matched_by)
            })
            .collect();
        assert_eq!(
            described,
            vec![
                (
                    "android/camera/src/Camera.kt",
                    "ios/camera/Sources/Camera.swift",
                    MatchedBy::Code
                ),
                (
                    "android/nfc/src/Nfc.kt",
                    "ios/nfc/Sources/Nfc.swift",
                    MatchedBy::Code
                ),
                (
                    "android/nfc/src/Nfc.kt",
                    "ios/nfc/Sources/Nfc.swift",
                    MatchedBy::Name
                ),
            ]
        );
    }

    #[test]
    fn one_stray_code_pair_does_not_link_two_modules_for_names() {
        // android/notify pairs by code twice with ios/notify and once with
        // ios/scan, whose own pairs go mostly to android/scan. The `status`
        // of android/notify has a namesake only in ios/scan, which neither
        // module pairs with most, so the two stay unpaired.
        let android = vec![
            source(
                "android/notify/src/Notify.kt",
                "kotlin",
                vec![
                    unit("kotlin", "show", 1, 9, 60),
                    unit("kotlin", "cancel", 20, 9, 60),
                    unit("kotlin", "permission", 40, 9, 60),
                    unit("kotlin", "status", 60, 9, 60),
                ],
            ),
            source(
                "android/scan/src/Scan.kt",
                "kotlin",
                vec![
                    unit("kotlin", "start", 1, 9, 60),
                    unit("kotlin", "stop", 20, 9, 60),
                ],
            ),
        ];
        let ios = vec![
            source(
                "ios/notify/Sources/Notify.swift",
                "swift",
                vec![
                    unit("swift", "showIos", 1, 9, 60),
                    unit("swift", "cancelIos", 20, 9, 60),
                ],
            ),
            source(
                "ios/scan/Sources/Scan.swift",
                "swift",
                vec![
                    unit("swift", "permissionIos", 1, 9, 60),
                    unit("swift", "Status", 20, 2, 20),
                    unit("swift", "startIos", 40, 9, 60),
                    unit("swift", "stopIos", 60, 9, 60),
                ],
            ),
        ];
        let embedder = table(&[
            ("show", axis_vec(0, 5, 0.2)),
            ("showIos", axis_vec(0, 6, 0.2)),
            ("cancel", axis_vec(1, 5, 0.2)),
            ("cancelIos", axis_vec(1, 6, 0.2)),
            ("permission", axis_vec(2, 5, 0.2)),
            ("permissionIos", axis_vec(2, 6, 0.2)),
            ("status", axis_vec(3, 5, 0.2)),
            ("Status", axis_vec(3, 6, 0.2)),
            ("start", axis_vec(7, 5, 0.2)),
            ("startIos", axis_vec(7, 6, 0.2)),
            ("stop", axis_vec(8, 5, 0.2)),
            ("stopIos", axis_vec(8, 6, 0.2)),
        ]);
        let sides = [android.as_slice(), ios.as_slice()];
        let result = compare(sides, &embedder, &PARAMS).unwrap();
        assert_eq!(
            pair_names(sides, &result),
            vec![
                ("show", "showIos", MatchedBy::Code),
                ("cancel", "cancelIos", MatchedBy::Code),
                ("permission", "permissionIos", MatchedBy::Code),
                ("start", "startIos", MatchedBy::Code),
                ("stop", "stopIos", MatchedBy::Code),
            ]
        );
    }

    #[test]
    fn modules_are_the_folders_under_the_shared_one() {
        assert_eq!(
            modules(&[
                "/r/android/nfc/src/main/Nfc.kt",
                "/r/android/camera/src/Camera.kt",
                "/r/android/Root.kt",
            ]),
            vec!["nfc", "camera", ""]
        );
        assert_eq!(modules(&["/r/java/A.java", "/r/java/B.java"]), vec!["", ""]);
        assert_eq!(modules(&["/r/java/x/A.java"]), vec![""]);
    }

    #[test]
    fn levels_follow_the_model_scale() {
        // CodeRankEmbed: 0.4125 across, 0.6375 within, group floor 0.7125.
        let bars = Thresholds {
            across: 0.4125,
            within: 0.6375,
            near_best: 0.075,
            group_floor: 0.7125,
        };
        let level = |s, same| Level::of(s, same, &bars);
        assert_eq!(level(0.45, false), Level::Low);
        assert_eq!(level(0.5625, false), Level::Medium);
        assert_eq!(level(0.69, false), Level::Medium);
        assert_eq!(level(0.7125, false), Level::High);
        // Within one language the middle moves up with the threshold.
        assert_eq!(level(0.66, true), Level::Low);
        assert_eq!(level(0.68, true), Level::Medium);
        assert_eq!(level(0.9, true), Level::High);
        // jina-embeddings-v2-base-code: the group floor is 0.8.
        assert_eq!(
            Level::of(0.75, false, &Thresholds::REFERENCE),
            Level::Medium
        );
        assert_eq!(Level::of(0.8, false, &Thresholds::REFERENCE), Level::High);
    }

    #[test]
    fn name_keys_ignore_case_and_underscores() {
        for name in [
            "encodeBinary",
            "encode_binary",
            "_encode_binary",
            "EncodeBinary",
        ] {
            assert_eq!(name_key(name), "encodebinary");
        }
        assert_eq!(name_key("__init__"), "init");
    }
}
