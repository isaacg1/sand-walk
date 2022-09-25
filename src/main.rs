#![feature(array_zip)]
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasherDefault;
use twox_hash::XxHash64;

use image::{ImageBuffer, RgbImage};
use rand::prelude::*;

type Color = [u8; 3];
type ColorBase = [u8; 3];

fn color_base_to_color(cb: ColorBase, color_size: u64) -> Color {
    cb.map(|cbc| (cbc as u64 * 255 / (color_size - 1)) as u8)
}
type ColorOffset = [i16; 3];
type Location = [usize; 2];
// type LocationOffset = [isize; 2];

fn make_image(
    scale: u64,
    num_seeds: usize,
    div: f64,
    spacing: bool,
    root_scale: f64,
    seed: u64,
) -> RgbImage {
    let mut rng = StdRng::seed_from_u64(seed);
    let size = scale.pow(3) as usize;
    let color_size = scale.pow(2);
    let mut color_bases: Vec<ColorBase> = (0..scale.pow(6))
        .map(|n| {
            let r_base = n % color_size;
            let g_base = (n / color_size) % color_size;
            let b_base = n / color_size.pow(2);
            [r_base as u8, g_base as u8, b_base as u8]
        })
        .collect();
    let mut color_offsets: Vec<ColorOffset> = color_bases
        .iter()
        .map(|color| color.map(|c| c as i16))
        .flat_map(|color| {
            vec![
                [color[0], color[1], color[2]],
                [color[0], color[1], -color[2]],
                [color[0], -color[1], color[2]],
                [color[0], -color[1], -color[2]],
                [-color[0], color[1], color[2]],
                [-color[0], color[1], -color[2]],
                [-color[0], -color[1], color[2]],
                [-color[0], -color[1], -color[2]],
            ]
            .into_iter()
        })
        .collect();
    color_bases.shuffle(&mut rng);
    color_offsets
        .sort_by_key(|color_offset| color_offset.map(|c| (c as i64).pow(2)).iter().sum::<i64>());

    let mut grid: Vec<Vec<Option<ColorBase>>> = vec![vec![None; size]; size];
    let mut color_base_to_location: HashMap<ColorBase, Location> = HashMap::new();
    let mut seed_locs: Vec<Location> = vec![];
    // Fixed hasher because we use the iteration order for randomization later
    let mut open_locs: HashSet<Location, BuildHasherDefault<XxHash64>> = (0..size)
        .flat_map(|i| (0..size).map(move |j| [i, j]))
        .collect();

    for (i, color_base) in color_bases.into_iter().enumerate() {
        if i < num_seeds {
            let mut row = rng.gen_range(0..size);
            let mut col = rng.gen_range(0..size);
            if spacing {
                loop {
                    let mut too_close = false;
                    for loc in &seed_locs {
                        let dist_sq: isize = loc
                            .zip([row, col])
                            .map(|(l1, l2)| {
                                let il1 = l1 as isize;
                                let il2 = l2 as isize;
                                (il1 - il2)
                                    .abs()
                                    .min(il1 - il2 + size as isize)
                                    .min(il1 - il2 + size as isize)
                            })
                            .map(|d| d.pow(2))
                            .iter()
                            .sum::<isize>();
                        let dist: f64 = (dist_sq as f64).sqrt();
                        let min_spacing = size as f64 / (2.0 * (num_seeds as f64).sqrt());
                        if dist < min_spacing {
                            too_close = true;
                        }
                    }
                    if !too_close {
                        break;
                    }
                    row = rng.gen_range(0..size);
                    col = rng.gen_range(0..size);
                }
            }
            let pixel = color_base_to_color(color_base, color_size);
            grid[row][col] = Some(pixel);
            color_base_to_location.insert(color_base, [row, col]);
            seed_locs.push([row, col]);
            continue;
        }
        let most_similar_location: Location = color_offsets
            .iter()
            .filter_map(|color_offset| {
                let prov_new_color_base =
                    color_base.zip(*color_offset).map(|(c, co)| c as i16 + co);
                if prov_new_color_base.iter().any(|&c| c < 0 || c > 255) {
                    None
                } else {
                    let new_color_base = prov_new_color_base.map(|c| c as u8);
                    color_base_to_location.get(&new_color_base).copied()
                }
            })
            .next()
            .unwrap();
        let mut x: isize = most_similar_location[0] as isize;
        let mut y: isize = most_similar_location[1] as isize;
        let dirs = [(1, 0), (0, 1), (-1, 0), (0, -1)];
        let clamp = &|f| {
            if f >= size as isize {
                f - size as isize
            } else if f < 0 {
                f + size as isize
            } else {
                f
            }
        };
        let mut i = 0;
        let mut last = rng.gen_range(0..4);
        let root_size = (root_scale * (size as f64).sqrt().ceil()) as usize;
        while grid[x as usize][y as usize].is_some() && i < root_size {
            i += 1;
            // Never opposite last
            let index = if rng.gen::<f64>() < div {
                [1, 3]
                    .map(|i| (last + i) % 4)
                    .into_iter()
                    .min_by_key(|j| {
                        let dir = dirs[*j];
                        let oth_color_base =
                            grid[clamp(x + dir.0) as usize][clamp(y + dir.1) as usize];
                        oth_color_base.map_or(0, |oth_color_base| {
                            color_base
                                .zip(oth_color_base)
                                .map(|(c1, c2)| (c1 as i64 - c2 as i64).pow(2))
                                .into_iter()
                                .sum()
                        })
                    })
                    .unwrap()
            } else {
                last
            };
            last = index;
            let (dx, dy) = dirs[index];
            x += dx;
            y += dy;
            x = clamp(x);
            y = clamp(y);
        }
        let loc = if grid[x as usize][y as usize].is_some() {
            // Depends on hashset ordering - stable because specified
            open_locs
                .iter()
                .take(root_size)
                .min_by_key(|loc| {
                    let lx = loc[0] as isize;
                    let ly = loc[1] as isize;
                    let dx = vec![x - lx, x - lx + size as isize, x - lx - size as isize]
                        .into_iter()
                        .map(|d| d.abs())
                        .min()
                        .unwrap();
                    let dy = vec![y - ly, y - ly + size as isize, y - ly - size as isize]
                        .into_iter()
                        .map(|d| d.abs())
                        .min()
                        .unwrap();
                    dx.pow(2) + dy.pow(2)
                })
                .expect("Somewhere open")
                .clone()
        } else {
            [x as usize, y as usize]
        };
        grid[loc[0]][loc[1]] = Some(color_base);
        color_base_to_location.insert(color_base, loc);
        open_locs.remove(&loc);
    }
    let mut img: RgbImage = ImageBuffer::new(size as u32, size as u32);
    for (i, row) in grid.into_iter().enumerate() {
        for (j, color_base) in row.into_iter().enumerate() {
            if let Some(color_base) = color_base {
                img.put_pixel(
                    i as u32,
                    j as u32,
                    image::Rgb(color_base_to_color(color_base, color_size)),
                );
            }
        }
    }
    img
}

fn main() {
    for rs in 1..=4 {
        let scale = 10;
        let num_seeds = 20;
        let seed = 0;
        let div = 0.5;
        let spacing = false;
        let root_scale = rs as f64;
        let filename = format!(
            "img-{}-{}-{}-{}-{}-{}.png",
            scale,
            num_seeds,
            div,
            if spacing { 't' } else { 'f' },
            root_scale,
            seed
        );
        println!("Start {}", filename);
        let img = make_image(scale, num_seeds, div, spacing, root_scale, seed);
        img.save(&filename).unwrap();
    }
}
