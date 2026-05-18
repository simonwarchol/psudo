//! Fast smoke: 2 optimized 6-color palettes. Run: `cargo test -p psudo palette_study_smoke`

use psudo::optimize;
use rand::rngs::StdRng;
use rand::{ Rng, SeedableRng };

const CHANNELS: usize = 6;

fn empty_names(n: usize) -> Vec<String> {
    (0..n).map(|_| String::new()).collect()
}

fn contrast_all(channels: usize) -> Vec<u16> {
    (0..channels).flat_map(|_| [0u16, 65535]).collect()
}

fn random_intensities(n_rows: usize, channels: usize, rng: &mut StdRng) -> Vec<u16> {
    let mut out = vec![0u16; n_rows * channels];
    for ch in 0..channels {
        for row in 0..n_rows {
            out[ch * n_rows + row] = rng.gen_range(2000u16..62000u16);
        }
    }
    out
}

fn random_colors_u16(channels: usize, rng: &mut StdRng) -> Vec<u16> {
    let mut c = Vec::with_capacity(channels * 3);
    for _ in 0..channels {
        let dominant = rng.gen_range(0u8..3);
        let mut rgb = [rng.gen_range(40u16..80), rng.gen_range(40u16..80), rng.gen_range(40u16..80)];
        rgb[dominant as usize] = rng.gen_range(200u16..255);
        c.extend_from_slice(&rgb);
    }
    c
}

#[test]
fn palette_study_smoke_two_palettes() {
    let n_rows = 128usize;
    let max_iters = 120u32;
    let lum = vec![45u16, 92];

    for i in 0..2usize {
        let mut rng = StdRng::seed_from_u64(7u64 + i as u64);
        let colors = random_colors_u16(CHANNELS, &mut rng);
        let locked = vec![0u16; CHANNELS];
        let intensities = random_intensities(n_rows, CHANNELS, &mut rng);
        let out = optimize(
            &colors,
            &locked,
            &intensities,
            &contrast_all(CHANNELS),
            &lum,
            vec![],
            empty_names(CHANNELS),
            Some(max_iters),
            Some(6),
            Some(false),
            Some(2),
        );
        assert_eq!(
            out.len(),
            CHANNELS * 3,
            "{CHANNELS} channels × 3 linear sRGB"
        );
    }
    eprintln!("[palette_study_smoke] ok");
}
