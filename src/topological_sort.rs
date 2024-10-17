use camino::Utf8Path;
use camino::Utf8PathBuf;
use miette::miette;
use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet as HashSet;

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
#[cfg_attr(not(test), expect(dead_code))]
pub fn topological_sort<P>(paths: &[P]) -> miette::Result<Vec<Utf8PathBuf>>
where
    P: AsRef<Utf8Path>,
{
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    // Compute edges.
    let mut edges = HashMap::<&Utf8Path, HashSet<&Utf8Path>>::default();
    let mut incoming_edges = HashMap::<&Utf8Path, HashSet<&Utf8Path>>::default();
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

    #[test]
    fn test_topological_sort_empty() {
        assert_eq!(
            topological_sort(&Vec::<&Utf8Path>::new()).unwrap(),
            Vec::<Utf8PathBuf>::new()
        );
    }

    #[test]
    fn test_topological_sort_unrelated() {
        assert_eq!(
            topological_sort(&[
                Utf8Path::new("/puppy"),
                Utf8Path::new("/doggy"),
                Utf8Path::new("/softie"),
                Utf8Path::new("/cutie"),
            ])
            .unwrap(),
            vec![
                // TODO: This probably depends on the hash function. >:(
                Utf8PathBuf::from("/cutie"),
                Utf8PathBuf::from("/softie"),
                Utf8PathBuf::from("/doggy"),
                Utf8PathBuf::from("/puppy"),
            ]
        );
    }

    #[test]
    fn test_topological_sort_mixed() {
        assert_eq!(
            topological_sort(&[
                Utf8Path::new("/puppy"),
                Utf8Path::new("/puppy/doggy/cutie"),
                Utf8Path::new("/puppy/softie"),
                Utf8Path::new("/puppy/doggy"),
                Utf8Path::new("/silly"),
                Utf8Path::new("/silly/goofy"),
            ])
            .unwrap(),
            vec![
                // TODO: This probably depends on the hash function. >:(
                Utf8PathBuf::from("/silly/goofy"),
                Utf8PathBuf::from("/silly"),
                Utf8PathBuf::from("/puppy/softie"),
                Utf8PathBuf::from("/puppy/doggy/cutie"),
                Utf8PathBuf::from("/puppy/doggy"),
                Utf8PathBuf::from("/puppy"),
            ]
        );
    }

    #[test]
    fn test_topological_sort_duplicate() {
        // This also warns the user.
        assert_eq!(
            topological_sort(&[Utf8Path::new("/puppy"), Utf8Path::new("/puppy")]).unwrap(),
            vec![Utf8PathBuf::from("/puppy"), Utf8PathBuf::from("/puppy")]
        );
    }
}
