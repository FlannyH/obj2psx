#![allow(clippy::identity_op, clippy::too_many_arguments, dead_code)]
use std::path::Path;

use psx_structs::{ModelPSX, TextureCollectionPSX};
use stb_image::stb_image::bindgen::stbi_set_flip_vertically_on_load;
use tobj::LoadOptions;

use crate::psx_structs::{MeshPSX, TextureCellPSX, VertexPSX};
use exoquant::{convert_to_indexed, ditherer, optimizer, Color};
mod helpers;
mod psx_structs;

fn main() {
    unsafe {stbi_set_flip_vertically_on_load(1);}
    let input_obj = String::from("D:\\Projects\\Git\\ShooterPSX\\assets_to_build\\level.obj");
    let (models, materials) = tobj::load_obj(
        "D:\\Projects\\Git\\ShooterPSX\\assets_to_build\\level.obj",
        &LoadOptions {
            single_index: true,
            ..Default::default()
        },
    )
    .expect("Failed to OBJ load file");

    let mut model_psx = ModelPSX::new();
    let mut txc_psx = TextureCollectionPSX::new();

    for model in &models {
        println!("parsing {}", model.name);
        let mut curr_index = 0;
        let mut triangles = Vec::<VertexPSX>::new();
        let mut quads = Vec::<VertexPSX>::new();

        let face_arities = match model.mesh.face_arities.is_empty() {
            false => model.mesh.face_arities.clone(),
            true => std::iter::repeat(3)
                .take(model.mesh.indices.len() / 3)
                .collect(),
        };

        for arity in &face_arities {
            let mut curr_primitive = Vec::<VertexPSX>::new();
            for in_face_index in curr_index as usize..(curr_index + arity) as usize {
                let index = model.mesh.indices[in_face_index] as usize;
                let vert = VertexPSX {
                    pos_x: (model.mesh.positions[index * 3 + 0] * -1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    pos_y: (model.mesh.positions[index * 3 + 1] * -1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    pos_z: (model.mesh.positions[index * 3 + 2] * 1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    color_r: (model.mesh.vertex_color[index * 3 + 0] * 255.0).clamp(0.0, 255.0)
                        as u8,
                    color_g: (model.mesh.vertex_color[index * 3 + 1] * 255.0).clamp(0.0, 255.0)
                        as u8,
                    color_b: (model.mesh.vertex_color[index * 3 + 2] * 255.0).clamp(0.0, 255.0)
                        as u8,
                    tex_u: (model.mesh.texcoords[index * 2 + 0] * 255.0) as u8,
                    tex_v: (model.mesh.texcoords[index * 2 + 1] * 255.0) as u8,
                    texture_id: match model.mesh.material_id {
                        None => 255,
                        Some(a) => a as u8,
                    },
                };
                curr_primitive.push(vert);
            }
            match arity {
                3 => {
                    for i in [0, 2, 1] {
                        triangles.push(curr_primitive[i]);
                    }
                },
                4 => {
                    for i in [0, 3, 1, 2]{
                        quads.push(curr_primitive[i]);
                    }
                },
                _ => println!("found polygon with more than 4 vertices! make sure the mesh only contains triangles and quads."),
            };

            curr_index += arity;
        }

        let n_triangles = triangles.len() / 3;
        let n_quads = quads.len() / 4;
        let mut combined_vector = Vec::<VertexPSX>::new();
        combined_vector.extend(triangles);
        combined_vector.extend(quads);
        model_psx.meshes.push(MeshPSX {
            verts: combined_vector,
            n_triangles,
            n_quads,
        });
        println!("{n_triangles} triangles and {n_quads} quads processed");
    }

    if let Ok(materials) = materials {
        for material in materials {
            let tex_data_src;
            let depth;
            let name: String;
            if material.diffuse_texture.is_some() {
                // Load the image file corresponding to the material
                let input_path = Path::new(&input_obj);
                let parent_directory = input_path.parent().expect("Invalid file path");
                let combined_path = parent_directory.join(material.diffuse_texture.unwrap());
                name = String::from(combined_path.to_str().unwrap());
                println!("converting texture {}", name);
                let raw_image = match stb_image::image::load(&name) {
                    stb_image::image::LoadResult::ImageU8(image) => image,
                    _ => panic!(),
                };

                // todo: Fit it to 64x64 pixels
                tex_data_src = raw_image.data;
                depth = raw_image.depth;
            } else {
                tex_data_src = vec![0xFF; 64*64*4];
                depth = 4;
                name = String::from("none");
            }

            // Create texture cell object
            let mut tex_cell = TextureCellPSX {
                texture_data: Vec::new(),
                palette: Vec::new(),
                texture_width: 64,
                texture_height: 64,
                avg_color: 0,
            };
            // Calculate average color
            let mut avg_r = 0;
            let mut avg_g = 0;
            let mut avg_b = 0;
            let mut avg_a = 0;
            for pixel in tex_data_src.chunks(depth) {
                avg_r += pixel[0] as u32;
                avg_g += pixel[1] as u32;
                avg_b += pixel[2] as u32;
                match depth {
                    4 => avg_a += pixel[3] as u32,
                    3 => avg_a += 255,
                    _ => panic!(),
                }
                if depth == 4 {
                    avg_a += pixel[3] as u32;
                } else {
                    avg_a += 255
                }
            }
            avg_r /= 64 * 64;
            avg_g /= 64 * 64;
            avg_b /= 64 * 64;
            avg_a /= 64 * 64;
            tex_cell.avg_color = avg_r | avg_b << 8 | avg_g << 16 | avg_a << 24;

            // Quantize it to 16 colours
            let mut tex_data_exoquant = Vec::new();
            for pixel in tex_data_src.chunks(depth) {
                match depth {
                    4 => tex_data_exoquant
                        .push(exoquant::Color::new(pixel[0], pixel[1], pixel[2], pixel[3])),
                    3 => tex_data_exoquant
                        .push(exoquant::Color::new(pixel[0], pixel[1], pixel[2], 255)),
                    _ => panic!(),
                }
            }
            let (palette, indexed_data) = convert_to_indexed(
                &tex_data_exoquant,
                64,
                16,
                &optimizer::KMeans,
                &ditherer::Ordered,
            );
            let color_b = Color {
                r: (avg_r) as u8,
                g: (avg_g) as u8,
                b: (avg_b) as u8,
                a: (avg_a) as u8,
            };
            for fade_level in 0..16 {
                for color in &palette {
                    let color: u16 = (color.a as u16).clamp(0, 1) << 15
                        | ((((fade_level * color_b.b as u16)
                            + ((15 - fade_level) * color.b as u16))
                            / 15)
                            >> 3)
                            .clamp(0, 31)
                            << 10
                        | ((((fade_level * color_b.g as u16)
                            + ((15 - fade_level) * color.g as u16))
                            / 15)
                            >> 3)
                            .clamp(0, 31)
                            << 5
                        | ((((fade_level * color_b.r as u16)
                            + ((15 - fade_level) * color.r as u16))
                            / 15)
                            >> 3)
                            .clamp(0, 31)
                            << 0;
                    tex_cell.palette.push(color);
                }
            }

            // Convert indices to 4 bit
            println!("{:?}", indexed_data.len());
            for i in (0..(64 * 64)).step_by(2) {
                if (i + 1) < indexed_data.len() {
                    tex_cell
                        .texture_data
                        .push((indexed_data[i + 0] << 4) | (indexed_data[i + 1]));
                } else {
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                }
            }

            // Add this cell to the collection
            txc_psx.texture_cells.push(tex_cell);
            txc_psx.texture_names.push(name);
        }
    }

    model_psx
        .save(Path::new(
            "D:\\Projects\\Git\\ShooterPSX\\ASSETS\\level.msh",
        ))
        .unwrap();
    txc_psx
        .save(Path::new(
            "D:\\Projects\\Git\\ShooterPSX\\ASSETS\\level.txc",
        ))
        .unwrap();
}
