use crate::compare::{level, Feature};
use crate::config::FieldConfig;

pub type Gamma = Vec<Option<u8>>;

pub struct Model {
    pub m: Vec<Vec<f64>>,
    pub u: Vec<Vec<f64>>,
    pub lambda: f64,
}

pub fn gamma(a: &[Option<Feature>], b: &[Option<Feature>], fcs: &[&FieldConfig]) -> Gamma {
    fcs.iter()
        .enumerate()
        .map(|(f, fc)| match (&a[f], &b[f]) {
            (Some(x), Some(y)) => Some(level(x, y, fc)),
            _ => None,
        })
        .collect()
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n as u64) as usize
    }
}

pub fn estimate_u(
    feats: &[Vec<Option<Feature>>],
    fcs: &[&FieldConfig],
    levels: &[usize],
    sample: usize,
) -> Vec<Vec<f64>> {
    let n = feats.len();
    let mut counts: Vec<Vec<f64>> = levels.iter().map(|l| vec![0.0; *l]).collect();
    let mut add = |i: usize, j: usize| {
        for (f, g) in gamma(&feats[i], &feats[j], fcs).into_iter().enumerate() {
            if let Some(l) = g {
                counts[f][l as usize] += 1.0;
            }
        }
    };
    let total = n.saturating_mul(n.saturating_sub(1)) / 2;
    if total <= sample {
        for i in 0..n {
            for j in i + 1..n {
                add(i, j);
            }
        }
    } else {
        let mut rng = Lcg(0x9E37_79B9_7F4A_7C15);
        let mut drawn = 0;
        while drawn < sample {
            let (i, j) = (rng.next(n), rng.next(n));
            if i != j {
                add(i, j);
                drawn += 1;
            }
        }
    }
    counts
        .into_iter()
        .map(|c| {
            let seen: f64 = c.iter().sum();
            let l = c.len() as f64;
            c.iter().map(|x| (x + 1.0) / (seen + l)).collect()
        })
        .collect()
}

fn initial_m(levels: usize) -> Vec<f64> {
    if levels == 2 {
        return vec![0.9, 0.1];
    }
    let middle = 0.3 / (levels - 2) as f64;
    (0..levels)
        .map(|l| match l {
            0 => 0.6,
            l if l == levels - 1 => 0.1,
            _ => middle,
        })
        .collect()
}

impl Model {
    pub fn fit(
        gammas: &[Gamma],
        labels: &[Option<bool>],
        u: Vec<Vec<f64>>,
        iterations: usize,
    ) -> Model {
        let mut model = Model {
            m: u.iter().map(|l| initial_m(l.len())).collect(),
            u,
            lambda: 0.1,
        };
        if gammas.is_empty() {
            return model;
        }
        for _ in 0..iterations {
            let posts: Vec<f64> = gammas
                .iter()
                .zip(labels)
                .map(|(g, l)| match l {
                    Some(true) => 1.0,
                    Some(false) => 0.0,
                    None => model.score(g).0,
                })
                .collect();
            model.lambda = (posts.iter().sum::<f64>() / posts.len() as f64).clamp(1e-4, 1.0 - 1e-4);
            for f in 0..model.m.len() {
                let mut acc = vec![0.0; model.m[f].len()];
                for (g, p) in gammas.iter().zip(&posts) {
                    if let Some(l) = g[f] {
                        acc[l as usize] += p;
                    }
                }
                let seen: f64 = acc.iter().sum();
                let k = acc.len() as f64;
                model.m[f] = acc
                    .iter()
                    .map(|x| ((x + 0.01) / (seen + 0.01 * k)).clamp(1e-4, 1.0 - 1e-4))
                    .collect();
            }
        }
        model
    }

    pub fn score(&self, g: &Gamma) -> (f64, Vec<Option<f64>>) {
        let weights: Vec<Option<f64>> = g
            .iter()
            .enumerate()
            .map(|(f, l)| l.map(|l| (self.m[f][l as usize] / self.u[f][l as usize]).ln()))
            .collect();
        let logit =
            (self.lambda / (1.0 - self.lambda)).ln() + weights.iter().flatten().sum::<f64>();
        (1.0 / (1.0 + (-logit).exp()), weights)
    }
}
