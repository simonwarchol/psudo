use std::collections::HashMap;
use std::f64::consts::LN_2;
use std::fs;
use std::io::Cursor;
use std::iter::FromIterator;
use std::io::{self};

use kd_tree::{KdPoint, KdTree};
use ndarray::Array1;
use ndarray::Array2;
use serde_json::{json, Value};

// define your own item type.
struct Item {
    point: [f64; 3],
    id: usize,
}

// implement `KdPoint` for your item type.
impl KdPoint for Item {
    type Scalar = f64;
    type Dim = typenum::U3;
    // 2 dimensional tree.
    fn at(&self, k: usize) -> f64 { self.point[k] }
}

pub struct C3 {
    color: Array2<i64>,
    c: usize,
    w: usize,
    a: Vec<f64>,
    t: HashMap<i64, i64>,
    terms: Vec<String>,
    min_e: f64,
    max_e: f64,
    color_count: Array1<i64>,
    terms_count: Array1<i64>,
    tree: KdTree<Item>,
}

// make impl public
impl C3 {
    pub fn new() -> C3 {

        let body = include_str!("data.json");
        let json: Value = serde_json::from_str(&body).unwrap();
        let _colorvec: Vec<i64> = serde_json::from_value(json["color"].clone()).unwrap();
        let rows = _colorvec.len() / 3;
        let _color = Array2::from_shape_vec((rows, 3), _colorvec).unwrap();

        let _c = _color.shape()[0];


        let _a: Vec<f64> = serde_json::from_value(json["A"].clone()).unwrap();
        let mut _t = HashMap::new();
        let t_vec: Vec<i64> = serde_json::from_value(json["T"].clone()).unwrap();


        // Iterate over every two elements in the vector
        for pair in t_vec.chunks(2) {
            _t.insert(pair[0], pair[1]);
        }
        let tmp_vec = vec!["green", "blue", "purple", "red", "pink", "yellow", "orange", "brown", "teal", "lightblue", "grey", "limegreen", "magenta", "lightgreen", "brightgreen", "skyblue", "cyan", "turquoise", "darkblue", "darkgreen", "aqua", "olive", "navyblue", "lavender", "fuchsia", "black", "royalblue", "violet", "hotpink", "tan", "forestgreen", "lightpurple", "neongreen", "yellowgreen", "maroon", "darkpurple", "salmon", "peach", "beige", "lime", "seafoamgreen", "mustard", "brightblue", "lilac", "seagreen", "palegreen", "bluegreen", "mint", "lightbrown", "mauve", "darkred", "greyblue", "burntorange", "darkpink", "indigo", "periwinkle", "bluegrey", "lightpink", "aquamarine", "gold", "brightpurple", "grassgreen", "redorange", "bluepurple", "greygreen", "kellygreen", "puke", "rose", "darkteal", "babyblue", "paleblue", "greenyellow", "brickred", "lightgrey", "darkgrey", "white", "brightpink", "chartreuse", "purpleblue", "royalpurple", "burgundy", "goldenrod", "darkbrown", "lightorange", "darkorange", "redbrown", "paleyellow", "plum", "offwhite", "pinkpurple", "darkyellow", "lightyellow", "mustardyellow", "brightred", "peagreen", "khaki", "orangered", "crimson", "deepblue", "springgreen", "cream", "palepink", "yelloworange", "deeppurple", "pinkred", "pastelgreen", "sand", "rust", "lightred", "taupe", "armygreen", "robinseggblue", "huntergreen", "greenblue", "lightteal", "cerulean", "flesh", "orangebrown", "slateblue", "slate", "coral", "blueviolet", "ochre", "leafgreen", "electricblue", "seablue", "midnightblue", "steelblue", "brick", "palepurple", "mediumblue", "burntsienna", "darkmagenta", "eggplant", "sage", "darkturquoise", "puce", "bloodred", "neonpurple", "mossgreen", "terracotta", "oceanblue", "yellowbrown", "brightyellow", "dustyrose", "applegreen", "neonpink", "skin", "cornflowerblue", "lightturquoise", "wine", "deepred", "azure"];
        let _terms: Vec<String> = tmp_vec.iter().map(|s| s.to_string()).collect();


        let _w = _terms.len();
        let mut color_count: Array1<i64> = Array1::zeros(_c);
        let mut terms_count: Array1<i64> = Array1::zeros(_w);
        for key in _t.keys() {
            let mut v = 0;
            if let Some(x) = _t.get(key) {
                v = *x;
            }
            color_count[(*key as f64 / _w as f64).floor() as usize] += v;
            terms_count[(*key % _w as i64) as usize] += v;
        }
        let pts = _color
            .outer_iter()
            .enumerate()
            .map(|(i, row)| Item { point: [row[0] as f64, row[1] as f64, row[2] as f64], id: i })
            .collect();
        let tree: KdTree<Item> = KdTree::build_by_ordered_float(pts);
        // make this a result
        C3 {
            c: _c,
            color: _color,
            a: _a,
            t: _t,
            w: _w,
            terms: _terms,
            min_e: -4.5,
            max_e: 0.0,
            color_count,
            terms_count,
            tree,
        }
    }
    fn color_entropy(&self, c: usize) -> f64 {
        let mut h: f64 = 0.0;
        for w in 0..self.w {
            let val = c as i64 * self.w as i64 + w as i64;
            let mut p = 0.0;
            if let Some(x) = self.t.get(&val as &i64) {
                p = *x as f64 / self.color_count[c] as f64;
            }
            if p > 0.0 {
                h += p * f64::ln(p) / LN_2;
            }
        }
        h
    }
    fn color_related_terms(
        &self,
        c: usize,
        limit: Option<usize>,
        min_count: Option<usize>,
        salience_threshold: Option<f64>,
    ) -> Vec<HashMap<&str, f64>> {
        let cc = c * self.w;
        let mut list = Vec::new();
        let mut sum = 0.0;
        for w in 0..self.w {
            if self.t.contains_key(&(cc as i64 + w as i64)) {
                sum += self.t[&(cc as i64 + w as i64)] as f64;
                list.push(HashMap::from_iter([
                    ("index", w as f64),
                    ("score", self.t[&(cc as i64 + w as i64)] as f64),
                ]));
            }
        }
        let mut filtered_list = list.iter().map(|x: &HashMap<&str, f64>| {
            let score = x["score"] / sum;
            let index = x["index"];
            HashMap::from_iter([("score", score), ("index", index)])
        }).collect::<Vec<HashMap<&str, f64>>>();

        if let Some(threshold) = salience_threshold {
            filtered_list = filtered_list.into_iter().filter(|x: &HashMap<&str, f64>| x["score"] > threshold).collect();
        }
        if let Some(min_count) = min_count {
            filtered_list = filtered_list
                .into_iter()
                .filter(|x| self.terms_count[x["index"] as usize] > min_count as i64)
                .collect();
        }
        filtered_list.sort_by(|a, b| b["score"].partial_cmp(&a["score"]).unwrap());
        if let Some(limit) = limit {
            filtered_list.truncate(limit);
        }
        filtered_list
    }
    pub(crate) fn color_cosine(
        &self,
        a: usize,
        b: usize,
    ) -> f64 {
        let mut sa = 0.0;
        let mut sb = 0.0;
        let mut sc = 0.0;
        for w in 0..self.w {
            let mut ta = 0.0;
            let mut tb = 0.0;
            if let Some(val) = self.t.get(&((a * self.w + w) as i64)) {
                ta = *val as f64;
            }
            if let Some(val) = self.t.get(&((b * self.w + w) as i64)) {
                tb = *val as f64;
            }
            sa += ta * ta;
            sb += tb * tb;
            sc += ta * tb;
        }
        sc / (sa.sqrt() * sb.sqrt())
    }
    fn color_index(&self, c: [f64; 3]) -> usize {
        // let nearest_neighbor = self.tree
        //     Query tree for closest c colors
        let found = self.tree.nearest(&c /* coord */).unwrap();
        let item = found.item.id;
        item
    }

