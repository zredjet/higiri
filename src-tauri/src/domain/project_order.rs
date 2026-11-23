//! プロジェクトの並び順 (P-4, v0.6.0). Pure.
//!
//! The screen sends the active projects in their new order. Archived projects
//! keep their places among the others, so a project restored from the archive
//! comes back where it was. The whole list is then numbered from 1, which also
//! settles numbers that clash (a JSON file edited by hand may give every
//! project the same `sort_order`).

use std::collections::HashSet;

/// The screen's list of active projects is not the one saved now (a project
/// was added, archived or deleted meanwhile, or an id is given twice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderChanged;

/// Every project id in its new display order.
///
/// `all` is every project, archived ones too, in the current display order
/// (`sort_order`, then `id`), as `(id, archived)`. `active` names the active
/// projects in their new order. The active ones take the places active
/// projects hold now, in the new order; the archived ones stay in theirs.
/// Refused unless `active` names each active project exactly once.
pub fn reordered(all: &[(i64, bool)], active: &[i64]) -> Result<Vec<i64>, OrderChanged> {
    let current: HashSet<i64> = all
        .iter()
        .filter(|(_, archived)| !archived)
        .map(|&(id, _)| id)
        .collect();
    let given: HashSet<i64> = active.iter().copied().collect();
    if given.len() != active.len() || given != current {
        return Err(OrderChanged);
    }
    let mut next = active.iter();
    Ok(all
        .iter()
        .map(|&(id, archived)| {
            if archived {
                id
            } else {
                // `active` has exactly as many ids as there are active projects.
                *next.next().expect("one new place per active project")
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: (i64, bool) = (1, false);
    const B: (i64, bool) = (2, false);
    const C: (i64, bool) = (3, false);
    const X: (i64, bool) = (4, true);

    #[test]
    fn puts_the_active_projects_in_the_new_order() {
        assert_eq!(reordered(&[A, B, C], &[3, 1, 2]), Ok(vec![3, 1, 2]));
        assert_eq!(reordered(&[A, B, C], &[1, 2, 3]), Ok(vec![1, 2, 3]));
        assert_eq!(reordered(&[A], &[1]), Ok(vec![1]));
        assert_eq!(reordered(&[], &[]), Ok(vec![]));
    }

    #[test]
    fn archived_projects_keep_their_places() {
        // A, X(archived), B, C → C, X, A, B: X stays second.
        assert_eq!(reordered(&[A, X, B, C], &[3, 1, 2]), Ok(vec![3, 4, 1, 2]));
        assert_eq!(reordered(&[X, A, B], &[2, 1]), Ok(vec![4, 2, 1]));
        assert_eq!(reordered(&[A, B, X], &[2, 1]), Ok(vec![2, 1, 4]));
        // Only archived projects: nothing to order, nothing moves.
        assert_eq!(reordered(&[X], &[]), Ok(vec![4]));
    }

    #[test]
    fn refuses_a_list_that_is_not_the_active_projects() {
        // Missing, extra, twice, archived, unknown.
        assert_eq!(reordered(&[A, B, C], &[1, 2]), Err(OrderChanged));
        assert_eq!(reordered(&[A, B], &[1, 2, 3]), Err(OrderChanged));
        assert_eq!(reordered(&[A, B], &[1, 1, 2]), Err(OrderChanged));
        assert_eq!(reordered(&[A, B], &[1, 1]), Err(OrderChanged));
        assert_eq!(reordered(&[A, X], &[1, 4]), Err(OrderChanged));
        assert_eq!(reordered(&[A, X], &[4]), Err(OrderChanged));
        assert_eq!(reordered(&[A, B], &[1, 9]), Err(OrderChanged));
        assert_eq!(reordered(&[A], &[]), Err(OrderChanged));
    }
}
