use camino::Utf8Path;
use camino::Utf8PathBuf;
use miette::miette;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

/// Topologically sort a set of paths.
///
/// If there are two paths `x` and `y` in the input where `x` contains `y` (e.g. `x` is `/puppy`
/// and `y` is `/puppy/doggy`), then there is an edge from `y` to `x`.
///
/// This function errors if any input path is relative.
///
/// This implements Kahn's algorithm.
///
/// See: <https://en.wikipedia.org/wiki/Topological_sorting#Kahn's_algorithm>
pub fn topological_sort<P>(paths: &[P]) -> miette::Result<Vec<Utf8PathBuf>>
where
    P: AsRef<Utf8Path>,
{
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    // Compute edges.
    let mut edges = FxHashMap::<&Utf8Path, FxHashSet<&Utf8Path>>::default();
    let mut incoming_edges = FxHashMap::<&Utf8Path, FxHashSet<&Utf8Path>>::default();
    for (i, path1) in paths[..paths.len()].iter().enumerate() {
        let path1 = path1.as_ref();
        if path1.is_relative() {
            return Err(miette!("Path is relative: {path1}"));
        }

        for path2 in &paths[i + 1..] {
            let path2 = path2.as_ref();

            if path1 == path2 {
                // Fucked up.
                tracing::warn!("Duplicate paths: {path1}");
                continue;
            }

            if path1.starts_with(path2) {
                edges.entry(path1).or_default().insert(path2);
                incoming_edges.entry(path2).or_default().insert(path1);
            } else if path2.starts_with(path1) {
                edges.entry(path2).or_default().insert(path1);
                incoming_edges.entry(path1).or_default().insert(path2);
            }
        }
    }

    // The inner loop above doesn't hit the last path, so we check if it's relative here.
    if let Some(path) = paths.last() {
        let path = path.as_ref();
        if path.is_relative() {
            return Err(miette!("Path is relative: {path}"));
        }
    }

    // Get the starting set of nodes with no incoming edges.
    // TODO: This can contain duplicate paths.
    let mut queue = paths
        .iter()
        .map(|path| path.as_ref())
        .filter(|path| {
            incoming_edges
                .get(path)
                .map(|edges_to_path| edges_to_path.is_empty())
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    // Collect the sorted list.
    let mut sorted = Vec::new();
    while let Some(path) = queue.pop() {
        sorted.push(path.to_owned());

        if let Some(path_edges) = edges.remove(path) {
            for next in path_edges {
                // There is an edge from `path` to `next`.
                // Remove `next <- path` incoming edge.
                if let Some(next_incoming_edges) = incoming_edges.get_mut(next) {
                    next_incoming_edges.remove(path);
                    if next_incoming_edges.is_empty() {
                        incoming_edges.remove(next);
                        queue.push(next);
                    }
                }
            }
        }
    }

    if edges.values().map(|edges| edges.len()).sum::<usize>() > 0 {
        unreachable!("The graph formed by common prefixes in directory names has cycles, which should not be possible")
    } else {
        Ok(sorted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[track_caller]
    fn test_topological_sort(input: &[&str], expect: &[&str]) {
        let input = input.iter().map(Utf8Path::new).collect::<Vec<_>>();
        let expect = expect.iter().map(Utf8Path::new).collect::<Vec<_>>();
        assert_eq!(topological_sort(&input).unwrap(), expect);
    }

    #[test]
    fn test_topological_sort_empty() {
        test_topological_sort(&[], &[]);
    }

    #[test]
    fn test_topological_sort_unrelated() {
        test_topological_sort(
            &["/puppy", "/doggy", "/softie", "/cutie"],
            &["/cutie", "/softie", "/doggy", "/puppy"],
        );
    }

    #[test]
    fn test_topological_sort_mixed() {
        test_topological_sort(
            &[
                "/puppy",
                "/puppy/doggy/cutie",
                "/puppy/softie",
                "/puppy/doggy",
                "/silly",
                "/silly/goofy",
            ],
            &[
                "/silly/goofy",
                "/silly",
                "/puppy/softie",
                "/puppy/doggy/cutie",
                "/puppy/doggy",
                "/puppy",
            ],
        );
    }

    #[test]
    fn test_topological_sort_duplicate() {
        // This also warns the user.
        test_topological_sort(&["/puppy", "/puppy"], &["/puppy", "/puppy"]);
    }

    #[test]
    fn test_topological_sort_deterministic() {
        test_topological_sort(
            &[
                "/puppy",
                "/silly/puppy",
                "/my-repo",
                "/silly/my-repo",
                "/puppy.git",
                "/a",
                "/b",
                "/c",
                "/d/c",
                "/e/c",
            ],
            &[
                "/e/c",
                "/d/c",
                "/c",
                "/b",
                "/a",
                "/puppy.git",
                "/silly/my-repo",
                "/my-repo",
                "/silly/puppy",
                "/puppy",
            ],
        );
    }
}
