use std::path::Path;

use exoquant::{
    generate_palette,
    optimizer::{self, Optimizer},
    Color, SimpleColorSpace,
};
use log::{error, warn};

use crate::psx_structs::{TextureCellPSX, TextureCollectionPSX};

pub fn txc_from_page(input: &Path) -> TextureCollectionPSX {
    // Open the image
    let image = match stb_image::image::load(input) {
        stb_image::image::LoadResult::Error(_) => panic!(),
        stb_image::image::LoadResult::ImageU8(data) => data,
        stb_image::image::LoadResult::ImageF32(_) => todo!(),
    };

    if image.height > 256 || image.width > 256 {
        error!("Image can not be bigger than 256x256")
    }

    // Quantize it to 256 colours
    let mut tex_data_exoquant = Vec::new();
    let mut has_transparent_pixels = false;
    for pixel in image.data.chunks(image.depth) {
        match image.depth {
            4 => {
                if pixel[3] == 0 {
                    has_transparent_pixels = true;
                    tex_data_exoquant.push(exoquant::Color::new(0, 0, 0, 0))
                } else {
                    tex_data_exoquant.push(exoquant::Color::new(pixel[0], pixel[1], pixel[2], 255))
                }
            }
            3 => tex_data_exoquant.push(exoquant::Color::new(pixel[0], pixel[1], pixel[2], 255)),
            _ => panic!(),
        }
    }
    // Make half the histogram transparent pixels so that the quantizer actually generates a palette that contains one of those
    let mut histogram_data = tex_data_exoquant.clone();
    if has_transparent_pixels {
        let n = tex_data_exoquant.len();
        for _ in 0..n {
            histogram_data.push(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            });
        }
    }
    let histogram: &exoquant::Histogram = &histogram_data.iter().cloned().collect();
    let palette = generate_palette(
        histogram,
        &SimpleColorSpace::default(),
        &optimizer::WeightedKMeans,
        256,
    );
    let mut palette = optimizer::WeightedKMeans.optimize_palette(
        &SimpleColorSpace::default(),
        &palette,
        histogram,
        8,
    );
    let mut indexed_data = exoquant::Remapper::new(
        &palette,
        &SimpleColorSpace::default(),
        &exoquant::ditherer::Ordered,
    )
    .remap(&tex_data_exoquant, image.width);

    // If the palette has a transparent pixel color, move it to index 0
    // This makes it easiest to implement using Nintendo DS OpenGL implementation
    // Additionally, make the color full black. This way the PS1 knows it's transparent too
    let mut n_transparent_colors = 0;
    let mut transparent_index = 0;
    for (index, color) in palette.iter_mut().enumerate() {
        if color.a == 0 {
            transparent_index = index;
            n_transparent_colors += 1;
            color.r = 0;
            color.g = 0;
            color.b = 0;
        }
    }

    if n_transparent_colors > 0 {
        // Swap the transparent and first colors
        let temp = palette[0];
        palette[0] = palette[transparent_index];
        palette[transparent_index] = temp;

        // Then remap the indices in the texture
        for pixel in &mut indexed_data {
            if *pixel == transparent_index as u8 {
                *pixel = 0;
            } else if *pixel == 0 {
                *pixel = transparent_index as u8;
            }
        }
    } else if n_transparent_colors > 1 {
        warn!("multiple transparent colors detected in texture {input:?}")
    }

    // Convert palette to 16 bit
    let mut tex_palette = Vec::<u16>::new();
    for color in palette {
        if color.a > 0 {
            tex_palette.push(
                (color.r as u16 & 0b11111000) >> 3
                    | (color.g as u16 & 0b11111000) << 2
                    | (color.b as u16 & 0b11111000) << 7
                    | match color.a {
                        0 => 0,
                        _ => 1 << 15,
                    },
            );
        } else {
            tex_palette.push(0);
        }
    }

    while tex_palette.len() < 256 {
        tex_palette.push(0);
    }

    let mut txc_psx = TextureCollectionPSX::new();
    txc_psx
        .texture_names
        .push(String::from(input.file_name().unwrap().to_string_lossy()));
    txc_psx.texture_cells.push(TextureCellPSX {
        texture_data: indexed_data,
        palette: tex_palette,
        texture_width: (image.width % 256) as u8,
        texture_height: (image.height % 256) as u8,
        texture_bpp: 8,
        avg_color: 0,
    });
    txc_psx
}