    fn color(&self, _x: [f64; 3]) -> HashMap<&str, f64> {
        let c = self.color_index(_x);
        let h = (self.color_entropy(c) - self.min_e) / (self.max_e - self.min_e);
        let mut map: HashMap<&str, f64> = HashMap::new();
        map.insert("c", c as f64);
        map.insert("h", h);
        map
    }

    pub(crate) fn analyze_palette(&self, palette: Array2<f64>) -> Vec<HashMap<&str, f64>> {
        palette
            .outer_iter()
            .map(|row| self.color([row[0], row[1], row[2]]))
            .collect()
    }
    fn get_palette_terms(&self, palette: Array2<f64>, color_term_limit: usize) -> Vec<Vec<HashMap<&str, f64>>> {
        let mut terms = Vec::new();
        for row in palette.outer_iter() {
            let c = self.color_index([row[0], row[1], row[2]]);
            let related_terms = self.color_related_terms(c, Some(color_term_limit), None, None);
            terms.push(related_terms);
        }
        terms
    }
    pub(crate) fn compute_color_name_distance_matrix(&self, data: Vec<HashMap<&str, f64>>) -> Array2<f64> {
        let n = data.len();
        let mut matrix = Array2::zeros((n, n));

        for i in 0..n {
            for j in 0..i {
                let cosine_distance = 1.0 - self.color_cosine(*data[i].get("c").unwrap() as usize, *data[j].get("c").unwrap() as usize);
                matrix[[i, j]] = cosine_distance;
                matrix[[j, i]] = cosine_distance;
            }
        }
        matrix
    }
}
//
// fn main() {
// //     Create new instance of C3
//     let mut c_3 = C3::new();
//     // -2.9358562554686025 should be value
//     let ce_test = c_3.color_entropy(7271);
//     println!("Color Entropy Test: {}", ce_test);
//     // [{'score': 0.31417624521072796, 'index': 4}, {'score': 0.1839080459770115, 'index': 12}, {'score': 0.13409961685823754, 'index': 2}, {'score': 0.1111111111111111, 'index': 24}, {'score': 0.09961685823754789, 'index': 28}, {'score': 0.034482758620689655, 'index': 76}, {'score': 0.02681992337164751, 'index': 60}, {'score': 0.019157088122605363, 'index': 27}, {'score': 0.019157088122605363, 'index': 31}, {'score': 0.019157088122605363, 'index': 89}]
//     let related_terms = c_3.color_related_terms(7271, Some(10), None, None);
//     println!("Related Terms Test: {:?}", related_terms);
//     //0.009036541917907336
//     let cosine_test = c_3.color_cosine(2173, 7271);
//     println!("Cosine Test: {}", cosine_test);
//     // Should be 7271
//     let color_index_test = c_3.color_index([60.3, 98.2, -60.8]);
//     println!("Color Index Test: {:?}", color_index_test);
//     let palette = Array2::from_shape_vec((3, 3), vec![
//         60.32273214, 98.2353325, -60.84232404,
//         79.42618245, -1.22650957, -19.14108948,
//         66.88027726, 43.42296322, 71.85391542,
//     ]).unwrap();
//     let analyzed_palette = c_3.analyze_palette(palette.clone());
//     println!("Analyzed Palette: {:?}", analyzed_palette);
//     let palette_terms = c_3.get_palette_terms(palette.clone(), 10);
//     println!("Analyzed Palette Terms: {:?}", palette_terms);
//     let cosine_matrix = c_3.compute_color_name_distance_matrix(analyzed_palette);
//     println!("Cosine Matrix: {:?}", cosine_matrix[[2, 1]]);
// //     print shape of cosine matrix
//     println!("Cosine Matrix: {:?}", cosine_matrix.shape());
// }