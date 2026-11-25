//! Which policies of a document inherit from themselves (ODRL IM 2.9:
//! "Inheritance MUST NOT be circular"), and which parents are outside the
//! document and so cannot be checked.

/// For each policy, whether it is on an inheritance cycle within the
/// document. `parents[i]` holds the positions of policy `i`'s parents.
///
/// Tarjan's strongly connected components, walked with an explicit stack:
/// linear in policies and links, whatever the depth of the chain.
pub(crate) fn on_cycle(parents: &[Vec<usize>]) -> Vec<bool> {
    let n = parents.len();
    const UNSEEN: usize = usize::MAX;
    let mut index = vec![UNSEEN; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut out = vec![false; n];
    let mut next = 0usize;
    for root in 0..n {
        if index[root] != UNSEEN {
            continue;
        }
        // (node, how many of its parents have been looked at)
        let mut walk: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(top) = walk.last_mut() {
            let v = top.0;
            if let Some(&w) = parents[v].get(top.1) {
                top.1 += 1;
                if index[w] == UNSEEN {
                    index[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    walk.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            walk.pop();
            if let Some(&(u, _)) = walk.last() {
                low[u] = low[u].min(low[v]);
            }
            if low[v] == index[v] {
                let mut component = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack[w] = false;
                    component.push(w);
                    if w == v {
                        break;
                    }
                }
                let cyclic = component.len() > 1 || parents[v].contains(&v);
                for w in component {
                    out[w] = cyclic;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::on_cycle;

    #[test]
    fn cycles_and_chains() {
        assert_eq!(on_cycle(&[]), Vec::<bool>::new());
        assert_eq!(on_cycle(&[vec![0]]), [true]);
        assert_eq!(on_cycle(&[vec![1], vec![0], vec![0]]), [true, true, false]);
        assert_eq!(on_cycle(&[vec![], vec![0], vec![1]]), [false, false, false]);
        assert_eq!(
            on_cycle(&[vec![1], vec![2], vec![0], vec![3, 0]]),
            [true, true, true, true]
        );
        assert_eq!(
            on_cycle(&[vec![1, 2], vec![], vec![1]]),
            [false, false, false]
        );
    }
}
