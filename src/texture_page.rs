use std::path::Path;

use exoquant::{convert_to_indexed, optimizer, ditherer};

use crate::psx_structs::{TextureCollectionPSX, TextureCellPSX};

pub fn txc_from_page(input: &Path) -> TextureCollectionPSX {
    // Open the image
    let image = match stb_image::image::load(input) {
        stb_image::image::LoadResult::Error(_) => panic!(),
        stb_image::image::LoadResult::ImageU8(data) => data,
        stb_image::image::LoadResult::ImageF32(_) => todo!(),
    };

    if image.height > 256 || image.width > 256 {
        eprintln!("Image can not be bigger than 256x256")
    }
    
    // Quantize it to 16 colours
    let mut tex_data_exoquant = Vec::new();
    for pixel in image.data.chunks(image.depth) {
        match image.depth {
            4 => tex_data_exoquant
                .push(exoquant::Color::new(pixel[0], pixel[1], pixel[2], pixel[3])),
            3 => tex_data_exoquant
                .push(exoquant::Color::new(pixel[0], pixel[1], pixel[2], 255)),
            _ => panic!(),
        }
    }
    let (palette, indexed_data) = convert_to_indexed(
        &tex_data_exoquant,
        image.width,
        256,
        &optimizer::KMeans,
        &ditherer::Ordered,
    );

    // Convert palette to 16 bit
    let mut tex_palette = Vec::<u16>::new();
    for color in palette {
        if color.a > 0 {
            tex_palette.push(
                (color.r as u16 & 0b11111000) >> 3 |
                (color.g as u16 & 0b11111000) << 2 | 
                (color.b as u16 & 0b11111000) << 7
            );
        }
        else {
            tex_palette.push(0);
        }
    }

    while tex_palette.len() < 256 {
        tex_palette.push(0);
    }

    let mut txc_psx = TextureCollectionPSX::new();
    txc_psx.texture_names.push(String::from(input.file_name().unwrap().to_string_lossy()));
    txc_psx.texture_cells.push(TextureCellPSX{
        texture_data: indexed_data,
        palette: tex_palette,
        texture_width: (image.width % 256) as u8,
        texture_height: (image.height % 256) as u8,
        avg_color: 0,
    });
    return txc_psx;
}