use std::collections::BTreeMap;

pub struct Edge {
    pub pair: usize,
    pub i: usize,
    pub j: usize,
    pub hard: bool,
    pub p: f64,
}

pub struct Clusters {
    pub groups: Vec<Vec<usize>>,
    pub blocked: Vec<(usize, usize)>,
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

pub fn cluster(n: usize, mut edges: Vec<Edge>, cannot: &[(usize, usize, usize)]) -> Clusters {
    let mut parent: Vec<usize> = (0..n).collect();
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut nope: Vec<BTreeMap<usize, usize>> = vec![BTreeMap::new(); n];
    for &(i, j, pair) in cannot {
        nope[i].insert(j, pair);
        nope[j].insert(i, pair);
    }
    edges.sort_by(|a, b| {
        b.hard
            .cmp(&a.hard)
            .then(b.p.total_cmp(&a.p))
            .then(a.pair.cmp(&b.pair))
    });
    let mut blocked = Vec::new();
    for e in &edges {
        let (ri, rj) = (find(&mut parent, e.i), find(&mut parent, e.j));
        if ri == rj {
            continue;
        }
        if let Some(by) = members[rj].iter().find_map(|y| nope[ri].get(y).copied()) {
            blocked.push((e.pair, by));
            continue;
        }
        let (big, small) = if members[ri].len() >= members[rj].len() {
            (ri, rj)
        } else {
            (rj, ri)
        };
        parent[small] = big;
        let moved = std::mem::take(&mut members[small]);
        members[big].extend(moved);
        let moved = std::mem::take(&mut nope[small]);
        nope[big].extend(moved);
    }
    let mut groups: Vec<Vec<usize>> = members.into_iter().filter(|m| !m.is_empty()).collect();
    for g in &mut groups {
        g.sort_unstable();
    }
    groups.sort();
    Clusters { groups, blocked }
}
